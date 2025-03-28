// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod column;
#[cfg(target_os = "linux")]
mod display;
mod errors;
#[cfg(target_os = "linux")]
mod message_queue;
#[cfg(target_os = "linux")]
mod semaphore;
#[cfg(target_os = "linux")]
mod shared_memory;
#[cfg(target_os = "linux")]
mod smartcols;
#[cfg(target_os = "linux")]
mod utils;

use std::ffi::c_uint;
use std::str::FromStr;

use clap::builder::{EnumValueParser, PossibleValue};
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command, ValueEnum, crate_version, value_parser};
use uucore::{error::UResult, format_usage, help_about, help_usage};

#[cfg(target_os = "linux")]
use crate::column::{ColumnInfo, OutputColumns};
use crate::errors::LsIpcError;
#[cfg(target_os = "linux")]
use crate::smartcols::TableOperations;

mod options {
    pub static BYTES: &str = "bytes";
    pub static CREATOR: &str = "creator";
    pub static EXPORT: &str = "export";
    pub static GLOBAL: &str = "global";
    pub static ID: &str = "id";
    pub static JSON: &str = "json";
    pub static LIST: &str = "list";
    pub static NEW_LINE: &str = "newline";
    pub static NO_HEADINGS: &str = "noheadings";
    pub static NO_TRUNCATE: &str = "notruncate";
    pub static NUMERIC_PERMS: &str = "numeric-perms";
    pub static OUTPUT: &str = "output";
    pub static QUEUES: &str = "queues";
    pub static RAW: &str = "raw";
    pub static SEMAPHORES: &str = "semaphores";
    pub static SHELL: &str = "shell";
    pub static SHMEMS: &str = "shmems";
    pub static TIME_FORMAT: &str = "time-format";
    pub static TIME: &str = "time";
    pub static TYPE: &str = "type";
    pub const SHORT: &str = "short";
    pub const FULL: &str = "full";
    pub const ISO: &str = "iso";
}

const ABOUT: &str = help_about!("lsipc.md");
const USAGE: &str = help_usage!("lsipc.md");

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
            Arg::new(options::CREATOR)
                .short('c')
                .long(options::CREATOR)
                .action(ArgAction::SetTrue)
                .requires("ipc-kind")
                .help("show creator and owner"),
        )
        .arg(
            Arg::new(options::EXPORT)
                .short('e')
                .long(options::EXPORT)
                .action(ArgAction::SetTrue)
                .help("display in an export-able output format"),
        )
        .arg(
            Arg::new(options::GLOBAL)
                .short('g')
                .long(options::GLOBAL)
                .action(ArgAction::SetTrue)
                .conflicts_with(options::ID)
                .help("info about system-wide usage"),
        )
        .arg(
            Arg::new(options::ID)
                .short('i')
                .long(options::ID)
                .value_name(options::ID)
                .value_parser(value_parser!(c_uint))
                .action(ArgAction::Set)
                .conflicts_with(options::GLOBAL)
                .requires("ipc-kind")
                .help("print details on resource identified by id"),
        )
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long(options::JSON)
                .action(ArgAction::SetTrue)
                .help("use the JSON output format"),
        )
        .arg(
            Arg::new(options::LIST)
                .short('l')
                .long(options::LIST)
                .action(ArgAction::SetTrue)
                .help("force list output format"),
        )
        .arg(
            Arg::new(options::NEW_LINE)
                .short('n')
                .long(options::NEW_LINE)
                .action(ArgAction::SetTrue)
                .help("display each piece of information on a new line"),
        )
        .arg(
            Arg::new(options::NO_HEADINGS)
                .long(options::NO_HEADINGS)
                .action(ArgAction::SetTrue)
                .help("don't print headings"),
        )
        .arg(
            Arg::new(options::NO_TRUNCATE)
                .long(options::NO_TRUNCATE)
                .action(ArgAction::SetTrue)
                .help("don't truncate output"),
        )
        .arg(
            Arg::new(options::NUMERIC_PERMS)
                .short('P')
                .long(options::NUMERIC_PERMS)
                .action(ArgAction::SetTrue)
                .help("print numeric permissions"),
        )
        .arg(
            Arg::new(options::OUTPUT)
                .short('o')
                .long(options::OUTPUT)
                .value_name(options::LIST)
                .value_parser(OutputColumns::from_str)
                .action(ArgAction::Set)
                .help("define the columns to output"),
        )
        .arg(
            Arg::new(options::QUEUES)
                .short('q')
                .long(options::QUEUES)
                .action(ArgAction::SetTrue)
                .help("message queues"),
        )
        .arg(
            Arg::new(options::RAW)
                .short('r')
                .long(options::RAW)
                .action(ArgAction::SetTrue)
                .help("display in raw mode"),
        )
        .arg(
            Arg::new(options::SEMAPHORES)
                .short('s')
                .long(options::SEMAPHORES)
                .action(ArgAction::SetTrue)
                .help("semaphores"),
        )
        .arg(
            Arg::new(options::SHELL)
                .short('y')
                .long(options::SHELL)
                .action(ArgAction::SetTrue)
                .help("use column names to be usable as shell variable identifiers"),
        )
        .arg(
            Arg::new(options::SHMEMS)
                .short('m')
                .long(options::SHMEMS)
                .action(ArgAction::SetTrue)
                .help("shared memory segments"),
        )
        .arg(
            Arg::new(options::TIME_FORMAT)
                .long(options::TIME_FORMAT)
                .value_name(options::TYPE)
                .value_parser(EnumValueParser::<TimeFormat>::new())
                .action(ArgAction::Set)
                .help("display dates in short, full or iso format"),
        )
        .arg(
            Arg::new(options::TIME)
                .short('t')
                .long(options::TIME)
                .action(ArgAction::SetTrue)
                .requires("ipc-kind")
                .help("show attach, detach and change times"),
        )
        .group(
            ArgGroup::new("ipc-kind")
                .args([options::SHMEMS, options::QUEUES, options::SEMAPHORES])
                .multiple(false),
        )
        .group(
            ArgGroup::new("output-kind")
                .args([
                    options::EXPORT,
                    options::JSON,
                    options::LIST,
                    options::NEW_LINE,
                    options::RAW,
                ])
                .multiple(false),
        )
        .after_help(include_str!("../after-help.txt"))
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args = uu_app().try_get_matches_from_mut(args)?;

    let output_mode = OutputMode::from(&args);

    let time_format = args
        .get_one::<TimeFormat>(options::TIME_FORMAT)
        .copied()
        .unwrap_or(if output_mode == OutputMode::Pretty {
            TimeFormat::Full
        } else {
            TimeFormat::Short
        });

    lsipc(&args, output_mode, time_format).map_err(From::from)
}

