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
use clap::{crate_version, Arg, Command};

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let nice_value = *matches.get_one::<i32>("nice_value").unwrap_or_else(|| {
        eprintln!("Invalid nice value");
        process::exit(1);
    });

    let pid = *matches.get_one::<i32>("pid").unwrap_or_else(|| {
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
        .arg(
            Arg::new("nice_value")
                .value_name("NICE_VALUE")
                .help("The new nice value for the process")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("pid")
                .value_name("PID")
                .help("The PID of the process")
                .required(true)
                .index(2),
        )
}
