// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use regex::Regex;
use serde::Serialize;
use std::{fs, str::FromStr};
use sysinfo::System;
use uucore::{error::UResult, format_usage, help_about, help_usage};

mod options {
    pub const HEX: &str = "hex";
    pub const JSON: &str = "json";
}

const ABOUT: &str = help_about!("lscpu.md");
const USAGE: &str = help_usage!("lscpu.md");

#[derive(Serialize)]
struct CpuInfos {
    lscpu: Vec<CpuInfo>,
}

#[derive(Serialize)]
struct CpuInfo {
    field: String,
    data: String,
}

impl CpuInfos {
    fn new() -> CpuInfos {
        CpuInfos {
            lscpu: Vec::<CpuInfo>::new(),
        }
    }

    fn push(&mut self, field: &str, data: &str) {
        let cpu_info = CpuInfo {
            field: String::from_str(field).unwrap(),
            data: String::from_str(data).unwrap(),
        };
        self.lscpu.push(cpu_info);
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let system = System::new_all();

    let _hex = matches.get_flag(options::HEX);
    let json = matches.get_flag(options::JSON);

    let mut cpu_infos = CpuInfos::new();
    cpu_infos.push("Architecture", &get_architecture());
    cpu_infos.push("CPU(s)", &format!("{}", system.cpus().len()));
    // Add more CPU information here...

    if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
        let re = Regex::new(r"^model name\s+:\s+(.*)$").unwrap();
        // Assuming all CPUs have the same model name
        if let Some(cap) = re.captures_iter(&contents).next() {
            cpu_infos.push("Model name", &cap[1]);
        };
    }

    if json {
        println!("{}", cpu_infos.to_json());
    } else {
        for elt in cpu_infos.lscpu {
            println!("{}: {}", elt.field, elt.data);
        }
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
        .arg(
            Arg::new(options::HEX)
                .short('x')
                .long("hex")
                .action(ArgAction::SetTrue)
                .help(
                    "Use hexadecimal masks for CPU sets (for example 'ff'). \
                    The default is to print the sets in list format (for example 0,1).",
                )
                .required(false),
        )
        .arg(
            Arg::new(options::JSON)
                .long("json")
                .help(
                    "Use JSON output format for the default summary or extended output \
                    (see --extended). For backward compatibility, JSON output follows the \
                    default summary behavior for non-terminals (e.g., pipes) where \
                    subsections are missing. See also --hierarchic.",
                )
                .action(ArgAction::SetTrue),
        )
}
