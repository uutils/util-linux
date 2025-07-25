// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use uucore::{format_usage, help_about, help_usage};

mod platform;

mod options {
    pub const SYSTEM: &str = "system";
    pub const HOSTLAST: &str = "hostlast";
    pub const NO_HOST: &str = "nohostname";
    pub const LIMIT: &str = "limit";
    pub const DNS: &str = "dns";
    pub const TIME_FORMAT: &str = "time-format";
    pub const SINCE: &str = "since";
    pub const UNTIL: &str = "until";
    pub const USER_TTY: &str = "username";
    pub const FILE: &str = "file";
}

const ABOUT: &str = help_about!("last.md");
const USAGE: &str = help_usage!("last.md");

#[uucore::main]
use platform::uumain;

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE)
                .short('f')
                .long("file")
                .action(ArgAction::Set)
                .default_value("/var/log/wtmp")
                .help("use a specific file instead of /var/log/wtmp")
                .required(false),
        )
        .arg(
            Arg::new(options::SYSTEM)
                .short('x')
                .long(options::SYSTEM)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("display system shutdown entries and run level changes"),
        )
        .arg(
            Arg::new(options::DNS)
                .short('d')
                .long(options::DNS)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("translate the IP number back into a hostname"),
        )
        .arg(
            Arg::new(options::HOSTLAST)
                .short('a')
                .long(options::HOSTLAST)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("display hostnames in the last column"),
        )
        .arg(
            Arg::new(options::NO_HOST)
                .short('R')
                .long(options::NO_HOST)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("don't display the hostname field"),
        )
        .arg(
            Arg::new(options::LIMIT)
                .short('n')
                .long(options::LIMIT)
                .action(ArgAction::Set)
                .required(false)
                .help("how many lines to show")
                .value_parser(clap::value_parser!(i32))
                .allow_negative_numbers(true),
        )
        .arg(
            Arg::new(options::TIME_FORMAT)
                .long(options::TIME_FORMAT)
                .action(ArgAction::Set)
                .required(false)
                .help("show timestamps in the specified <format>: notime|short|full|iso")
                .default_value("short"),
        )
        .arg(
            Arg::new(options::SINCE)
                .short('s')
                .long("since")
                .action(ArgAction::Set)
                .required(false)
                .help("display the lines since the specified time")
                .value_name("time")
                .default_value("0000-01-01 00:00:00"),
        )
        .arg(
            Arg::new(options::UNTIL)
                .short('t')
                .long("until")
                .action(ArgAction::Set)
                .required(false)
                .help("display the lines until the specified time")
                .value_name("time")
                .default_value("9999-12-31 23:59:59"),
        )
        .arg(Arg::new(options::USER_TTY).action(ArgAction::Append))
}
