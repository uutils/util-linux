// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::borrow::Cow;
use std::ffi::{CStr, CString, c_int, c_uint};
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use crate::column::ColumnInfo;
use crate::display::{
    describe_integer, describe_key, describe_owner, describe_permissions, format_time,
    new_global_line,
};
use crate::errors::LsIpcError;
use crate::smartcols::{LineRef, Table, TableOperations};
use crate::utils::{GroupDbRecordRef, UserDbRecordRef, pid_command_line, time_of_day};

const SEMVMX: u64 = 0x7fff;

static _PATH_PROC_SYSV_SEM: &str = "/proc/sysvipc/sem";
static _PATH_PROC_IPC_SEM: &str = "/proc/sys/kernel/sem";

fn semctl(id: c_int, index: c_int, cmd: c_int) -> Result<(c_int, libc::semid_ds), LsIpcError> {
    let mut stat: libc::semid_ds;
    let r = unsafe {
        stat = std::mem::zeroed();
        libc::semctl(id, index, cmd, &mut stat)
    };

    if r == -1 {
        Err(LsIpcError::last_io0("semctl"))
    } else {
        Ok((r, stat))
    }
}

struct Limits {
    sem_vmx: u64,
    sem_mni: u64,
    sem_msl: u64,
    sem_mns: u64,
    sem_opm: u64,
}

impl Limits {
    fn new() -> Result<Self, LsIpcError> {
        if let Ok(file) = File::open(_PATH_PROC_IPC_SEM).map(BufReader::new) {
            Self::from_proc(file)
        } else {
            Self::from_syscall()
        }
    }

    fn from_proc(mut file: BufReader<File>) -> Result<Self, LsIpcError> {
        let mut line = String::default();
        let count = file
            .read_line(&mut line)
            .map_err(|err| LsIpcError::io1("reading file", _PATH_PROC_IPC_SEM, err))?;

        if count == 0 {
            let err = io::ErrorKind::UnexpectedEof;
            Err(LsIpcError::io1("reading file", _PATH_PROC_IPC_SEM, err))
        } else {
            line.parse()
        }
    }

    fn from_syscall() -> Result<Self, LsIpcError> {
        let (_, stat) = semctl(0, 0, libc::IPC_INFO)?;

        let sem_info = unsafe { &*(&stat as *const libc::semid_ds).cast::<libc::seminfo>() };

        let err_map = move |_| LsIpcError::io0("invalid data", io::ErrorKind::InvalidData);

        Ok(Self {
            sem_vmx: u64::try_from(sem_info.semvmx).map_err(err_map)?,
            sem_mni: u64::try_from(sem_info.semmni).map_err(err_map)?,
            sem_msl: u64::try_from(sem_info.semmsl).map_err(err_map)?,
            sem_mns: u64::try_from(sem_info.semmns).map_err(err_map)?,
            sem_opm: u64::try_from(sem_info.semopm).map_err(err_map)?,
        })
    }
}

impl FromStr for Limits {
    type Err = LsIpcError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let err_map = || {
            let err = io::ErrorKind::InvalidData;
            LsIpcError::io1("invalid data", _PATH_PROC_IPC_SEM, err)
        };

        let mut iter = line.split_ascii_whitespace();
        let sem_msl = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let sem_mns = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let sem_opm = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let sem_mni = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;

        Ok(Self {
            sem_vmx: SEMVMX,
            sem_mni,
            sem_msl,
            sem_mns,
            sem_opm,
        })
    }
}

struct SysVIpcEntryElement {
    sem_val: c_int,
    ncount: u64,
    zcount: u64,
    pid: libc::pid_t,
}

impl SysVIpcEntryElement {
    fn new(semid: c_int, index: usize) -> Result<Self, LsIpcError> {
        let (sem_val, _) = semctl(semid, index as c_int, libc::GETVAL)?;
        let (ncount, _) = semctl(semid, index as c_int, libc::GETNCNT)?;
        let (zcount, _) = semctl(semid, index as c_int, libc::GETZCNT)?;
        let (pid, _) = semctl(semid, index as c_int, libc::GETPID)?;

        let err_map = move |_| LsIpcError::io0("invalid data", io::ErrorKind::InvalidData);

        Ok(Self {
            sem_val,
            ncount: u64::try_from(ncount).map_err(err_map)?,
            zcount: u64::try_from(zcount).map_err(err_map)?,
            pid,
        })
    }
}

