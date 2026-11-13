// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{builder::ValueParser, crate_version, value_parser, Arg, ArgAction, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("flock.md");
const USAGE: &str = help_usage!("flock.md");

mod options {
    pub const SHARED: &str = "shared";
    pub const TIMEOUT: &str = "timeout";
    pub const EXCLUSIVE: &str = "exclusive";
    pub const UNLOCK: &str = "unlock";
    pub const NONBLOCK: &str = "nonblock";
    pub const CONFLICT_EXIT_CODE: &str = "conflict-exit-code";
    pub const COMMAND: &str = "command";

    // Positional arguments
    pub const FILE_OR_FD: &str = "file_or_fd";
    pub const PROG_ARGS: &str = "prog_args";
}

#[cfg(unix)]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;
    let _shared: bool = matches.get_flag(options::SHARED);
    let _timeout: Option<f64> = matches.get_one::<f64>(options::TIMEOUT).copied();
    let _exclusive: bool = matches.get_flag(options::EXCLUSIVE);
    // TODO: Logic
    Ok(())
}

#[cfg(not(unix))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;
    Err(uucore::error::USimpleError::new(
        1,
        "`flock` is unavailable on this platform",
    ))
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::SHARED)
                .short('s')
                .long("shared")
                .help("get a shared lock")
                .overrides_with_all([options::SHARED, options::EXCLUSIVE, options::UNLOCK])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::EXCLUSIVE)
                .short('x')
                .short_alias('e')
                .long("exclusive")
                .help("get an exclusive lock (default)")
                .overrides_with_all([options::SHARED, options::EXCLUSIVE, options::UNLOCK])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::UNLOCK)
                .short('u')
                .long("unlock")
                .help("remove a lock")
                .overrides_with_all([options::SHARED, options::EXCLUSIVE, options::UNLOCK])
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::NONBLOCK)
                .short('n')
                .long("nonblocking")
                .visible_alias("nb")
                .help("fail rather than wait")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::TIMEOUT)
                .short('w')
                .long("timeout")
                .alias("wait")
                .value_name("secs")
                .help("wait for a limited amount of time")
                .value_parser(value_parser!(f64)),
        )
        .arg(
            Arg::new(options::CONFLICT_EXIT_CODE)
                .short('E')
                .long("conflict-exit-code")
                .value_name("number")
                .help("exit code after conflict or timeout")
                .default_value("1")
                .value_parser(value_parser!(u8)),
        )
        .arg(
            Arg::new(options::COMMAND)
                .short('c')
                .long("command")
                .value_name("command")
                .help("run a single command through the shell")
                .conflicts_with(options::PROG_ARGS)
                .value_parser(ValueParser::os_string()),
        )
        .arg(
            Arg::new(options::FILE_OR_FD)
                .index(1)
                .required(true)
                .help("file, directory, or file descriptor number")
                .value_parser(ValueParser::os_string()),
        )
        .arg(
            Arg::new(options::PROG_ARGS)
                .index(2)
                .num_args(1..)
                .trailing_var_arg(true)
                .help("command to execute and its arguments")
                .value_parser(ValueParser::os_string()),
        )
    // TODO:-o, -F and --verbose
}
