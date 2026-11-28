// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) getopt sched SIGPIPE

use clap::builder::ValueParser;
use clap::{crate_version, Arg, ArgAction, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

#[cfg(target_os = "linux")]
mod core;
#[cfg(target_os = "linux")]
mod errors;
#[cfg(target_os = "linux")]
mod output;
mod parse;
#[cfg(target_os = "linux")]
mod policy;
#[cfg(target_os = "linux")]
mod report;
#[cfg(target_os = "linux")]
mod request;
#[cfg(target_os = "linux")]
mod sched;

const ABOUT: &str = help_about!("chrt.md");
const USAGE: &str = help_usage!("chrt.md");

pub(crate) mod options {
    pub const BATCH: &str = "batch";
    pub const DEADLINE: &str = "deadline";
    pub const EXT: &str = "ext";
    pub const FIFO: &str = "fifo";
    pub const IDLE: &str = "idle";
    pub const OTHER: &str = "other";
    pub const RR: &str = "rr";
    pub const RESET_ON_FORK: &str = "reset-on-fork";
    pub const RUNTIME: &str = "sched-runtime";
    pub const PERIOD: &str = "sched-period";
    pub const DEADLINE_PARAMETER: &str = "sched-deadline";
    pub const ALL_TASKS: &str = "all-tasks";
    pub const MAX: &str = "max";
    pub const PID: &str = "pid";
    pub const VERBOSE: &str = "verbose";
    pub const ARGS: &str = "args";
}

#[cfg(target_os = "linux")]
mod linux {
    use uucore::error::UResult;

    use crate::core;
    use crate::options;
    use crate::output::print;
    use crate::policy::Policy;
    use crate::report::max_row;
    use crate::request::Invocation;

    pub(crate) fn run(args: impl uucore::Args) -> UResult<()> {
        // Rust starts with SIGPIPE ignored; chrt(1) and the command it runs die
        // of it. An inherited SIG_IGN is already lost here, so it is always reset.
        uucore::signals::enable_pipe_errors()?;

        let matches = match crate::uu_app().try_get_matches_from(args) {
            Ok(matches) => matches,
            // Help and version go out through `print`, which reports a failed
            // write; uucore panics on one.
            Err(error) if !error.use_stderr() => return Ok(print(&error.to_string())?),
            Err(error) => return Err(error.into()),
        };
        if matches.get_flag(options::MAX) {
            return Ok(print(&max_table())?);
        }
        core::run(&Invocation::from_matches(&matches))
    }

    fn max_table() -> String {
        Policy::ALL
            .iter()
            .map(|&policy| max_row(policy, policy.kernel_bounds()))
            .collect()
    }
}

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    linux::run(args)
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`chrt` is available only on Linux.",
    ))
}

fn flag(id: &'static str, short: char, help: &'static str) -> Arg {
    Arg::new(id)
        .short(short)
        .long(id)
        .help(help)
        .action(ArgAction::SetTrue)
}

/// A value may start with a hyphen, as getopt allows: `-T -0` is a zero
/// runtime, not an unknown option.
fn nanoseconds(id: &'static str, short: char, help: &'static str) -> Arg {
    Arg::new(id)
        .short(short)
        .long(id)
        .help(help)
        .value_name("ns")
        .allow_hyphen_values(true)
        .value_parser(parse::nanoseconds)
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        // getopt lets a flag repeat, and so does chrt(1): `-p -p PID` reads.
        .args_override_self(true)
        .arg(flag(options::BATCH, 'b', "set policy to SCHED_BATCH"))
        .arg(flag(options::DEADLINE, 'd', "set policy to SCHED_DEADLINE"))
        .arg(flag(options::EXT, 'e', "set policy to SCHED_EXT"))
        .arg(flag(options::FIFO, 'f', "set policy to SCHED_FIFO"))
        .arg(flag(options::IDLE, 'i', "set policy to SCHED_IDLE"))
        .arg(flag(options::OTHER, 'o', "set policy to SCHED_OTHER"))
        .arg(flag(options::RR, 'r', "set policy to SCHED_RR (default)"))
        .arg(flag(options::RESET_ON_FORK, 'R', "set reset-on-fork flag"))
        .arg(nanoseconds(
            options::RUNTIME,
            'T',
            "runtime parameter for DEADLINE, or the slice for OTHER and BATCH",
        ))
        .arg(nanoseconds(
            options::PERIOD,
            'P',
            "period parameter for DEADLINE",
        ))
        .arg(nanoseconds(
            options::DEADLINE_PARAMETER,
            'D',
            "deadline parameter for DEADLINE",
        ))
        .arg(flag(
            options::ALL_TASKS,
            'a',
            "operate on all the tasks (threads) for a given pid",
        ))
        .arg(flag(options::MAX, 'm', "show min and max valid priorities"))
        .arg(flag(options::PID, 'p', "operate on existing given pid"))
        .arg(flag(options::VERBOSE, 'v', "display status information"))
        // The usage lines already name the operands, and their meaning
        // depends on --pid, so they are not listed again.
        .arg(
            Arg::new(options::ARGS)
                .value_name("command")
                .hide(true)
                .action(ArgAction::Append)
                .num_args(0..)
                .trailing_var_arg(true)
                .value_parser(ValueParser::os_string()),
        )
}
