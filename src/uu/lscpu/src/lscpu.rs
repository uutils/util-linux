// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use regex::RegexBuilder;
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<CpuInfo>,
}

impl CpuInfos {
    fn new() -> CpuInfos {
        CpuInfos {
            lscpu: Vec::<CpuInfo>::new(),
        }
    }

    fn push(&mut self, field: &str, data: &str, children: Option<Vec<CpuInfo>>) {
        let cpu_info = CpuInfo {
            field: String::from_str(field).unwrap(),
            data: String::from_str(data).unwrap(),
            children: children.unwrap_or_default(),
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
    cpu_infos.push("Architecture", &get_architecture(), None);
    cpu_infos.push("CPU(s)", &format!("{}", system.cpus().len()), None);
    // Add more CPU information here...

    if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
        if let Some(cpu_model) = find_cpuinfo_value(&contents, "model name") {
            if let Some(addr_sizes) = find_cpuinfo_value(&contents, "address sizes") {
                cpu_infos.push(
                    "Model name",
                    cpu_model.as_str(),
                    Some(vec![CpuInfo {
                        field: "Address sizes".to_string(),
                        data: addr_sizes,
                        children: vec![],
                    }]),
                );
            } else {
                cpu_infos.push("Model name", cpu_model.as_str(), None);
            }
        }
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

fn find_cpuinfo_value(contents: &str, key: &str) -> Option<String> {
    let pattern = format!(r"^{}\s+:\s+(.*)$", key);
    let re = RegexBuilder::new(pattern.as_str())
        .multi_line(true)
        .build()
        .unwrap();

    if let Some(cap) = re.captures_iter(contents).next() {
        return Some(cap[1].to_string());
    };

    None
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