fn semaphore_elements(semid: c_int, nsems: u64) -> Result<Vec<SysVIpcEntryElement>, LsIpcError> {
    (0..usize::try_from(nsems).unwrap_or(usize::MAX))
        .map(move |index| SysVIpcEntryElement::new(semid, index))
        .collect()
}

struct SysVIpcEntry {
    key: libc::key_t,
    semid: c_int,
    perms: c_uint,
    uid: libc::uid_t,
    gid: libc::gid_t,
    cuid: libc::uid_t,
    cgid: libc::gid_t,
    otime: libc::time_t,
    ctime: libc::time_t,
    elements: Vec<SysVIpcEntryElement>,
}

impl SysVIpcEntry {
    fn from_semid_ds(
        semid: c_int,
        stat: &libc::semid_ds,
        elements: Vec<SysVIpcEntryElement>,
    ) -> Self {
        Self {
            key: stat.sem_perm.__key,
            semid,
            perms: c_uint::from(stat.sem_perm.mode),
            uid: stat.sem_perm.uid,
            gid: stat.sem_perm.gid,
            cuid: stat.sem_perm.cuid,
            cgid: stat.sem_perm.cgid,
            otime: stat.sem_otime,
            ctime: stat.sem_ctime,
            elements,
        }
    }
}

impl FromStr for SysVIpcEntry {
    type Err = LsIpcError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let err_map = || {
            let err = io::ErrorKind::InvalidData;
            LsIpcError::io1("invalid data", _PATH_PROC_SYSV_SEM, err)
        };

        let mut iter = line.split_ascii_whitespace();
        let key = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let semid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let perms = iter
            .next()
            .and_then(|s| c_uint::from_str_radix(s, 8).ok())
            .ok_or_else(err_map)?;
        let nsems = iter
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
        let otime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let ctime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;

        let elements = semaphore_elements(semid, nsems)?;

        Ok(Self {
            key,
            semid,
            perms,
            uid,
            gid,
            cuid,
            cgid,
            otime,
            ctime,
            elements,
        })
    }
}

struct SysVIpc(Vec<SysVIpcEntry>);

impl SysVIpc {
    fn new(id: Option<c_uint>) -> Result<Self, LsIpcError> {
        if let Ok(file) = File::open(_PATH_PROC_SYSV_SEM).map(BufReader::new) {
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
            move |err: std::io::Error| LsIpcError::io1("reading file", _PATH_PROC_SYSV_SEM, err);

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
            move |err: std::io::Error| LsIpcError::io1("reading file", _PATH_PROC_SYSV_SEM, err);

        file.skip_until(b'\n').map_err(err_map)?; // Skip the header line.

        let mut line = String::default();

        loop {
            line.clear();
            if file.read_line(&mut line).map_err(err_map)? == 0 {
                break Ok(Self(Vec::default()));
            }

            let semid: c_uint = line
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| {
                    let err = io::ErrorKind::InvalidData;
                    LsIpcError::io1("invalid data", _PATH_PROC_SYSV_SEM, err)
                })?;

            if semid == id {
                let entry = line.parse()?;
                break Ok(Self(vec![entry]));
            }
        }
    }

    fn from_syscall() -> Result<Self, LsIpcError> {
        let (max_id, _) = semctl(0, 0, libc::SEM_INFO)?;

        let list = (0..max_id)
            .filter_map(|id| semctl(id, 0, libc::SEM_STAT).ok())
            .filter_map(|(semid, stat)| {
                let elements = semaphore_elements(semid, stat.sem_nsems).ok()?;
                Some(SysVIpcEntry::from_semid_ds(semid, &stat, elements))
            })
            .collect();

        Ok(Self(list))
    }

    fn from_syscall_by_id(id: c_uint) -> Result<Self, LsIpcError> {
        let (max_id, _) = semctl(0, 0, libc::SEM_INFO)?;

        let list = (0..max_id)
            .map(|current_id| semctl(current_id, 0, libc::SEM_STAT))
            .filter_map(Result::ok)
            .filter(|(semid, _stat)| c_uint::try_from(*semid).is_ok_and(|semid| semid == id))
            .filter_map(|(semid, stat)| {
                let elements = semaphore_elements(semid, stat.sem_nsems).ok()?;
                Some(SysVIpcEntry::from_semid_ds(semid, &stat, elements))
            })
            .next()
            .map_or_else(Vec::default, |entry| vec![entry]);

        Ok(Self(list))
    }
}

