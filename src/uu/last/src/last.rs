use uucore::{error::{UError, UResult}, format_usage, help_about, help_usage, libc::utmpx, utmpx::{Utmpx, UtmpxIter}};
use std::{cmp::Reverse, error::Error, fmt::{self, Display}, io::BufReader, iter::Rev, net::IpAddr, str::{FromStr, Utf8Error}};
use std::io::Write;
use std::fs::File;
use clap::{crate_version, Arg, ArgAction, ArgMatches, Command};
use dns_lookup::lookup_addr;

mod platform;

mod options {
    pub const SYSTEM: &str = "system";
    pub const HOSTLAST: &str = "hostlast";
    pub const NO_HOST: &str = "nohostname";
    pub const LIMIT: &str = "limit";
    pub const DNS: &str = "dns";
    pub const TIME_FORMAT: &str = "time-format";
    pub const USER_TTY: &str = "username";
    pub const FILE: &str = "file";
}

const ABOUT: &str = help_about!("last.md");
const USAGE: &str = help_usage!("last.md");

#[uucore::main]
use platform::uumain;

#[cfg(target_os = "linux")]
static RUNLEVEL_HELP: &str = "print current runlevel";
#[cfg(not(target_os = "linux"))]
static RUNLEVEL_HELP: &str = "print current runlevel (This is meaningless on non Linux)";

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
                .required(false)
        )
        .arg(
            Arg::new(options::SYSTEM)
                .short('x')
                .long(options::SYSTEM)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("display system shutdown entries and run level changes")
        )
        .arg(
            Arg::new(options::DNS)
                .short('d')
                .long(options::DNS)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("translate the IP number back into a hostname")
        )
        .arg(
            Arg::new(options::HOSTLAST)
                .short('a')
                .long(options::HOSTLAST)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("display hostnames in the last column")
        ) 
        .arg(
            Arg::new(options::NO_HOST)
                .short('R')
                .long(options::NO_HOST)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("don't display the hostname field")
        )
        .arg(
            Arg::new(options::LIMIT)
                .short('n')
                .long(options::LIMIT)
                .action(ArgAction::Set)
                .required(false)
                .help("how many lines to show")
                .value_parser(clap::value_parser!(i32))
                .allow_negative_numbers(true)
        )
        .arg(
            Arg::new(options::TIME_FORMAT)
                .long(options::TIME_FORMAT)
                .action(ArgAction::Set)
                .required(false)
                .help("show timestamps in the specified <format>: notime|short|full|iso")
                .default_value("short")
        )
        .arg(
            Arg::new(options::USER_TTY)
                .action(ArgAction::Append)
        )
}