// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, CString, c_int, c_uint};
use std::fs::{File, FileType};
use std::io;
use std::io::{BufRead, BufReader};
use std::ops::Range;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use libmount_sys::{
    MNT_ITER_BACKWARD, mnt_fs_get_target, mnt_new_table_from_file, mnt_table_find_devno,
};

use crate::errors::LsLocksError;

pub(crate) static _PATH_PROC: &str = "/proc";
pub(crate) static _PATH_PROC_LOCKS: &str = "/proc/locks";
static _PATH_PROC_MOUNTINFO: &str = "/proc/self/mountinfo";
static _PATH_PROC_MOUNTINFO_C: &CStr = c"/proc/self/mountinfo";

pub(crate) fn entry_is_dir_or_unknown(file_type: &FileType) -> bool {
    file_type.is_dir()
        || (!file_type.is_file()
            && !file_type.is_symlink()
            && !file_type.is_block_device()
            && !file_type.is_char_device()
            && !file_type.is_fifo()
            && !file_type.is_socket())
}

pub(crate) fn proc_pid_command_name(proc_path: &Path) -> Result<CString, LsLocksError> {
    let path = proc_path.join("comm");

    let mut contents =
        std::fs::read(&path).map_err(|err| LsLocksError::io1("reading file", &path, err))?;
    if contents.last().copied() == Some(0_u8) {
        contents.pop();
    }
    if contents.last().copied() == Some(b'\n') {
        contents.pop();
    }

    CString::new(contents).map_err(|_| {
        let err = io::ErrorKind::InvalidData;
        LsLocksError::io1("invalid data", &path, err)
    })
}

fn pid_command_name(pid: libc::pid_t) -> Result<CString, LsLocksError> {
    proc_pid_command_name(&Path::new(_PATH_PROC).join(pid.to_string()))
}

fn path_and_size_of_inode_opened_by_process(
    pid: libc::pid_t,
    inode: libc::ino_t,
) -> Result<(PathBuf, u64), LsLocksError> {
    let path = Path::new(_PATH_PROC).join(format!("{pid}")).join("fd");

    let dir_entries = path
        .read_dir()
        .map_err(|err| LsLocksError::io1("reading directory", &path, err))?;

    for entry in dir_entries {
        let entry =
            entry.map_err(|err| LsLocksError::io1("reading directory entry", &path, err))?;

        if entry
            .file_name()
            .as_bytes()
            .iter()
            .any(|&b| !b.is_ascii_digit())
        {
            continue;
        }

        let path = entry.path();

        let md = path
            .metadata()
            .map_err(|err| LsLocksError::io1("reading file metadata", &path, err))?;

        if md.ino() == inode {
            let path = entry
                .path()
                .read_link()
                .map_err(|err| LsLocksError::io1("reading symbolic link", &path, err))?;

            return Ok((path, md.len()));
        }
    }

    Err(LsLocksError::io0(
        "looking for inode open in process",
        io::ErrorKind::NotFound,
    ))
}

pub(crate) struct BinFileLineIter {
    path: PathBuf,
    input: BufReader<File>,
    buffer: Vec<u8>,
}

impl BinFileLineIter {
    pub(crate) fn open(path: &Path) -> Result<Self, LsLocksError> {
        let input = File::open(path)
            .map(BufReader::new)
            .map_err(|err| LsLocksError::io1("opening file", path, err))?;

        Ok(Self {
            path: path.into(),
            input,
            buffer: Vec::default(),
        })
    }

    pub(crate) fn next_line(&mut self) -> Result<Option<&[u8]>, LsLocksError> {
        self.buffer.clear();

        let count = self
            .input
            .read_until(b'\n', &mut self.buffer)
            .map_err(|err| LsLocksError::io1("reading file", &self.path, err))?;
        if count == 0 {
            return Ok(None);
        }

        if self.buffer.last().copied() == Some(b'\n') {
            self.buffer.pop();
        }
        if self.buffer.last().copied() == Some(b'\r') {
            self.buffer.pop();
        }
        Ok(Some(&self.buffer))
    }
}

pub(crate) struct LockInfo {
    pub(crate) command_name: Option<CString>,
    pub(crate) process_id: libc::pid_t,
    pub(crate) path: Option<PathBuf>,
    pub(crate) kind: CString,
    pub(crate) mode: CString,
    pub(crate) range: Range<u64>,
    pub(crate) inode: libc::ino_t,
    pub(crate) device_id: libc::dev_t,
    pub(crate) mandatory: bool,
    pub(crate) blocked: bool,
    pub(crate) size: Option<u64>,
    pub(crate) file_descriptor: c_int,
    pub(crate) id: i64,
}

