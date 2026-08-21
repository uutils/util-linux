// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};
#[cfg(not(windows))]
use libc::{PRIO_PGRP, PRIO_PROCESS, PRIO_USER};
#[cfg(not(windows))]
use std::io::Error;
use std::process;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("renice.md");
const USAGE: &str = help_usage!("renice.md");

#[derive(Clone, Copy)]
enum TargetKind {
    Process,
    ProcessGroup,
    User,
}

impl TargetKind {
    #[cfg(not(windows))]
    fn which(self) -> u32 {
        match self {
            Self::Process => PRIO_PROCESS,
            Self::ProcessGroup => PRIO_PGRP,
            Self::User => PRIO_USER,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Process => "process ID",
            Self::ProcessGroup => "process group ID",
            Self::User => "user ID",
        }
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;
    let arguments = matches
        .get_many::<String>("arguments")
        .unwrap()
        .collect::<Vec<_>>();

    let priority_option = matches.get_one::<String>("priority_option");
    let relative_option = matches.get_one::<String>("relative");
    let (nice_value_str, identifiers, relative) =
        if let Some(nice_value_str) = priority_option.or(relative_option) {
            (
                nice_value_str,
                arguments.as_slice(),
                relative_option.is_some(),
            )
        } else {
            let Some((nice_value_str, identifiers)) = arguments.split_first() else {
                eprintln!("Invalid nice value");
                process::exit(1);
            };
            (*nice_value_str, identifiers, false)
        };

    if identifiers.is_empty() {
        eprintln!("Invalid identifier");
        process::exit(1);
    }

    let nice_value = nice_value_str.parse::<i32>().unwrap_or_else(|_| {
        eprintln!("Invalid nice value");
        process::exit(1);
    });

    let target_kind = if matches.get_flag("pgrp") {
        TargetKind::ProcessGroup
    } else if matches.get_flag("user") {
        TargetKind::User
    } else {
        TargetKind::Process
    };

    for identifier in identifiers {
        let id = identifier.parse().unwrap_or_else(|_| {
            eprintln!("Invalid {}", target_kind.label());
            process::exit(1);
        });

        set_nice_value(target_kind, id, nice_value, relative);
    }

    Ok(())
}

#[cfg(not(windows))]
fn set_nice_value(target_kind: TargetKind, id: u32, nice_value: i32, relative: bool) {
    let which = target_kind.which();
    let nice_value = if relative {
        let current = unsafe { libc::getpriority(which, id) };
        if current == -1 {
            eprintln!(
                "Failed to get nice value for {} {id}: {}",
                target_kind.label(),
                Error::last_os_error()
            );
            process::exit(1);
        }
        current + nice_value
    } else {
        nice_value
    };

    if unsafe { libc::setpriority(which, id, nice_value) } == -1 {
        eprintln!(
            "Failed to set nice value for {} {id}: {}",
            target_kind.label(),
            Error::last_os_error()
        );
        process::exit(1);
    }

    println!(
        "Nice value of {} {id} set to {nice_value}",
        target_kind.label()
    );
}

// TODO: implement functionality on windows
#[cfg(windows)]
fn set_nice_value(_target_kind: TargetKind, id: u32, nice_value: i32, _relative: bool) {
    println!("Nice value of process ID {id} set to {nice_value}");
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .group(
            ArgGroup::new("priority")
                .args(["priority_option", "relative"])
                .multiple(false),
        )
        .group(
            ArgGroup::new("target")
                .args(["pid", "pgrp", "user"])
                .multiple(false),
        )
        .arg(
            Arg::new("priority_option")
                .short('n')
                .long("priority")
                .value_name("NICE_VALUE")
                .allow_negative_numbers(true)
                .help("The new nice value for the process"),
        )
        .arg(
            Arg::new("relative")
                .long("relative")
                .value_name("NICE_VALUE")
                .allow_negative_numbers(true)
                .help("Adjust the nice value relative to the current value"),
        )
        .arg(
            Arg::new("pid")
                .short('p')
                .long("pid")
                .action(ArgAction::SetTrue)
                .help("Interpret identifiers as process IDs"),
        )
        .arg(
            Arg::new("pgrp")
                .short('g')
                .long("pgrp")
                .action(ArgAction::SetTrue)
                .help("Interpret identifiers as process group IDs"),
        )
        .arg(
            Arg::new("user")
                .short('u')
                .long("user")
                .action(ArgAction::SetTrue)
                .help("Interpret identifiers as user IDs"),
        )
        .arg(
            Arg::new("arguments")
                .value_name("NICE_VALUE IDENTIFIER...")
                .allow_negative_numbers(true)
                .num_args(1..)
                .required(true),
        )
}
