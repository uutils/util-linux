// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(not(unix))]
use clap::ArgMatches;
use clap::builder::ValueParser;
use clap::parser::ValuesRef;
use clap::{Arg, ArgAction, Command};
#[cfg(unix)]
use nix::unistd;
use std::env;
use std::ffi::OsString;
use std::io;
use std::io::prelude::*;
#[cfg(unix)]
use std::os::fd::AsFd;
use std::string::FromUtf8Error;
use thiserror::Error;

#[cfg(unix)]
use uucore::{error::USimpleError, utmpx::Utmpx};
use uucore::{
    error::{UError, UResult},
    format_usage, translate,
};

const STRING: &str = "string";
const OPT_GROUP: &str = "group";
#[cfg(target_os = "linux")]
const OPT_NOBANNER: &str = "nobanner";
#[cfg(target_os = "linux")]
const OPT_TIMEOUT: &str = "timeout";

#[derive(Error, Debug)]
enum WallError {
    #[error("wall: cannot read stdin")]
    Stdin(#[from] io::Error),
    #[error("wall: encoding error")]
    VecToString(#[from] FromUtf8Error),
    #[error("wall: osstring conversion failed")]
    ToStringError,
}

impl UError for WallError {
    fn code(&self) -> i32 {
        1
    }
}

#[cfg(target_family = "unix")]
#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)
        .map_err(|e| USimpleError::new(1, e.to_string()))?; // Clap would have return 101
    // Might be considered wrong for --help and --version
    let message = get_message(matches.get_many(STRING).unwrap_or_default())?;
    let users = find_logged_users();
    write_to_terminals(message, users)?;
    Ok(())
}

#[cfg(not(target_family = "unix"))]
#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: ArgMatches = uu_app().try_get_matches_from(args)?;
    Err(uucore::error::USimpleError::new(
        1,
        "`wall` is available only on Unix platforms.",
    ))
}

#[cfg(target_os = "linux")]
pub fn uu_app() -> Command {
    Command::new("wall")
        .version(uucore::crate_version!())
        .about(translate!("wall.md"))
        .infer_long_args(true)
        .override_usage(format_usage(&translate!("wall.md")))
        .arg(
            Arg::new(OPT_GROUP) // TODO(FEAT): Implement -g/--groups to target specific
                // users inside a group
                .short('g')
                .long(OPT_GROUP)
                .value_name("GROUP")
                .help("Send restrict to only users in the group(s)")
                .num_args(1)
                .required(false)
                .action(ArgAction::Append) // User can target more than one group
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new(OPT_NOBANNER) // TODO(FEAT): Implement -n/--nobanner to remove broadcasting
                // intro message
                .short('n')
                .long(OPT_NOBANNER)
                .required(false)
                .action(ArgAction::SetTrue)
                .help("Suppress the intro branner of the broadcast"),
        )
        .arg(
            Arg::new(OPT_TIMEOUT) // TODO(FEAT): Implement -t --timeout to stop trying to print
                // after passed a delay
                .short('t')
                .long(OPT_TIMEOUT)
                .required(false)
                .value_name("SECONDS")
                .help("Abandon after t seconds the write attempt to the terminals")
                .num_args(1),
        )
        .arg(
            Arg::new(STRING)
                .action(ArgAction::Append)
                .value_parser(ValueParser::os_string()),
        )
}

#[cfg(not(target_os = "linux"))]
pub fn uu_app() -> Command {
    Command::new("wall")
        .version(uucore::crate_version!())
        .about(translate!("wall.md"))
        .infer_long_args(true)
        .override_usage(format_usage(&translate!("wall.md")))
        .arg(
            Arg::new(OPT_GROUP) // TODO(FEAT): Implement -g/--groups to target specific
                // users inside a group
                .short('g')
                .long(OPT_GROUP)
                .value_name("GROUP")
                .help("Send restrict to only users in the group(s)")
                .num_args(1)
                .required(false)
                .action(ArgAction::Append) // User can target more than one group
                .value_parser(clap::value_parser!(String)),
        )
        .arg(
            Arg::new(STRING)
                .action(ArgAction::Append)
                .value_parser(ValueParser::os_string()),
        )
}

