// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::borrow::Cow;
use std::ffi::{c_int, c_uint};
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
use crate::utils::{GroupDbRecordRef, UserDbRecordRef, read_value, time_of_day};

static _PATH_PROC_SYSV_MSG: &str = "/proc/sysvipc/msg";
static _PATH_PROC_IPC_MSGMNI: &str = "/proc/sys/kernel/msgmni";
static _PATH_PROC_IPC_MSGMNB: &str = "/proc/sys/kernel/msgmnb";
static _PATH_PROC_IPC_MSGMAX: &str = "/proc/sys/kernel/msgmax";

fn msgctl(id: c_int, cmd: c_int) -> Result<(c_int, libc::msqid_ds), LsIpcError> {
    let mut stat: libc::msqid_ds;
    let r = unsafe {
        stat = std::mem::zeroed();
        libc::msgctl(id, cmd, &mut stat)
    };

    if r == -1 {
        Err(LsIpcError::last_io0("msgctl"))
    } else {
        Ok((r, stat))
    }
}

struct Limits {
    msg_mni: u64,
    msg_mnb: u64,
    msg_max: u64,
}

impl Limits {
    fn new() -> Result<Self, LsIpcError> {
        Self::from_proc().or_else(|_| Self::from_syscall())
    }

    fn from_proc() -> Result<Self, LsIpcError> {
        Ok(Self {
            msg_mni: read_value::<u64>(_PATH_PROC_IPC_MSGMNI)?,
            msg_mnb: read_value::<u64>(_PATH_PROC_IPC_MSGMNB)?,
            msg_max: read_value::<u64>(_PATH_PROC_IPC_MSGMAX)?,
        })
    }

    fn from_syscall() -> Result<Self, LsIpcError> {
        let (_, stat) = msgctl(0, libc::IPC_INFO)?;

        let msg_info = unsafe { &*(&stat as *const libc::msqid_ds).cast::<libc::msginfo>() };

        let err_map = move |_| LsIpcError::io0("invalid data", io::ErrorKind::InvalidData);

        Ok(Self {
            msg_mni: u64::try_from(msg_info.msgmni).map_err(err_map)?,
            msg_mnb: u64::try_from(msg_info.msgmnb).map_err(err_map)?,
            msg_max: u64::try_from(msg_info.msgmax).map_err(err_map)?,
        })
    }
}

struct SysVIpcEntry {
    key: libc::key_t,
    msqid: c_int,
    perms: c_uint,
    cbytes: u64,
    qnum: libc::msgqnum_t,
    lspid: libc::pid_t,
    lrpid: libc::pid_t,
    uid: libc::uid_t,
    gid: libc::gid_t,
    cuid: libc::uid_t,
    cgid: libc::gid_t,
    stime: libc::time_t,
    rtime: libc::time_t,
    ctime: libc::time_t,
}

impl SysVIpcEntry {
    fn from_msqid_ds(msqid: c_int, stat: &libc::msqid_ds) -> Self {
        Self {
            key: stat.msg_perm.__key,
            msqid,
            perms: c_uint::from(stat.msg_perm.mode),
            cbytes: stat.__msg_cbytes,
            qnum: stat.msg_qnum,
            lspid: stat.msg_lspid,
            lrpid: stat.msg_lrpid,
            uid: stat.msg_perm.uid,
            gid: stat.msg_perm.gid,
            cuid: stat.msg_perm.cuid,
            cgid: stat.msg_perm.cgid,
            stime: stat.msg_stime,
            rtime: stat.msg_rtime,
            ctime: stat.msg_ctime,
        }
    }
}

impl FromStr for SysVIpcEntry {
    type Err = LsIpcError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let err_map = || {
            let err = io::ErrorKind::InvalidData;
            LsIpcError::io1("invalid data", _PATH_PROC_SYSV_MSG, err)
        };

        let mut iter = line.split_ascii_whitespace();
        let key = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let msqid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let perms = iter
            .next()
            .and_then(|s| c_uint::from_str_radix(s, 8).ok())
            .ok_or_else(err_map)?;
        let cbytes = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let qnum = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let lspid = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let lrpid = iter
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
        let stime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let rtime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;
        let ctime = iter
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(err_map)?;

