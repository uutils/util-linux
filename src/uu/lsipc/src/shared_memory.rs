// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::borrow::Cow;
use std::ffi::{CString, c_int, c_uint, c_ulong};
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use crate::column::ColumnInfo;
use crate::display::{
    describe_integer, describe_key, describe_owner, describe_permissions, describe_size,
    format_time, new_global_line,
};
use crate::errors::LsIpcError;
use crate::smartcols::{Table, TableOperations};
use crate::utils::{
    GroupDbRecordRef, UserDbRecordRef, get_page_size, pid_command_line, read_value, time_of_day,
};

const SHM_STAT: c_int = 13 | (libc::IPC_STAT & 0x100);
const SHM_INFO: c_int = 14;
const SHM_DEST: c_uint = 0o1000;
const SHM_LOCKED: c_uint = 0o2000;

static _PATH_PROC_SYSV_SHM: &str = "/proc/sysvipc/shm";
static _PATH_PROC_IPC_SHMMAX: &str = "/proc/sys/kernel/shmmax";
static _PATH_PROC_IPC_SHMMNI: &str = "/proc/sys/kernel/shmmni";
static _PATH_PROC_IPC_SHMALL: &str = "/proc/sys/kernel/shmall";

// The C struct is defined in <sys/shm.h>
#[allow(non_camel_case_types)]
#[repr(C)]
struct shminfo {
    shmmax: c_ulong,
    shmmin: c_ulong,
    shmmni: c_ulong,
    shmseg: c_ulong,
    shmall: c_ulong,
    __unused: [c_ulong; 4],
}

fn shmctl(id: c_int, cmd: c_int) -> Result<(c_int, libc::shmid_ds), LsIpcError> {
    let mut stat: libc::shmid_ds;
    let r = unsafe {
        stat = std::mem::zeroed();
        libc::shmctl(id, cmd, &mut stat)
    };

    if r == -1 {
        Err(LsIpcError::last_io0("shmctl"))
    } else {
        Ok((r, stat))
    }
}

struct Limits {
    shm_max: u64,
    shm_min: u64,
    shm_mni: u64,
    shm_all: u64,
}

impl Limits {
    fn new() -> Result<Self, LsIpcError> {
        Self::from_proc().or_else(|_| Self::from_syscall())
    }

    fn from_proc() -> Result<Self, LsIpcError> {
        Ok(Self {
            shm_max: read_value::<u64>(_PATH_PROC_IPC_SHMMAX)?,
            shm_min: 1,
            shm_mni: read_value::<u64>(_PATH_PROC_IPC_SHMMNI)?,
            shm_all: read_value::<u64>(_PATH_PROC_IPC_SHMALL)?,
        })
    }

    fn from_syscall() -> Result<Self, LsIpcError> {
        let (_, stat) = shmctl(0, libc::IPC_INFO)?;

        let shm_info = unsafe { &*(&stat as *const libc::shmid_ds).cast::<shminfo>() };

        Ok(Self {
            shm_max: shm_info.shmmax,
            shm_min: shm_info.shmmin,
            shm_mni: shm_info.shmmni,
            shm_all: shm_info.shmall,
        })
    }
}

struct SysVIpcEntry {
    key: libc::key_t,
    shmid: c_int,
    perms: c_uint,
    segsz: u64,
    cpid: libc::pid_t,
    lpid: libc::pid_t,
    nattch: u64,
    uid: libc::uid_t,
    gid: libc::gid_t,
    cuid: libc::uid_t,
    cgid: libc::gid_t,
    atime: libc::time_t,
    dtime: libc::time_t,
    ctime: libc::time_t,
}

impl SysVIpcEntry {
    fn from_shmid_ds(shmid: c_int, stat: &libc::shmid_ds) -> Self {
        Self {
            key: stat.shm_perm.__key,
            shmid,
            perms: c_uint::from(stat.shm_perm.mode),
            segsz: u64::try_from(stat.shm_segsz).unwrap_or(u64::MAX),
            cpid: stat.shm_cpid,
            lpid: stat.shm_lpid,
            nattch: stat.shm_nattch,
            uid: stat.shm_perm.uid,
            gid: stat.shm_perm.gid,
            cuid: stat.shm_perm.cuid,
            cgid: stat.shm_perm.cgid,
            atime: stat.shm_atime,
            dtime: stat.shm_dtime,
            ctime: stat.shm_ctime,
        }
    }