// The Linux implementation resides in `crate::column`.
#[cfg(not(target_os = "linux"))]
#[derive(Debug, Clone)]
struct OutputColumns;

#[cfg(not(target_os = "linux"))]
impl FromStr for OutputColumns {
    type Err = LsIpcError;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        unimplemented!()
    }
}

#[cfg(target_os = "linux")]
fn lsipc(
    args: &ArgMatches,
    output_mode: OutputMode,
    time_format: TimeFormat,
) -> Result<(), LsIpcError> {
    let columns = OutputColumns::from(args);

    let columns = if columns.append {
        let mut default_columns = if output_mode == OutputMode::Pretty
            && !args.get_flag(options::CREATOR)
            && !args.get_flag(options::TIME)
        {
            column::all_defaults(args)?
        } else {
            column::filter_defaults(args)?
        };

        default_columns.extend(columns.list);
        default_columns
    } else {
        columns.list
    };

    smartcols::initialize();

    let mut table = display::new_table(args, output_mode)?;

    let no_truncate = args.get_flag(options::NO_TRUNCATE);
    for &column in &columns {
        let mut flags = column.flags;
        if no_truncate {
            flags &= !smartcols_sys::SCOLS_FL_TRUNC;
        }
        table.new_column(column.id, column.width_hint, flags)?;
    }

    let print_global = args.get_flag(options::GLOBAL);

    if print_global {
        table.set_name(c"ipclimits")?;
    }

    type GlobalProc =
        fn(&ArgMatches, &[&ColumnInfo], &mut smartcols::Table) -> Result<(), LsIpcError>;

    type DescribeProc = fn(
        &ArgMatches,
        TimeFormat,
        &[&ColumnInfo],
        &mut smartcols::Table,
        Option<c_uint>,
    ) -> Result<(), LsIpcError>;

    let queues = args.get_flag(options::QUEUES);
    let shmems = args.get_flag(options::SHMEMS);
    let semaphores = args.get_flag(options::SEMAPHORES);

    let config_list: [(bool, GlobalProc, DescribeProc); 3] = [
        (queues, message_queue::print_global, message_queue::describe),
        (shmems, shared_memory::print_global, shared_memory::describe),
        (semaphores, semaphore::print_global, semaphore::describe),
    ];

    let id = args.get_one::<c_uint>(options::ID).copied();

    for (flag, global, describe) in config_list {
        if flag || (!queues && !shmems && !semaphores) {
            if print_global {
                global(args, &columns, &mut table)?;
            } else {
                describe(args, time_format, &columns, &mut table, id)?;
            }
        }
    }

    if output_mode == OutputMode::Pretty {
        display::print_pretty_table(&table)?;
    } else {
        table.print()?;
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn lsipc(
    _args: &ArgMatches,
    _output_mode: OutputMode,
    _time_format: TimeFormat,
) -> Result<(), LsIpcError> {
    unimplemented!()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum TimeFormat {
    Short = 0,
    Full,
    Iso,
}

impl FromStr for TimeFormat {
    type Err = LsIpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            options::SHORT => Ok(Self::Short),
            options::FULL => Ok(Self::Full),
            options::ISO => Ok(Self::Iso),
            _ => Err(LsIpcError::InvalidTimeFormat(s.into())),
        }
    }
}

impl ValueEnum for TimeFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Short, Self::Full, Self::Iso]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        let name = match self {
            Self::Short => options::SHORT,
            Self::Full => options::FULL,
            Self::Iso => options::ISO,
        };
        Some(PossibleValue::new(name))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum OutputMode {
    None = 0,
    Export,
    NewLine,
    Raw,
    Json,
    Pretty,
    List,
}

impl From<&'_ ArgMatches> for OutputMode {
    fn from(args: &'_ ArgMatches) -> Self {
        if args.get_flag(options::EXPORT) {
            Self::Export
        } else if args.get_flag(options::JSON) {
            Self::Json
        } else if args.get_flag(options::LIST) {
            Self::List
        } else if args.get_flag(options::NEW_LINE) {
            Self::NewLine
        } else if args.get_flag(options::RAW) {
            Self::Raw
        } else if args.contains_id(options::ID) {
            Self::Pretty
        } else {
            Self::None
        }
    }
}