        Ok(Self {
            key,
            msqid,
            perms,
            cbytes,
            qnum,
            lspid,
            lrpid,
            uid,
            gid,
            cuid,
            cgid,
            stime,
            rtime,
            ctime,
        })
    }
}

struct SysVIpc(Vec<SysVIpcEntry>);

impl SysVIpc {
    fn new(id: Option<c_uint>) -> Result<Self, LsIpcError> {
        if let Ok(file) = File::open(_PATH_PROC_SYSV_MSG).map(BufReader::new) {
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
            move |err: std::io::Error| LsIpcError::io1("reading file", _PATH_PROC_SYSV_MSG, err);

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
            move |err: std::io::Error| LsIpcError::io1("reading file", _PATH_PROC_SYSV_MSG, err);

        file.skip_until(b'\n').map_err(err_map)?; // Skip the header line.

        let mut line = String::default();

        loop {
            line.clear();
            if file.read_line(&mut line).map_err(err_map)? == 0 {
                break Ok(Self(Vec::default()));
            }

            let msqid: c_uint = line
                .split_ascii_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| {
                    let err = io::ErrorKind::InvalidData;
                    LsIpcError::io1("invalid data", _PATH_PROC_SYSV_MSG, err)
                })?;

            if msqid == id {
                let entry = line.parse()?;
                break Ok(Self(vec![entry]));
            }
        }
    }

    fn from_syscall() -> Result<Self, LsIpcError> {
        let (max_id, _) = msgctl(0, libc::MSG_INFO)?;

        let list = (0..max_id)
            .map(|id| msgctl(id, libc::MSG_STAT))
            .filter_map(Result::ok)
            .map(|(msqid, stat)| SysVIpcEntry::from_msqid_ds(msqid, &stat))
            .collect();

        Ok(Self(list))
    }

    fn from_syscall_by_id(id: c_uint) -> Result<Self, LsIpcError> {
        let (max_id, _) = msgctl(0, libc::MSG_INFO)?;

        let list = (0..max_id)
            .map(|current_id| msgctl(current_id, libc::MSG_STAT))
            .filter_map(Result::ok)
            .filter(|(msqid, _stat)| c_uint::try_from(*msqid).is_ok_and(|msqid| msqid == id))
            .map(|(msqid, stat)| SysVIpcEntry::from_msqid_ds(msqid, &stat))
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
    let count = SysVIpc::new(None)?.0.len();
    let in_bytes = args.get_flag(crate::options::BYTES);

    let lines_config = [
        (
            c"MSGMNI",
            c"Number of message queues",
            Some(u64::try_from(count).unwrap_or(u64::MAX)),
            limits.msg_mni,
            true,
        ),
        (
            c"MSGMAX",
            c"Max size of message (bytes)",
            None,
            limits.msg_max,
            in_bytes,
        ),
        (
            c"MSGMNB",
            c"Default max size of queue (bytes)",
            None,
            limits.msg_mnb,
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

    table.set_name(c"messages")?;

    let mut users = UserDbRecordRef::default();
    let mut groups = GroupDbRecordRef::default();

    for entry in sys_v_ipc.0 {
        let mut line = table.new_line(None)?;

        for (cell_index, &column) in columns.iter().enumerate() {
            let data_str = match column.id.to_bytes() {
                b"KEY" => describe_key(entry.key),
                b"ID" => describe_integer(entry.msqid),
                b"CUID" => describe_integer(entry.cuid),
                b"CGID" => describe_integer(entry.cgid),
                b"UID" => describe_integer(entry.uid),
                b"GID" => describe_integer(entry.gid),
                b"LSPID" => describe_integer(entry.lspid),
                b"LRPID" => describe_integer(entry.lrpid),
                b"MSGS" => describe_integer(entry.qnum),
                b"CTIME" => format_time(time_format, &now, entry.ctime)?.map(Cow::Owned),
                b"SEND" => format_time(time_format, &now, entry.stime)?.map(Cow::Owned),
                b"RECV" => format_time(time_format, &now, entry.rtime)?.map(Cow::Owned),
                b"USEDBYTES" => describe_size(entry.cbytes, args.get_flag(crate::options::BYTES)),
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
