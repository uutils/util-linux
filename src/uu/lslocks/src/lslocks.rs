// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.
#![cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, non_camel_case_types, unused_imports)
)]

#[cfg(target_os = "linux")]
mod column;
#[cfg(target_os = "linux")]
mod display;
mod errors;
#[cfg(target_os = "linux")]
mod smartcols;
#[cfg(target_os = "linux")]
mod utils;

use std::borrow::Cow;
use std::ffi::{CStr, CString, OsStr};
use std::io::Write;
use std::path::Path;
use std::ptr;
use std::str::FromStr;

use clap::{Arg, ArgAction, ArgMatches, Command, crate_version, value_parser};
use uucore::{error::UResult, format_usage, help_about, help_usage};

#[cfg(target_os = "linux")]
use crate::column::{ColumnInfo, OutputColumns};
#[cfg(target_os = "linux")]
use crate::display::{describe_holders, describe_integer, describe_size};
use crate::errors::LsLocksError;
#[cfg(target_os = "linux")]
use crate::smartcols::{Table, TableOperations};
#[cfg(target_os = "linux")]
use crate::utils::{
    _PATH_PROC, _PATH_PROC_LOCKS, BinFileLineIter, LockInfo, entry_is_dir_or_unknown,
    proc_pid_command_name,
};
#[cfg(target_os = "linux")]
use libc::pid_t;
#[cfg(not(target_os = "linux"))]
type pid_t = i32;

mod options {
    pub static BYTES: &str = "bytes";
    pub static JSON: &str = "json";
    pub static LIST_COLUMNS: &str = "list-columns";
    pub static LIST: &str = "list";
    pub static NO_HEADINGS: &str = "noheadings";
    pub static NO_INACCESSIBLE: &str = "noinaccessible";
    pub static NO_TRUNCATE: &str = "notruncate";
    pub static OUTPUT_ALL: &str = "output-all";
    pub static OUTPUT: &str = "output";
    pub static PID: &str = "pid";
    pub static RAW: &str = "raw";
}

const ABOUT: &str = help_about!("lslocks.md");
const USAGE: &str = help_usage!("lslocks.md");

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::BYTES)
                .short('b')
                .long(options::BYTES)
                .action(ArgAction::SetTrue)
                .help("print SIZE in bytes rather than in human readable format"),
        )
        .arg(
            Arg::new(options::NO_INACCESSIBLE)
                .short('i')
                .long(options::NO_INACCESSIBLE)
                .action(ArgAction::SetTrue)
                .help("ignore locks without read permissions"),
        )
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long(options::JSON)
                .action(ArgAction::SetTrue)
                .conflicts_with(options::RAW)
                .help("use JSON output format"),
        )
        .arg(
            Arg::new(options::LIST_COLUMNS)
                .short('H')
                .long(options::LIST_COLUMNS)
                .action(ArgAction::SetTrue)
                .conflicts_with_all([
                    options::BYTES,
                    options::NO_INACCESSIBLE,
                    options::NO_HEADINGS,
                    options::OUTPUT,
                    options::OUTPUT_ALL,
                    options::PID,
                    options::NO_TRUNCATE,
                ])
                .help("list the available columns"),
        )
        .arg(
            Arg::new(options::NO_HEADINGS)
                .short('n')
                .long(options::NO_HEADINGS)
                .action(ArgAction::SetTrue)
                .help("don't print headings"),
        )
        .arg(
            Arg::new(options::OUTPUT)
                .short('o')
                .long(options::OUTPUT)
                .value_name(options::LIST)
                .value_parser(OutputColumns::from_str)
                .action(ArgAction::Set)
                .help("output columns (see --list-columns)"),
        )
        .arg(
            Arg::new(options::OUTPUT_ALL)
                .long(options::OUTPUT_ALL)
                .action(ArgAction::SetTrue)
                .help("output all columns"),
        )
        .arg(
            Arg::new(options::PID)
                .short('p')
                .long(options::PID)
                .value_name(options::PID)
                .value_parser(value_parser!(pid_t))
                .action(ArgAction::Set)
                .help("display only locks held by this process"),
        )
        .arg(
            Arg::new(options::NO_TRUNCATE)
                .short('u')
                .long(options::NO_TRUNCATE)
                .action(ArgAction::SetTrue)
                .help("don't truncate text in columns"),
        )
        .arg(
            Arg::new(options::RAW)
                .short('r')
                .long(options::RAW)
                .action(ArgAction::SetTrue)
                .conflicts_with(options::JSON)
                .help("use the raw output format"),
        )
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args = uu_app().try_get_matches_from_mut(args)?;

    let output_mode = OutputMode::from(&args);

    if args.get_flag(options::LIST_COLUMNS) {
        list_columns(output_mode)?;
        return Ok(());
    }

    lslocks(&args, output_mode).map_err(From::from)
}

// The Linux implementation resides in `crate::column`.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone)]
struct OutputColumns;

#[cfg(not(target_os = "linux"))]
impl FromStr for OutputColumns {
    type Err = LsLocksError;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        unimplemented!()
    }
}

