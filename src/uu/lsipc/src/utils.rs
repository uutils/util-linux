// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::AtomicU64;
use std::{io, ptr};

use crate::errors::LsIpcError;

pub(crate) fn get_page_size() -> Result<u64, LsIpcError> {
    use std::sync::atomic::Ordering;

    static VALUE: AtomicU64 = AtomicU64::new(0);

    let mut page_size = VALUE.load(Ordering::Acquire);

    if page_size == 0 {
        let value = unsafe { libc::sysconf(libc::_SC_PAGE_SIZE) };
        if value == -1 || value == 0 {
            return Err(LsIpcError::last_io0("sysconf(_SC_PAGE_SIZE)"));
        }

        let value = value as u64;

        match VALUE.compare_exchange(0, value, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_previous_value) => page_size = value,
            Err(previous_value) => page_size = previous_value,
        }
    }

    Ok(page_size)
}

pub(crate) fn local_time(time: libc::time_t) -> Result<libc::tm, LsIpcError> {
    let mut detailed_time: libc::tm;

    let r = unsafe {
        detailed_time = std::mem::zeroed();
        libc::localtime_r(&time, &mut detailed_time)
    };

    if r.is_null() {
        Err(LsIpcError::last_io0("localtime_r"))
    } else {
        Ok(detailed_time)
    }
}

pub(crate) fn time_of_day() -> Result<libc::timeval, LsIpcError> {
    let mut time: libc::timeval;
    let r = unsafe {
        time = std::mem::zeroed();
        libc::gettimeofday(&mut time, ptr::null_mut())
    };

    if r == -1 {
        Err(LsIpcError::last_io0("gettimeofday"))
    } else {
        Ok(time)
    }
}

pub(crate) fn pid_command_line(pid: libc::pid_t) -> Result<CString, LsIpcError> {
    let path = Path::new("/proc").join(pid.to_string()).join("cmdline");

    let mut contents =
        std::fs::read(&path).map_err(|err| LsIpcError::io1("reading file", path, err))?;
    if contents.last().copied() == Some(0_u8) {
        contents.pop();
    }
    find_replace_in_vec(0_u8, b' ', &mut contents);
    contents.push(0_u8);

    Ok(unsafe { CString::from_vec_with_nul_unchecked(contents) })
}

pub(crate) fn find_replace_in_vec<T>(needle: T, replacement: T, data: &mut [T])
where
    T: Copy + Eq,
{
    let mut start = 0;
    while let Some((index, found)) = data[start..]
        .iter_mut()
        .enumerate()
        .find(|&(_index, &mut element)| element == needle)
    {
        *found = replacement;

        start += index + 1;
        if start >= data.len() {
            break;
        }
    }
}

pub(crate) fn read_value<T>(path: impl AsRef<Path>) -> Result<T, LsIpcError>
where
    T: FromStr,
{
    let path = path.as_ref();

    let mut file = File::open(path)
        .map(BufReader::new)
        .map_err(|err| LsIpcError::io1("opening file", path, err))?;

    let mut line = String::default();
    file.read_line(&mut line)
        .map_err(|err| LsIpcError::io1("reading file", path, err))?;

    line.trim()
        .parse()
        .map_err(|_| LsIpcError::io1("invalid data", path, io::ErrorKind::InvalidData))
}

pub(crate) struct UserDbRecordRef(*const libc::passwd);

impl Default for UserDbRecordRef {
    fn default() -> Self {
        Self(ptr::null())
    }
}

impl UserDbRecordRef {
    pub(crate) fn for_id(&mut self, uid: libc::uid_t) -> &Self {
        if unsafe { self.0.as_ref() }.is_none_or(|record| record.pw_uid != uid) {
            self.0 = unsafe { libc::getpwuid(uid) };
        }
        self
    }

    pub(crate) fn name(&self) -> Option<&CStr> {
        unsafe { self.0.as_ref() }
            .and_then(|record| (!record.pw_name.is_null()).then_some(record.pw_name))
            .map(|name| unsafe { CStr::from_ptr(name) })
    }
}

pub(crate) struct GroupDbRecordRef(*const libc::group);

impl Default for GroupDbRecordRef {
    fn default() -> Self {
        Self(ptr::null())
    }
}

impl GroupDbRecordRef {
    pub(crate) fn for_id(&mut self, gid: libc::gid_t) -> &Self {
        if unsafe { self.0.as_ref() }.is_none_or(|record| record.gr_gid != gid) {
            self.0 = unsafe { libc::getgrgid(gid) };
        }
        self
    }

    pub(crate) fn name(&self) -> Option<&CStr> {
        unsafe { self.0.as_ref() }
            .and_then(|record| (!record.gr_name.is_null()).then_some(record.gr_name))
            .map(|name| unsafe { CStr::from_ptr(name) })
    }
}
