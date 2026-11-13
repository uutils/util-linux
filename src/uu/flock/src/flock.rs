// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{builder::ValueParser, crate_version, value_parser, Arg, ArgAction, Command};
#[cfg(unix)]
use std::{
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    os::fd::{AsRawFd, RawFd},
    os::unix::process::ExitStatusExt,
    time::{Duration, Instant},
};
#[cfg(unix)]
use uucore::error::FromIo;
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
enum LockMode {
    Shared,
    Exclusive,
    Unlock,
}

#[cfg(unix)]
fn open_lock_file(path: &OsStr) -> UResult<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .or_else(|err| OpenOptions::new().read(true).open(path).map_err(|_| err))
        .map_err_context(|| format!("cannot open lock file {}", path.to_string_lossy()))
}

#[cfg(unix)]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;
    let timeout: Option<f64> = matches.get_one::<f64>(options::TIMEOUT).copied();

    let lock_mode = if matches.get_flag(options::SHARED) {
        LockMode::Shared
    } else if matches.get_flag(options::UNLOCK) {
        LockMode::Unlock
    } else {
        LockMode::Exclusive
    };

    let file_or_fd = matches.get_one::<OsString>(options::FILE_OR_FD).unwrap();
    let has_command =
        matches.contains_id(options::PROG_ARGS) || matches.contains_id(options::COMMAND);

    let (fd, _keep_open): (RawFd, Option<File>) = if has_command {
        let file = open_lock_file(file_or_fd)?;
        let fd = file.as_raw_fd();
        (fd, Some(file))
    } else {
        let fd = file_or_fd
            .to_str()
            .and_then(|s| s.parse::<RawFd>().ok())
            .ok_or_else(|| {
                uucore::error::USimpleError::new(
                    1,
                    format!("Invalid file descriptor: {}", file_or_fd.to_string_lossy()),
                )
            })?;
        (fd, None)
    };

    let nonblock = matches.get_flag(options::NONBLOCK);
    let conflict_exit_code = *matches.get_one::<u8>(options::CONFLICT_EXIT_CODE).unwrap();

    let operation = match lock_mode {
        LockMode::Shared => libc::LOCK_SH,
        LockMode::Exclusive => libc::LOCK_EX,
        LockMode::Unlock => libc::LOCK_UN,
    };

    let deadline = timeout
        .map(|secs| {
            Duration::try_from_secs_f64(secs)
                .ok()
                .and_then(|d| Instant::now().checked_add(d))
                .ok_or_else(|| uucore::error::USimpleError::new(1, "invalid timeout"))
        })
        .transpose()?;

    loop {
        let op = if nonblock || deadline.is_some() {
            operation | libc::LOCK_NB
        } else {
            operation
        };

        if unsafe { libc::flock(fd, op) } == 0 {
            break;
        }

        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::WouldBlock {
            if nonblock {
                return Err(uucore::error::ExitCode::new(i32::from(conflict_exit_code)));
            }
            if let Some(dl) = deadline {
                let now = Instant::now();
                if now >= dl {
                    return Err(uucore::error::ExitCode::new(i32::from(conflict_exit_code)));
                }
                std::thread::sleep((dl - now).min(Duration::from_millis(5)));
                continue;
            }
        }
        return Err(err.map_err_context(|| "flock failed".to_string()));
    }

    let mut cmd = if let Some(prog_args) = matches.get_many::<OsString>(options::PROG_ARGS) {
        let args: Vec<&OsString> = prog_args.collect();
        let mut command = std::process::Command::new(args[0]);
        command.args(&args[1..]);
        command
    } else if let Some(shell_cmd) = matches.get_one::<OsString>(options::COMMAND) {
        let shell = std::env::var_os("SHELL")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| OsString::from("/bin/sh"));
        let mut command = std::process::Command::new(shell);
        command.arg("-c").arg(shell_cmd);
        command
    } else {
        return Ok(());
    };

    match cmd.status() {
        Ok(status) => {
            let code = status
                .code()
                .or_else(|| status.signal().map(|sig| 128 + sig))
                .unwrap_or(1);
            uucore::error::set_exit_code(code);
            Ok(())
        }
        Err(err) => {
            let code = if err.kind() == std::io::ErrorKind::NotFound {
                127
            } else {
                126
            };
            Err(uucore::error::USimpleError::new(
                code,
                format!("failed to execute command: {err}"),
            ))
        }
    }
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
                .allow_negative_numbers(true)
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
