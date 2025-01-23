// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use regex::Regex;
use std::fs;
use sysinfo::System;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("lscpu.md");
const USAGE: &str = help_usage!("lscpu.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let system = System::new_all();
    let hex= _matches.get_flag(options::HEX);

    println!("Architecture: {}", get_architecture());
    if hex {
        println!("CPU(s): 0x{:x}", system.cpus().len());
    } else {
        println!("CPU(s): {}", system.cpus().len());
    }
    // Add more CPU information here...

    if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
        let re = Regex::new(r"^model name\s+:\s+(.*)$").unwrap();
        // Assuming all CPUs have the same model name
        if let Some(cap) = re.captures_iter(&contents).next() {
            println!("Model name: {}", &cap[1]);
        };
    }
    Ok(())
}

// More options can be added here
mod options {
    pub const HEX: &str = "hex";
}

fn get_architecture() -> String {
    if cfg!(target_arch = "x86") {
        "x86".to_string()
    } else if cfg!(target_arch = "x86_64") {
        "x86_64".to_string()
    } else {
        "Unknown".to_string()
    }
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true).arg(Arg::new(options::HEX).short('x').long("hex").action(ArgAction::SetTrue).help("Use hexadecimal masks for CPU sets (for example 'ff'). The default is to print the
        sets in list format (for example 0,1). Note that before version 2.30 the mask has been
        printed with 0x prefix.").required(false))
}