    fn status_desc(&self) -> String {
        static STATUS_MAP: [(c_uint, &str); 4] = [
            (SHM_DEST, "dest"),
            (SHM_LOCKED, "locked"),
            (libc::SHM_HUGETLB as c_uint, "hugetlb"),
            (libc::SHM_NORESERVE as c_uint, "noreserve"),
        ];

        let mut separator = "";
        let mut result = String::default();

        for (flag, name) in STATUS_MAP {
            if (self.perms & flag) != 0 {
                result.push_str(separator);
                result.push_str(name);
                separator = ",";
            }
        }

        result
    }
}

impl FromStr for SysVIpcEntry {
    type Err = LsIpcError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let err_map = || {
            let err = io::ErrorKind::InvalidData;
            LsIpcError::io1("invalid data", _PATH_PROC_SYSV_SHM, err)
        };

        let mut iter = line.split_ascii_whitespace();
        let key = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let shmid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let perms = iter
            .next()
            .and_then(|s| c_uint::from_str_radix(s, 8).ok())
            .ok_or_else(err_map)?;
        let segsz = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let cpid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let lpid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let nattch = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let uid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let gid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let cuid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let cgid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let atime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let dtime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let ctime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;

        Ok(Self {
            key,
            shmid,
            perms,
            segsz,
            cpid,
            lpid,
            nattch,
            uid,
            gid,
            cuid,
            cgid,
            atime,
            dtime,
            ctime,
        })
    }
}

struct SysVIpc(Vec<SysVIpcEntry>);

impl SysVIpc {
    fn new(id: Option<c_uint>) -> Result<Self, LsIpcError> {
        if let Ok(file) = File::open(_PATH_PROC_SYSV_SHM).map(BufReader::new) {
            if let Some(id) = id {
                Self::from_proc_by_id(id, file)
            } else {
                Self::from_proc(file)
            }
        } else if let Some(id) = id {
            Self::from_syscall_by_id(id)
        } else {
            Self::from_syscall()
        }
    }

    fn from_proc(mut file: BufReader<File>) -> Result<Self, LsIpcError> {
        let err_map =
            move |err: std::io::Error| LsIpcError::io1("reading file", _PATH_PROC_SYSV_SHM, err);

        file.skip_until(b'\n').map_err(err_map)?; // Skip the header line.

        let mut list = Vec::default();
        let mut line = String::default();

        loop {
            line.clear();
            if file.read_line(&mut line).map_err(err_map)? == 0 {
                break;
            }

            let entry = line.parse()?;
            list.push(entry);
        }
        Ok(Self(list))
    }

    fn from_proc_by_id(id: c_uint, mut file: BufReader<File>) -> Result<Self, LsIpcError> {
        let err_map =
            move |err: std::io::Error| LsIpcError::io1("reading file", _PATH_PROC_SYSV_SHM, err);

        file.skip_until(b'\n').map_err(err_map)?; // Skip the header line.

        let mut line = String::default();

        loop {
            line.clear();
            if file.read_line(&mut line).map_err(err_map)? == 0 {
                break Ok(Self(Vec::default()));
            }

            let shmid: c_uint = line
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| {
                    let err = io::ErrorKind::InvalidData;
                    LsIpcError::io1("invalid data", _PATH_PROC_SYSV_SHM, err)
                })?;

            if shmid == id {
                let entry = line.parse()?;
                break Ok(Self(vec![entry]));
            }
        }
    }

    fn from_syscall() -> Result<Self, LsIpcError> {
        let (max_id, _) = shmctl(0, SHM_INFO)?;

        let list = (0..max_id)
            .map(|id| shmctl(id, SHM_STAT))
            .filter_map(Result::ok)
            .map(|(shmid, stat)| SysVIpcEntry::from_shmid_ds(shmid, &stat))
            .collect();

        Ok(Self(list))
    }

    fn from_syscall_by_id(id: c_uint) -> Result<Self, LsIpcError> {
        let (max_id, _) = shmctl(0, SHM_INFO)?;

        let list = (0..max_id)
            .map(|current_id| shmctl(current_id, SHM_STAT))
            .filter_map(Result::ok)
            .filter(|(shmid, _stat)| c_uint::try_from(*shmid).is_ok_and(|shmid| shmid == id))
            .map(|(shmid, stat)| SysVIpcEntry::from_shmid_ds(shmid, &stat))
            .next()
            .map_or_else(Vec::default, |entry| vec![entry]);

        Ok(Self(list))
    }
}