#[cfg(target_os = "linux")]
fn lslocks(args: &ArgMatches, output_mode: OutputMode) -> Result<(), LsLocksError> {
    let columns = collect_output_columns(args)?;

    let in_bytes = args.get_flag(options::BYTES);
    let mut table = setup_table(args, output_mode, in_bytes, &columns)?;

    let no_inaccessible = args.get_flag(options::NO_INACCESSIBLE);
    let pid_locks = collect_pid_locks(no_inaccessible)?;

    let path = Path::new(_PATH_PROC_LOCKS);
    let mut lines = BinFileLineIter::open(path)?;

    let mut proc_locks = Vec::default();

    while let Some(line) = lines.next_line()? {
        if let Some(lock) =
            LockInfo::parse(no_inaccessible, path, None, -1, c"", Some(&pid_locks), line)?
        {
            proc_locks.push(lock);
        }
    }

    let target_pid = args.get_one::<pid_t>(options::PID).copied();

    fill_table(
        output_mode,
        &columns,
        target_pid,
        in_bytes,
        &mut table,
        &pid_locks,
        &proc_locks,
    )?;

    table.print()
}

#[cfg(not(target_os = "linux"))]
fn lslocks(_args: &ArgMatches, _output_mode: OutputMode) -> Result<(), LsLocksError> {
    unimplemented!()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum OutputMode {
    None = 0,
    Raw,
    Json,
}

impl From<&'_ ArgMatches> for OutputMode {
    fn from(args: &ArgMatches) -> Self {
        if args.get_flag(options::JSON) {
            Self::Json
        } else if args.get_flag(options::RAW) {
            Self::Raw
        } else {
            Self::None
        }
    }
}

fn list_columns(output_mode: OutputMode) -> Result<(), LsLocksError> {
    let mut stdout = std::io::stdout().lock();

    match output_mode {
        OutputMode::None => stdout.write_all(include_bytes!("../columns.txt")),
        OutputMode::Raw => stdout.write_all(include_bytes!("../columns.raw")),
        OutputMode::Json => stdout.write_all(include_bytes!("../columns.json")),
    }
    .map_err(|err| LsLocksError::io0("stdout", err))
}