pub(crate) fn print_global(
    _args: &clap::ArgMatches,
    columns: &[&ColumnInfo],
    table: &mut Table,
) -> Result<(), LsIpcError> {
    let limits = Limits::new()?;
    let sys_v_ipc = SysVIpc::new(None)?;
    let total_nsems: usize = sys_v_ipc.0.iter().map(|sem| sem.elements.len()).sum();

    let lines_config = [
        (
            c"SEMMNI",
            c"Number of semaphore identifiers",
            Some(u64::try_from(sys_v_ipc.0.len()).unwrap_or(u64::MAX)),
            limits.sem_mni,
        ),
        (
            c"SEMMNS",
            c"Total number of semaphores",
            Some(u64::try_from(total_nsems).unwrap_or(u64::MAX)),
            limits.sem_mns,
        ),
        (
            c"SEMMSL",
            c"Max semaphores per semaphore set",
            None,
            limits.sem_msl,
        ),
        (
            c"SEMOPM",
            c"Max number of operations per semop(2)",
            None,
            limits.sem_opm,
        ),
        (c"SEMVMX", c"Semaphore max value", None, limits.sem_vmx),
    ];

    lines_config
        .into_iter()
        .try_for_each(move |(resource, description, used, limit)| {
            new_global_line(columns, table, resource, description, used, limit, true)
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

    if let Some(id) = id
        && sys_v_ipc.0.len() != 1
    {
        eprintln!("id {id} not found");
        return Ok(());
    }

    table.set_name(c"semaphores")?;

    let mut users = UserDbRecordRef::default();
    let mut groups = GroupDbRecordRef::default();

    for entry in sys_v_ipc.0 {
        let mut line = table.new_line(None)?;

        for (cell_index, &column) in columns.iter().enumerate() {
            let data_str = match column.id.to_bytes() {
                b"KEY" => describe_key(entry.key),
                b"ID" => describe_integer(entry.semid),
                b"CUID" => describe_integer(entry.cuid),
                b"CGID" => describe_integer(entry.cgid),
                b"UID" => describe_integer(entry.uid),
                b"GID" => describe_integer(entry.gid),
                b"NSEMS" => describe_integer(entry.elements.len()),
                b"CTIME" => format_time(time_format, &now, entry.ctime)?.map(Cow::Owned),
                b"OTIME" => format_time(time_format, &now, entry.otime)?.map(Cow::Owned),
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

        if id.is_some() && !entry.elements.is_empty() {
            describe_elements(&mut line, &entry.elements)?;
        }
    }
    Ok(())
}

fn describe_elements(
    line: &mut LineRef,
    elements: &[SysVIpcEntryElement],
) -> Result<(), LsIpcError> {
    static ELEMENT_COLUMNS: [&CStr; 6] = [
        c"SEMNUM", c"VALUE", c"NCOUNT", c"ZCOUNT", c"PID", c"COMMAND",
    ];

    let mut sub_table = Table::new()?;
    sub_table.set_name(c"elements")?;
    sub_table.enable_headings(true)?;

    ELEMENT_COLUMNS.into_iter().try_for_each(|name| {
        sub_table.new_column(name, 0.0, smartcols_sys::SCOLS_FL_RIGHT)?;
        Ok(())
    })?;

    for (index, element) in elements.iter().enumerate() {
        let mut sub_line = sub_table.new_line(None)?;

        sub_line.set_data(0, &CString::new(index.to_string()).unwrap())?;
        sub_line.set_data(1, &CString::new(element.sem_val.to_string()).unwrap())?;
        sub_line.set_data(2, &CString::new(element.ncount.to_string()).unwrap())?;
        sub_line.set_data(3, &CString::new(element.zcount.to_string()).unwrap())?;
        sub_line.set_data(4, &CString::new(element.pid.to_string()).unwrap())?;

        if let Ok(cmd_line) = pid_command_line(element.pid) {
            sub_line.set_data(5, &cmd_line)?;
        }
    }

    line.set_user_data(sub_table.into_inner().as_ptr().cast())
}
