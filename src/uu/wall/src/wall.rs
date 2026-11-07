// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::builder::ValueParser;
#[cfg(all(unix, not(target_os = "openbsd")))]
use clap::parser::ValuesRef;
#[cfg(any(target_os = "windows", target_os = "openbsd"))]
use clap::ArgMatches;
use clap::{Arg, ArgAction, Command};
#[cfg(all(unix, not(target_os = "openbsd")))]
use nix::{
    errno::Errno,
    poll::{poll, PollFd, PollFlags, PollTimeout},
    unistd,
};
use std::env;
#[cfg(all(unix, not(target_os = "openbsd")))]
use std::ffi::OsString;
#[cfg(all(unix, not(target_os = "openbsd")))]
use std::io;
#[cfg(all(unix, not(target_os = "openbsd")))]
use std::io::prelude::*;
#[cfg(all(unix, not(target_os = "openbsd")))]
use std::os::fd::AsFd;
#[cfg(all(unix, not(target_os = "openbsd")))]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(all(unix, not(target_os = "openbsd")))]
use std::string::FromUtf8Error;
#[cfg(all(unix, not(target_os = "openbsd")))]
use thiserror::Error;

use uucore::error::UResult;
#[cfg(all(unix, not(target_os = "openbsd")))]
use uucore::error::{UError, USimpleError};
#[cfg(all(unix, not(target_os = "openbsd")))]
use uucore::utmpx::Utmpx;

const STRING: &str = "string";
// TODO(FEAT): Option for wall
// const OPT_GROUP: &str = "group";
#[cfg(target_os = "linux")]
const OPT_NOBANNER: &str = "nobanner";
#[cfg(target_os = "linux")]
const OPT_TIMEOUT: &str = "timeout";

#[cfg(all(unix, not(target_os = "openbsd")))]
#[derive(Error, Debug)]
enum WallError {
    #[error("I/O error")]
    IOerr(#[from] io::Error),
    #[error("encoding error")]
    VecToString(#[from] FromUtf8Error),
    #[error("osstring conversion failed")]
    ToStringError,
    #[error("terminal write timed out")]
    TimeoutError,
}

#[cfg(all(unix, not(target_os = "openbsd")))]
impl UError for WallError {
    fn code(&self) -> i32 {
        1
    }
}

#[cfg(all(unix, not(target_os = "openbsd")))]
#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uucore::clap_localization::handle_clap_result(uu_app(), args)?; // Clap would have return 101
    let message = get_message(matches.get_many(STRING).unwrap_or_default())?;
    #[cfg(target_os = "linux")]
    let option_n = matches.get_flag(OPT_NOBANNER);
    #[cfg(not(target_os = "linux"))]
    let option_n = false;

    #[cfg(target_os = "linux")]
    let timeout = *matches.get_one::<u16>(OPT_TIMEOUT).unwrap();
    #[cfg(not(target_os = "linux"))]
    let timeout = 30;
    let users = find_logged_users();
    let message = formatted_message(message, !option_n);
    write_to_terminals(users, &message, timeout)?;
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

#[cfg(target_os = "openbsd")]
#[uucore::main(no_signals)]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: ArgMatches = uu_app().try_get_matches_from(args)?;
    Err(uucore::error::USimpleError::new(
        1,
        "`wall` is not available on OpenBSD. Use of Utmpx crate is required, but it's not available on OpenBSD",
    ))
}

#[cfg(target_os = "linux")]
pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .about(uucore::help_about!("wall.md"))
        .infer_long_args(true)
        .override_usage(uucore::format_usage(uucore::help_usage!("wall.md")))
        // .arg(
        //     Arg::new(OPT_GROUP) // TODO(FEAT): Implement -g/--groups to target specific
        //         // users inside a group
        //         .short('g')
        //         .long(OPT_GROUP)
        //         .value_name("GROUP")
        //         .help("Send restrict to only users in the group(s)")
        //         .num_args(1)
        //         .required(false)
        //         .action(ArgAction::Append) // User can target more than one group
        //         .value_parser(clap::value_parser!(String)),
        // )
        .arg(
            Arg::new(OPT_NOBANNER)
                .short('n')
                .long(OPT_NOBANNER)
                .required(false)
                .action(ArgAction::SetTrue)
                .help("Suppress the intro banner of the broadcast"),
        )
        .arg(
            Arg::new(OPT_TIMEOUT) // TODO(FEAT): Implement -t --timeout to stop trying to print
                // after passed a delay
                .short('t')
                .long(OPT_TIMEOUT)
                .required(false)
                .value_name("SECONDS")
                .value_parser(clap::value_parser!(u16).range(1..))
                .default_value("30")
                .help("Abandon after t seconds the write attempt to the terminals")
                .num_args(1),
        )
        .arg(
            Arg::new(STRING)
                .action(ArgAction::Append)
                .value_parser(ValueParser::os_string()),
        )
}

