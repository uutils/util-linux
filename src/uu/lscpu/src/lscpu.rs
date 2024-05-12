// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Command};
use regex::Regex;
use std::fs;
use sysinfo::System;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("lscpu.md");
const USAGE: &str = help_usage!("lscpu.md");

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    let system = System::new_all();
    let _cpu = system.global_cpu_info();

    println!("Architecture: {}", get_architecture());
    println!("CPU(s): {}", system.cpus().len());
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
        .infer_long_args(true)
}
