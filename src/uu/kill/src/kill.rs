// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod errors;

use crate::errors::KillError;
#[cfg(target_os = "linux")]
use crate::errors::KillError::{NoSuchProcess, OperationNotPermitted};
use clap::{Arg, ArgAction, Command, crate_version, value_parser};
use uucore::libc;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("kill.md");
const USAGE: &str = help_usage!("kill.md");

#[cfg(not(target_os = "linux"))]
fn kill(_pid: i32, _signal: i32) -> Result<(), KillError> {
    Err(KillError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn kill(pid: i32, signal: i32) -> Result<(), KillError> {
    unsafe { libc::kill(pid, signal) };

    let err = std::io::Error::last_os_error().raw_os_error();
    if let Some(err_no) = err {
        match err_no {
            libc::EPERM => return Err(OperationNotPermitted(pid)),
            libc::ESRCH => return Err(NoSuchProcess(pid)),
            _ => {}
        }
    }

    Ok(())
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    if let Some(pids) = matches.get_many::<i32>("pid") {
        for pid in pids {
            kill(*pid, libc::SIGTERM)?;
        }
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("pid")
                .help("PID of the process to kill")
                .required(true)
                .action(ArgAction::Append)
                .value_name("PID")
                .value_parser(value_parser!(i32)),
        )
}
