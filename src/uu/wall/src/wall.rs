// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

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
use uucore::{
    error::{UError, UResult},
    format_usage,
    translate, // unused at the moment...
    utmpx::Utmpx,
};

const STRING: &str = "string";
const OPT_GROUP: &str = "group";
const OPT_NOBANNER: &str = "nobanner";
const OPT_TIMEOUT: &str = "timeout";

#[cfg(target_os = "macos")]
mod options {
    use super::OPT_GROUP; // module don't automatically has access to const of parent

    pub const VALID_SHORT: &[char] = &['g'];
    pub const VALID_LONG: &[&str] = &[OPT_GROUP];
}

#[cfg(target_os = "linux")]
mod options {
    use super::{OPT_GROUP, OPT_NOBANNER, OPT_TIMEOUT};

    pub const VALID_SHORT: &[char] = &['g', 'n', 't'];
    pub const VALID_LONG: &[&str] = &[OPT_GROUP, OPT_NOBANNER, OPT_TIMEOUT];
}

#[derive(Error, Debug)]
enum WallError {
    #[error("wall: invalid argument")]
    ArgError,
    #[error("wall: cannot read stdin")]
    Stdin(#[from] io::Error),
    #[error("wall: encoding error")]
    VecToString(#[from] FromUtf8Error),
    #[error("wall: osstring conversion failed")]
    ToStringError,
    #[error("wall is not supported on windows")]
    WindowsError,
}

impl UError for WallError {
    fn code(&self) -> i32 {
        1
    }
}

#[cfg(target_family = "unix")]
#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    #[cfg(windows)]
    return Err(io::Error::new(WallError::WindowsError));
    let args = args.skip(1).peekable();
    match args_pre_scan(&args) {
        Ok(_) => {}
        Err(e) => {
            return Err(WallError::ArgError);
        }
    }
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?;
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

#[cfg(target_os = "macos")]
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

fn args_pre_scan(args: &ValuesRef<OsString>) -> Result<(), String> {
    for arg in args {
        let arg = arg.to_string_lossy();
        if arg == "--" {
            break;
        }
    }
    Ok(())
}

fn get_message(args: ValuesRef<OsString>) -> Result<String, WallError> {
    if args.len() == 0 {
        read_from_stdin()
    } else if args.len() == 1 {
        read_from_file(args.into_iter().next().unwrap())
    } else {
        concatenate_message(args)
    }
}

fn read_from_stdin() -> Result<String, WallError> {
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    let res = String::from_utf8(buffer)?;
    Ok(res)
}

fn read_from_file(file: &OsString) -> Result<String, WallError> {
    let mut buffer = Vec::new();
    let mut file = std::fs::File::open(file)?;
    file.read_to_end(&mut buffer)?;
    let res = String::from_utf8(buffer)?;
    Ok(res)
}

fn concatenate_message(args: ValuesRef<OsString>) -> Result<String, WallError> {
    let mut res = String::new();
    for arg in args {
        res.push_str(arg.to_str().ok_or(WallError::ToStringError)?);
        res.push(' ');
    }
    res.pop();
    Ok(res)
}

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

fn wall_intro_message() -> String {
    let user = "USER";
    let biding = unistd::gethostname().unwrap_or_else(|_| "".into());
    let hostname = biding.to_string_lossy();

    let user = env::var_os(user).unwrap_or_default();
    // Fetch the TTY of the process calling wall (requires OS-specific calls or a wrapper function)
    let tty = &get_sender();

    let datetime = get_hour_and_date();
    format!(
        "\r\nBroadcast message from {}@{} ({tty}) at ({datetime}) \r\n\r\n",
        user.to_string_lossy(),
        hostname
    )
}

fn write_to_terminals(message: String, users: Vec<OsString>) -> UResult<()> {
    let format_message = message.replace("\n", "\r\n\n");
    let transmission = wall_intro_message() + &format_message;
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
    chrono::Local::now().format("%a %b %e %H:%M %Z").to_string()
}

#[cfg(target_os = "macos")]
fn get_hour_and_date() -> String {
    chrono::Local::now().format("%a %b %e %H:%M %Z").to_string()
}

fn get_sender() -> String {
    unistd::ttyname(std::io::stdin().as_fd())
        .unwrap_or_else(|_| "".into())
        .to_string_lossy()
        .to_string()
}