pub(crate) fn print_global(
    args: &clap::ArgMatches,
    columns: &[&ColumnInfo],
    table: &mut Table,
) -> Result<(), LsIpcError> {
    let limits = Limits::new()?;
    let sys_v_ipc = SysVIpc::new(None)?;
    let segsz_pages = sys_v_ipc.0.iter().map(|shm| shm.segsz).sum::<u64>() / get_page_size()?;
    let in_bytes = args.get_flag(crate::options::BYTES);

    let lines_config = [
        (
            c"SHMMNI",
            c"Shared memory segments",
            Some(u64::try_from(sys_v_ipc.0.len()).unwrap_or(u64::MAX)),
            limits.shm_mni,
            true,
        ),
        (
            c"SHMALL",
            c"Shared memory pages",
            Some(segsz_pages),
            limits.shm_all,
            true,
        ),
        (
            c"SHMMAX",
            c"Max size of shared memory segment (bytes)",
            None,
            limits.shm_max,
            in_bytes,
        ),
        (
            c"SHMMIN",
            c"Min size of shared memory segment (bytes)",
            None,
            limits.shm_min,
            in_bytes,
        ),
    ];

    lines_config
        .into_iter()
        .try_for_each(move |(resource, description, used, limit, in_bytes)| {
            new_global_line(columns, table, resource, description, used, limit, in_bytes)
        })
}

pub(crate) fn describe(
    args: &clap::ArgMatches,
    time_format: crate::TimeFormat,
    columns: &[&ColumnInfo],
    table: &mut Table,
    id: Option<c_uint>,
) -> Result<(), LsIpcError> {
    let now = time_of_day()?;
    let sys_v_ipc = SysVIpc::new(id)?;

    if let Some(id) = id {
        if sys_v_ipc.0.len() != 1 {
            eprintln!("id {id} not found");
            return Ok(());
        }
    }

    table.set_name(c"sharedmemory")?;

    let mut users = UserDbRecordRef::default();
    let mut groups = GroupDbRecordRef::default();

    for entry in sys_v_ipc.0 {
        let mut line = table.new_line(None)?;

        for (cell_index, &column) in columns.iter().enumerate() {
            let data_str = match column.id.to_bytes() {
                b"KEY" => describe_key(entry.key),
                b"ID" => describe_integer(entry.shmid),
                b"CUID" => describe_integer(entry.cuid),
                b"CGID" => describe_integer(entry.cgid),
                b"UID" => describe_integer(entry.uid),
                b"GID" => describe_integer(entry.gid),
                b"NATTCH" => describe_integer(entry.nattch),
                b"CPID" => describe_integer(entry.cpid),
                b"LPID" => describe_integer(entry.lpid),
                b"CTIME" => format_time(time_format, &now, entry.ctime)?.map(Cow::Owned),
                b"ATTACH" => format_time(time_format, &now, entry.atime)?.map(Cow::Owned),
                b"DETACH" => format_time(time_format, &now, entry.dtime)?.map(Cow::Owned),
                b"COMMAND" => pid_command_line(entry.cpid).map(Cow::Owned).map(Some)?,
                b"STATUS" => Some(Cow::Owned(CString::new(entry.status_desc()).unwrap())),
                b"SIZE" => describe_size(entry.segsz, args.get_flag(crate::options::BYTES)),
                b"OWNER" => describe_owner(&mut users, entry.uid),
                b"CUSER" => users.for_id(entry.cuid).name().map(Cow::Borrowed),
                b"USER" => users.for_id(entry.uid).name().map(Cow::Borrowed),
                b"CGROUP" => groups.for_id(entry.cgid).name().map(Cow::Borrowed),
                b"GROUP" => groups.for_id(entry.gid).name().map(Cow::Borrowed),
                b"PERMS" => describe_permissions(args, entry.perms & 0o777),

                _ => continue,
            };

            if let Some(data_str) = data_str {
                line.set_data(cell_index, &data_str)?;
            }
        }
    }
    Ok(())
}
