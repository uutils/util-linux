// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::builder::ValueParser;
use clap::{crate_version, Command};
use clap::{Arg, ArgAction};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("setsid.md");
const USAGE: &str = help_usage!("setsid.md");

#[cfg(target_family = "unix")]
mod unix {
    pub use std::ffi::{OsStr, OsString};
    pub use std::os::unix::process::CommandExt;
    pub use std::{io, process};
    pub use uucore::error::{FromIo, UIoError, UResult};

    // The promise made by setsid(1) is that it forks if the process
    // is already a group leader, not session leader.
    pub fn already_group_leader() -> bool {
        let leader = unsafe { libc::getpgrp() };
        leader == process::id() as i32
    }

    pub fn report_failure_to_exec(error: io::Error, executable: &OsStr, set_error: bool) {
        let kind = error.kind();

        // FIXME: POSIX wants certain exit statuses for specific errors, should
        // these be handled by uucore::error? We should be able to just return
        // the UError here.
        uucore::show_error!(
            "failed to execute {}: {}",
            executable.to_string_lossy(),
            UIoError::from(error)
        );

        if set_error {
            if kind == io::ErrorKind::NotFound {
                uucore::error::set_exit_code(127);
            } else if kind == io::ErrorKind::PermissionDenied {
                uucore::error::set_exit_code(126);
            }
        }
    }

    // This function will be potentially called after a fork(), so what it can do
    // is quite restricted. This is the meat of the program.
    pub fn prepare_child(take_controlling_tty: bool) -> io::Result<()> {
        // SAFETY: this is effectively a wrapper to the setsid syscall.
        let pid = unsafe { libc::setsid() };

        // We fork if we are already a group leader, so an error
        // here should be impossible.
        assert_eq!(pid, process::id() as i32);

        // On some platforms (i.e. aarch64 Linux) TIOCSCTTY is the same type as the second argument,
        // but on some it is u64, while the expected type is u32.
        // SAFETY: the ioctl should not make any changes to memory, basically a wrapper
        // to the syscall.
        #[allow(clippy::useless_conversion)]
        if take_controlling_tty && unsafe { libc::ioctl(0, libc::TIOCSCTTY.into(), 1) } < 0 {
            // This is unfortunate, but we are bound by the Result type pre_exec requires,
            // as well as the limitations imposed by this being executed post-fork().
            // Ideally we would return an io::Error of the Other kind so that we could handle
            // everything at the same place, but that would require an allocation.
            uucore::show_error!(
                "failed to set the controlling terminal: {}",
                UIoError::from(io::Error::last_os_error())
            );

            // SAFETY: this is actually safer than calling process::exit(), as that may
            // allocate, which is not safe post-fork.
            unsafe { libc::_exit(1) };
        }
        Ok(())
    }

    pub fn spawn_command(
        mut to_run: process::Command,
        executable: &OsStr,
        wait_child: bool,
    ) -> UResult<()> {
        let mut child = match to_run.spawn() {
            Ok(child) => child,
            Err(error) => {
                report_failure_to_exec(error, executable, wait_child);
                return Ok(());
            }
        };

        if !wait_child {
            return Ok(());
        }

        match child.wait() {
            Ok(status) => {
                uucore::error::set_exit_code(status.code().unwrap());
                Ok(())
            }
            Err(error) => {
                Err(error.map_err_context(|| format!("failed to wait on PID {}", child.id())))
            }
        }
    }
}

#[cfg(target_family = "unix")]
use unix::*;

#[cfg(target_family = "unix")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let force_fork = matches.get_flag("fork");
    let wait_child = matches.get_flag("wait");
    let take_controlling_tty = matches.get_flag("ctty");

    let command: Vec<_> = match matches.get_many::<OsString>("command") {
        Some(v) => v.collect(),
        None => return Err(uucore::error::USimpleError::new(1, "no command specified")),
    };

    // We know we have at least one item, as none was
    // handled as an error on the match above.
    let executable = command[0];
    let arguments = command.get(1..).unwrap_or(&[]);

    let mut to_run = process::Command::new(executable);
    to_run.args(arguments.iter());

    // SAFETY: pre_exec() happens post-fork, so the process can potentially
    // be in a broken state; allocations are not safe, and we should exit
    // as soon as possible if we cannot go ahead.
    unsafe {
        to_run.pre_exec(move || prepare_child(take_controlling_tty));
    };

    if force_fork || already_group_leader() {
        spawn_command(to_run, executable, wait_child)?;
    } else {
        let error = to_run.exec();
        report_failure_to_exec(error, executable, true);
    }

    Ok(())
}

#[cfg(not(target_family = "unix"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`setsid` is unavailable on non-UNIX-like platforms.",
    ))
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("ctty")
                .short('c')
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool())
                .help("Take the current controlling terminal"),
        )
        .arg(
            Arg::new("fork")
                .short('f')
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool())
                .long_help("Always create a new process. By default this is only done if we are already a process group lead."),
        )
        .arg(
            Arg::new("wait")
                .short('w')
                .action(ArgAction::SetTrue)
                .value_parser(ValueParser::bool())
                .help("Wait for the command to finish and exit with its exit code."),
        )
        .arg(
            Arg::new("command")
                .help("Program to be executed, followed by its arguments")
                .index(1)
                .action(ArgAction::Set)
                .trailing_var_arg(true)
                .value_parser(ValueParser::os_string())
                .num_args(1..),
        )
}