#[cfg(target_os = "linux")]
fn collect_output_columns(args: &ArgMatches) -> Result<Vec<&'static ColumnInfo>, LsLocksError> {
    let columns = OutputColumns::from(args);

    let columns = if columns.append {
        let default_names: &[&str] = if args.get_flag(options::OUTPUT_ALL) {
            &column::ALL
        } else {
            &column::DEFAULT
        };

        let mut default_columns = default_names
            .iter()
            .map(|&name| {
                column::COLUMN_INFOS
                    .iter()
                    .find(|&column| column.id.to_str().unwrap() == name)
                    .ok_or_else(|| LsLocksError::InvalidColumnName(name.into()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        default_columns.extend(columns.list);
        default_columns
    } else {
        columns.list
    };

    Ok(columns)
}

#[cfg(target_os = "linux")]
fn setup_table(
    args: &ArgMatches,
    output_mode: OutputMode,
    in_bytes: bool,
    columns: &[&ColumnInfo],
) -> Result<Table, LsLocksError> {
    use smartcols_sys::{
        SCOLS_FL_TRUNC, SCOLS_FL_WRAP, scols_wrapnl_chunksize, scols_wrapnl_nextchunk,
    };

    smartcols::initialize();

    let mut table = Table::new()?;

    if args.get_flag(options::JSON) {
        table.set_name(c"locks")?;
    }

    if args.get_flag(options::NO_HEADINGS) {
        table.enable_headings(false)?;
    }

    match output_mode {
        OutputMode::Raw => table.enable_raw(true)?,
        OutputMode::Json => table.enable_json(true)?,
        OutputMode::None => {}
    }

    let no_truncate = args.get_flag(options::NO_TRUNCATE);

    for &column_info in columns {
        let mut flags = column_info.flags;
        if no_truncate {
            flags &= !SCOLS_FL_TRUNC;
        }
        let mut column = table.new_column(column_info.id, column_info.width_hint, flags)?;

        if (flags & SCOLS_FL_WRAP) != 0 {
            column.set_wrap_func(
                Some(scols_wrapnl_chunksize),
                Some(scols_wrapnl_nextchunk),
                ptr::null_mut(),
            )?;

            column.set_safe_chars(c"\n")?;
        }

        if output_mode == OutputMode::Json {
            column.set_json_type(column_info.json_type(in_bytes))?;
        }
    }

    Ok(table)
}

#[cfg(target_os = "linux")]
fn collect_pid_locks(no_inaccessible: bool) -> Result<Vec<LockInfo>, LsLocksError> {
    let path = Path::new(_PATH_PROC);
    let dir_entries = path
        .read_dir()
        .map_err(|err| LsLocksError::io1("reading directory", path, err))?;

    let mut pid_locks = Vec::default();

    for entry in dir_entries {
        let entry = entry.map_err(|err| LsLocksError::io1("reading directory entry", path, err))?;

        let Some(process_id) = entry.file_name().to_str().and_then(|s| s.parse().ok()) else {
            continue;
        };

        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| LsLocksError::io1("reading directory entry type", &path, err))?;

        if !entry_is_dir_or_unknown(&file_type) {
            continue;
        }

        let command_name = proc_pid_command_name(&path);

        // We should report the error instead of silently continuing.
        let _ignored = append_pid_locks(
            no_inaccessible,
            &path.join("fdinfo"),
            process_id,
            command_name.as_deref().unwrap_or(c""),
            &mut pid_locks,
        );
    }

    Ok(pid_locks)
}

#[cfg(target_os = "linux")]
fn append_pid_locks(
    no_inaccessible: bool,
    path: &Path,
    process_id: pid_t,
    command_name: &CStr,
    pid_locks: &mut Vec<LockInfo>,
) -> Result<(), LsLocksError> {
    let Ok(dir_entries) = path.read_dir() else {
        //eprintln!("{}", LsLocksError::io1("reading directory", &path, err));
        return Ok(()); // We should report the error instead of silently continuing.
    };

    for entry in dir_entries {
        let entry = entry.map_err(|err| LsLocksError::io1("reading directory entry", path, err))?;

        let Some(file_descriptor) = entry.file_name().to_str().and_then(|s| s.parse().ok()) else {
            continue;
        };

        let path = entry.path();
        let mut lines = BinFileLineIter::open(&path)?;

        // This silently ignores potential errors that prevent us from reading the next line.
        // We should report the error instead of silently ignoring it.
        while let Ok(Some(line)) = lines.next_line() {
            let Some(suffix) = line.strip_prefix(b"lock:").map(<[u8]>::trim_ascii) else {
                continue;
            };

            if let Some(lock) = LockInfo::parse(
                no_inaccessible,
                &path,
                Some(process_id),
                file_descriptor,
                command_name,
                None,
                suffix,
            )? {
                pid_locks.push(lock);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn fill_table(
    output_mode: OutputMode,
    columns: &[&ColumnInfo],
    target_pid: Option<pid_t>,
    in_bytes: bool,
    table: &mut Table,
    pid_locks: &[LockInfo],
    proc_locks: &[LockInfo],
) -> Result<(), LsLocksError> {
    use std::os::unix::ffi::OsStrExt;

    for proc_lock in proc_locks
        .iter()
        .rev()
        .filter(|lock| target_pid.is_none_or(|target_pid| lock.process_id == target_pid))
    {
        let mut line = table.new_line(None)?;

        for (cell_index, &column) in columns.iter().enumerate() {
            let data_str = match column.id.to_bytes() {
                b"PID" => describe_integer(proc_lock.process_id),
                b"INODE" => describe_integer(proc_lock.inode),
                b"M" => describe_integer(u8::from(proc_lock.mandatory)),
                b"START" => describe_integer(proc_lock.range.start),
                b"END" => describe_integer(proc_lock.range.end),

                b"SIZE" => proc_lock
                    .size
                    .and_then(|size| describe_size(size, in_bytes)),

                b"TYPE" => Some(Cow::Borrowed(proc_lock.kind.as_c_str())),

                b"MAJ:MIN" => {
                    let major = libc::major(proc_lock.device_id);
                    let minor = libc::minor(proc_lock.device_id);
                    let value = if matches!(output_mode, OutputMode::Json | OutputMode::Raw) {
                        format!("{major}:{minor}")
                    } else {
                        format!("{major:3}:{minor:<3}")
                    };
                    Some(Cow::Owned(CString::new(value).unwrap()))
                }

                b"MODE" => {
                    if proc_lock.blocked {
                        let mut buffer = proc_lock.mode.clone().into_bytes();
                        buffer.push(b'*');
                        Some(Cow::Owned(CString::new(buffer).unwrap()))
                    } else {
                        Some(Cow::Borrowed(proc_lock.mode.as_c_str()))
                    }
                }

                b"COMMAND" => proc_lock.command_name.as_deref().map(Cow::Borrowed),

                b"PATH" => proc_lock
                    .path
                    .as_deref()
                    .map(Path::as_os_str)
                    .map(OsStr::as_bytes)
                    .map(|path| Cow::Owned(CString::new(path).unwrap())),

                b"BLOCKER" => (proc_lock.blocked && proc_lock.id != -1)
                    .then(|| proc_locks.iter())
                    .and_then(|mut iter| {
                        iter.find(|&lock| !lock.blocked && lock.id == proc_lock.id)
                    })
                    .and_then(|found| describe_integer(found.process_id)),

                b"HOLDERS" => describe_holders(proc_lock, pid_locks)
                    .map(Cow::Owned)
                    .map(Some)?,

                _ => continue,
            };

            if let Some(data_str) = data_str {
                line.set_data(cell_index, &data_str)?;
            }
        }
    }
    Ok(())
}