impl LockInfo {
    pub(crate) fn parse(
        no_inaccessible: bool,
        fdinfo_path: &Path,
        process_id: Option<libc::pid_t>,
        file_descriptor: c_int,
        command_name: &CStr,
        pid_locks: Option<&[Self]>,
        line: &[u8],
    ) -> Result<Option<Self>, LsLocksError> {
        let err_map = || {
            let err = io::ErrorKind::InvalidData;
            LsLocksError::io1("parsing lock information", fdinfo_path, err)
        };

        let mut elements = line
            .split(|&b| b.is_ascii_whitespace())
            .filter(|&b| !b.is_empty());

        let element = elements.next().ok_or_else(err_map)?;

        let id = if process_id.is_none() {
            element
                .strip_suffix(b":")
                .ok_or_else(err_map)
                .and_then(|b| std::str::from_utf8(b).map_err(|_| err_map()))?
                .parse()
                .map_err(|_| err_map())?
        } else {
            -1
        };

        let mut blocked = false;

        let kind = loop {
            let element = elements.next().ok_or_else(err_map)?;
            if element != b"->" {
                break CString::new(element).map_err(|_| err_map())?;
            }

            blocked = true;
        };

        let mandatory = elements.next().ok_or_else(err_map)?.starts_with(b"M");

        let element = elements.next().ok_or_else(err_map)?;
        let mode = CString::new(element).map_err(|_| err_map())?;

        let mut unknown_command_name = false;

        let element = elements.next().ok_or_else(err_map)?;

        let (mut process_id, mut command_name) = if let Some(process_id) = process_id {
            (process_id, Some(CString::from(command_name)))
        } else {
            let process_id: libc::pid_t = std::str::from_utf8(element)
                .map_err(|_| err_map())?
                .parse()
                .map_err(|_| err_map())?;

            let command_name = if process_id > 0 {
                if let Ok(cmd_line) = pid_command_name(process_id).map(Some) {
                    cmd_line
                } else {
                    unknown_command_name = true;
                    None
                }
            } else {
                None
            };

            (process_id, command_name)
        };

        let mut iter = elements.next().ok_or_else(err_map)?.split(|&b| b == b':');

        let major = iter
            .next()
            .ok_or_else(err_map)
            .and_then(|b| std::str::from_utf8(b).map_err(|_| err_map()))
            .and_then(|s| c_uint::from_str_radix(s, 16).map_err(|_| err_map()))?;
        let minor = iter
            .next()
            .ok_or_else(err_map)
            .and_then(|b| std::str::from_utf8(b).map_err(|_| err_map()))
            .and_then(|s| c_uint::from_str_radix(s, 16).map_err(|_| err_map()))?;
        let inode = iter
            .next()
            .ok_or_else(err_map)
            .and_then(|b| std::str::from_utf8(b).map_err(|_| err_map()))?
            .parse()
            .map_err(|_| err_map())?;

        let device_id = libc::makedev(major, minor);

        let element = elements.next().ok_or_else(err_map)?;

        let start = if element == b"EOF" {
            0
        } else {
            std::str::from_utf8(element)
                .map_err(|_| err_map())?
                .parse()
                .map_err(|_| err_map())?
        };

        let element = elements.next().ok_or_else(err_map)?;

        let end = if element == b"EOF" {
            0
        } else {
            std::str::from_utf8(element)
                .map_err(|_| err_map())?
                .parse()
                .map_err(|_| err_map())?
        };

        let range = start..end;

        if let Some(pid_locks) = pid_locks
            && command_name.is_none()
            && !blocked
        {
            let lock_compare = |lock: &&LockInfo| {
                lock.range == range
                    && lock.inode == inode
                    && lock.device_id == device_id
                    && lock.mandatory == mandatory
                    && lock.blocked == blocked
                    && lock.kind == kind
                    && lock.mode == mode
            };

            if let Some(found) = pid_locks.iter().find(lock_compare) {
                process_id = found.process_id;
                command_name = found.command_name.clone();
            }
        }

        if command_name.is_none() {
            command_name = if unknown_command_name {
                Some(CString::from(c"(unknown)"))
            } else {
                Some(CString::from(c"(undefined)"))
            };
        }

        let (mut path, size) = path_and_size_of_inode_opened_by_process(process_id, inode)
            .ok()
            .map_or((None, None), |(path, size)| (Some(path), Some(size)));

        if path.is_none() {
            if no_inaccessible {
                return Ok(None);
            }

            path = fall_back_file_name(device_id).ok();
        }

        Ok(Some(Self {
            command_name,
            process_id,
            path,
            kind,
            mode,
            range,
            inode,
            device_id,
            mandatory,
            blocked,
            size,
            file_descriptor,
            id,
        }))
    }
}

fn fall_back_file_name(device_id: libc::dev_t) -> Result<PathBuf, LsLocksError> {
    let table = unsafe { mnt_new_table_from_file(_PATH_PROC_MOUNTINFO_C.as_ptr()) };
    if table.is_null() {
        return Err(LsLocksError::io1(
            "mnt_new_table_from_file",
            _PATH_PROC_MOUNTINFO,
            io::ErrorKind::InvalidData,
        ));
    };

    let fs = unsafe { mnt_table_find_devno(table, device_id, MNT_ITER_BACKWARD as c_int) };
    if fs.is_null() {
        let err = io::ErrorKind::NotFound;
        return Err(LsLocksError::io0("mnt_table_find_devno", err));
    };

    let target = unsafe { mnt_fs_get_target(fs) };
    if target.is_null() {
        let err = io::ErrorKind::NotFound;
        return Err(LsLocksError::io0("mnt_fs_get_target", err));
    };

    let target = unsafe { CStr::from_ptr(target) }
        .to_str()
        .map_err(|_| LsLocksError::io0("data is not UTF-8", io::ErrorKind::InvalidData))?;

    Ok(PathBuf::from(format!(
        "{target}{}...",
        if target.ends_with("/") { "" } else { "/" }
    )))
}