// #[cfg(all(unix, not(target_os = "linux"), not(target_os = "openbsd")))]
#[cfg(not(target_os = "linux"))]
pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(uucore::crate_version!())
        .about(uucore::help_about!("wall.md"))
        .infer_long_args(true)
        .override_usage(uucore::format_usage(uucore::help_usage!("wall.md")))
        // .arg(
        //     Arg::new(OPT_GROUP) // TODO(FEAT): Implement -g/--groups to target specific
        //         // users inside a group
        //         .short('g')
        //         .long(OPT_GROUP)
        //         .value_name("GROUP")
        //         .help("Send restrict to only users in the group(s)")
        //         .num_args(1)
        //         .required(false)
        //         .action(ArgAction::Append) // User can target more than one group
        //         .value_parser(clap::value_parser!(String)),
        // )
        .arg(
            Arg::new(STRING)
                .action(ArgAction::Append)
                .value_parser(ValueParser::os_string()),
        )
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn get_message(args: ValuesRef<OsString>) -> Result<String, WallError> {
    if args.len() == 0 {
        read_from_stdin()
    } else if args.len() == 1 {
        match read_from_file(args.clone().next().unwrap()) {
            Ok(str) => Ok(str),
            Err(WallError::IOerr(e)) if e.kind() == io::ErrorKind::NotFound => {
                #[cfg(target_os = "linux")]
                return concatenate_message(args);
                #[cfg(not(target_os = "linux"))]
                return Err(WallError::IOerr(e));
            }
            Err(e) => Err(e),
        }
    } else {
        concatenate_message(args)
    }
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn read_from_stdin() -> Result<String, WallError> {
    let mut buffer = Vec::new();
    io::stdin().read_to_end(&mut buffer)?;
    let res = String::from_utf8(buffer)?;
    Ok(res)
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn read_from_file(file: &std::ffi::OsStr) -> Result<String, WallError> {
    let mut buffer = Vec::new();
    let mut file = std::fs::File::open(std::path::Path::new(file))?;
    file.read_to_end(&mut buffer)?;
    let res = String::from_utf8(buffer)?;
    Ok(res)
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn concatenate_message(args: ValuesRef<OsString>) -> Result<String, WallError> {
    let mut res = String::new();
    for arg in args {
        res.push_str(arg.to_str().ok_or(WallError::ToStringError)?);
        res.push(' ');
    }
    res.pop();
    Ok(res)
}

#[cfg(all(unix, not(target_os = "openbsd")))]
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

#[cfg(all(unix, not(target_os = "openbsd")))]
fn wall_intro_message() -> String {
    let user = "USER";
    let binding = unistd::gethostname().unwrap_or_else(|_| "".into());
    let hostname = binding.to_string_lossy();

    let user: String = env::var_os(user)
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    // Control characters (such as clearing the terminal) could be hidden into user.

    // Fetch the TTY of the process calling wall (requires OS-specific calls or a wrapper function)
    let tty = get_sender();

    let datetime = get_hour_and_date();
    #[cfg(target_os = "macos")]
    return format!(
        "\r\nBroadcast message from {}@{} ({tty}) at ({datetime}) \r\n\r\n",
        user, hostname
    );
    #[cfg(not(target_os = "macos"))]
    return format!(
        "\r\nBroadcast message from {}@{} ({tty}) ({datetime}) \r\n\r\n",
        user, hostname
    );
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn formatted_message(message: String, banner: bool) -> String {
    let formatted_message: String = message
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    // Same here, control characters could also be hidden in the message provided. We still want newline char of course...
    #[cfg(not(target_os = "macos"))]
    let formatted_message = formatted_message.replace('\n', "\r\n\n");
    #[cfg(not(target_os = "macos"))]
    let formatted_message = format!("{formatted_message}\r\n\n");

    #[cfg(target_os = "macos")]
    let formatted_message = formatted_message.replace('\n', "\r\n");

    let transmission = if banner {
        format!("{}{}", wall_intro_message(), formatted_message)
    } else {
        formatted_message
    };
    transmission
}

#[cfg(all(unix, not(target_os = "openbsd")))]
struct Terminal {
    file: std::fs::File,
    written: usize,
    failed: bool,
}

#[cfg(all(unix, not(target_os = "openbsd")))]
impl Terminal {
    fn new(file: std::fs::File) -> Self {
        Terminal {
            file,
            written: 0,
            failed: false,
        }
    }
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn write_to_terminals(users: Vec<OsString>, message: &str, timeout: u16) -> UResult<()> {
    let mut ttys: Vec<Terminal> = add_users_terminal(&users);
    let poll_errors = PollFlags::POLLHUP | PollFlags::POLLERR | PollFlags::POLLNVAL;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(u64::from(timeout));

    while !ttys.is_empty() {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(WallError::TimeoutError.into());
        }
        let events = poll_terminals(&ttys, remaining)?;
        for (tty, event) in ttys.iter_mut().zip(events) {
            if event.contains(PollFlags::POLLOUT) {
                let remaining = &message.as_bytes()[tty.written..];
                match unistd::write(tty.file.as_fd(), remaining) {
                    Ok(n) => tty.written += n,
                    Err(Errno::EAGAIN | Errno::EINTR) => continue,
                    Err(_) => tty.failed = true,
                }
            } else if event.intersects(poll_errors) {
                tty.failed = true;
            }
        }
        ttys.retain(|tty| tty.written < message.len() && !tty.failed);
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn poll_terminals(
    terminals: &[Terminal],
    remaining: std::time::Duration,
) -> UResult<Vec<PollFlags>> {
    let mut fds: Vec<_> = terminals
        .iter()
        .map(|tty| PollFd::new(tty.file.as_fd(), PollFlags::POLLOUT))
        .collect();
    let wait = PollTimeout::try_from(remaining)
        .map_err(|e| USimpleError::new(1, format!("invalid poll timeout: {e}")))?;

    match poll(&mut fds, wait) {
        Ok(0) => return Err(WallError::TimeoutError.into()),
        Ok(_) => {}
        Err(Errno::EINTR) => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    }
    Ok(fds
        .iter()
        .map(|fd| fd.revents().unwrap_or(PollFlags::empty()))
        .collect())
}

#[cfg(all(unix, not(target_os = "openbsd")))]
fn add_users_terminal(users: &Vec<OsString>) -> Vec<Terminal> {
    let mut ttys: Vec<Terminal> = Vec::new();
    for user in users {
        let file = match std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(user)
        {
            Ok(f) => f,
            Err(_) => continue,
        };
        if !unistd::isatty(&file).unwrap_or(false) {
            continue;
        }
        ttys.push(Terminal::new(file));
    }
    ttys
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "openbsd")))]
fn get_hour_and_date() -> String {
    chrono::Local::now().format("%a %b %e %H:%M %Y").to_string()
}

#[cfg(target_os = "macos")]
fn get_hour_and_date() -> String {
    chrono::Local::now().format("%a %b %e %H:%M %Z").to_string()
}

#[cfg(target_os = "macos")]
fn get_sender() -> String {
    unistd::ttyname(std::io::stderr().as_fd())
        .unwrap_or_else(|_| "".into())
        .to_string_lossy()
        .to_string()
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "openbsd")))]
fn get_sender() -> String {
    unistd::ttyname(std::io::stderr().as_fd())
        .unwrap_or_else(|_| "".into())
        .to_string_lossy()
        .strip_prefix("/dev/") // Wall doesn't print /dev/ after tty name, but might not be the way it does it
        .unwrap_or("")
        .to_string()
}