#[cfg(target_family = "unix")]
fn get_message(args: ValuesRef<OsString>) -> Result<String, WallError> {
    if args.len() == 0 {
        read_from_stdin()
    } else if args.len() == 1 {
        match read_from_file(args.clone().next().unwrap()) {
            Ok(str) => Ok(str),
            Err(_e) => {
                #[cfg(target_os = "linux")]
                return concatenate_message(args);
                #[cfg(not(target_os = "linux"))]
                return Err(_e);
            }
        }
    } else {
        concatenate_message(args)
    }
}

#[cfg(target_family = "unix")]
fn read_from_stdin() -> Result<String, WallError> {
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    let res = String::from_utf8(buffer)?;
    Ok(res)
}

#[cfg(target_family = "unix")]
fn read_from_file(file: &OsString) -> Result<String, WallError> {
    let mut buffer = Vec::new();
    let mut file = std::fs::File::open(file)?;
    file.read_to_end(&mut buffer)?;
    let res = String::from_utf8(buffer)?;
    Ok(res)
}

#[cfg(target_family = "unix")]
fn concatenate_message(args: ValuesRef<OsString>) -> Result<String, WallError> {
    let mut res = String::new();
    for arg in args {
        res.push_str(arg.to_str().ok_or(WallError::ToStringError)?);
        res.push(' ');
    }
    res.pop();
    Ok(res)
}

#[cfg(target_family = "unix")]
fn find_logged_users() -> Vec<OsString> {
    let mut res = Vec::<OsString>::new();
    for ut in Utmpx::iter_all_records() {
        if ut.is_user_process() {
            let mut tty_path = OsString::from("/dev/");
            tty_path.push(OsString::from(&ut.tty_device().clone()));
            res.push(tty_path);
        }
    }
    res
}

#[cfg(target_family = "unix")]
fn wall_intro_message() -> String {
    let user = "USER";
    let biding = unistd::gethostname().unwrap_or_else(|_| "".into());
    let hostname = biding.to_string_lossy();

    let user = env::var_os(user).unwrap_or_default();
    // Fetch the TTY of the process calling wall (requires OS-specific calls or a wrapper function)
    let tty = &get_sender();

    let datetime = get_hour_and_date();
    #[cfg(target_os = "macos")]
    return format!(
        "\r\nBroadcast message from {}@{} ({tty}) at ({datetime} \r\n\r\n",
        user.to_string_lossy(),
        hostname
    );
    #[cfg(target_os = "linux")]
    return format!(
        "\r\nBroadcast message from {}@{} ({tty}) ({datetime}) \r\n\r\n",
        user.to_string_lossy(),
        hostname
    );
}

#[cfg(target_family = "unix")]
fn write_to_terminals(message: String, users: Vec<OsString>) -> UResult<()> {
    #[cfg(target_os = "linux")]
    let mut formatted_message = message.replace('\n', "\r\n\n");
    #[cfg(target_os = "linux")]
    formatted_message.push_str("\r\n\n");

    #[cfg(not(target_os = "linux"))]
    let formatted_message = message.replace('\n', "\r\n\n");

    let transmission = format!("{}{}", wall_intro_message(), formatted_message);
    for user in users {
        let mut file = match std::fs::OpenOptions::new().write(true).open(user) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if !unistd::isatty(&file).unwrap_or(false) {
            continue;
        }
        write!(file, "{transmission}").map_err(|e| {
            eprintln!("wall-error: terminal write:, {e}",);
            WallError::Stdin(e)
        })?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn get_hour_and_date() -> String {
    chrono::Local::now().format("%a %b %e %H:%M %Y").to_string()
}

#[cfg(target_os = "macos")]
fn get_hour_and_date() -> String {
    chrono::Local::now().format("%a %b %e %H:%M %Z").to_string()
}

#[cfg(target_os = "macos")]
fn get_sender() -> String {
    unistd::ttyname(std::io::stdin().as_fd())
        .unwrap_or_else(|_| "".into())
        .to_string_lossy()
        .to_string()
}

#[cfg(target_os = "linux")]
fn get_sender() -> String {
    unistd::ttyname(std::io::stdin().as_fd())
        .unwrap_or_else(|_| "".into())
        .to_string_lossy()
        .strip_prefix("/dev/") // Wall doesn't print /dev/ after tty name, but might not be the way it does it
        .unwrap_or("")
        .to_string()
}
