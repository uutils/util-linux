// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use libc::PRIO_PROCESS;
use std::env;
use std::io::Error;
use std::process;
use std::str::FromStr;
use uucore::{error::UResult, format_usage, help_about, help_usage};
const ABOUT: &str = help_about!("renice.md");
const USAGE: &str = help_usage!("renice.md");
use clap::{crate_version, Command};

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: renice <nice value> <pid>");
        process::exit(1);
    }

    let nice_value = i32::from_str(&args[1]).unwrap_or_else(|_| {
        eprintln!("Invalid nice value");
        process::exit(1);
    });

    let pid = i32::from_str(&args[2]).unwrap_or_else(|_| {
        eprintln!("Invalid PID");
        process::exit(1);
    });

    if unsafe { libc::setpriority(PRIO_PROCESS, pid.try_into().unwrap(), nice_value) } == -1 {
        eprintln!("Failed to set nice value: {}", Error::last_os_error());
        process::exit(1);
    }

    println!("Nice value of process {} set to {}", pid, nice_value);
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
