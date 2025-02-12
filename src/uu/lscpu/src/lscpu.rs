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

struct OutputOptions {
    json: bool,
    _hex: bool,
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let system = System::new_all();

    let output_opts = OutputOptions {
        _hex: matches.get_flag(options::HEX),
        json: matches.get_flag(options::JSON),
    };

    let mut cpu_infos = CpuInfos::new();
    cpu_infos.push("CPU(s)", &format!("{}", system.cpus().len()), None);
    // Add more CPU information here...

    let contents = match fs::read_to_string("/proc/cpuinfo") {
        // Early return if we can't read /proc/cpuinfo for whatever reason
        // TODO: Should this return an non-zero exit code to user, or do we just ignore it and display whatever CPU info we could find?
        Err(_) => return Ok(()),
        Ok(contents) => contents,
    };

    let mut arch_children: Vec<CpuInfo> = vec![];

    if let Some(addr_sizes) = find_cpuinfo_value(&contents, "address sizes") {
        arch_children.push(CpuInfo {
            field: "Address sizes".to_string(),
            data: addr_sizes,
            children: vec![],
        })
    }

    cpu_infos.push("Architecture", &get_architecture(), Some(arch_children));

    // TODO: This is currently quite verbose and doesn't strictly respect the hierarchy of `/proc/cpuinfo` contents
    // ie. the file might contain multiple sections, each with their own vendor_id/model name etc. but right now
    // we're just taking whatever our regex matches first and using that
    if let Some(vendor) = find_cpuinfo_value(&contents, "vendor_id") {
        let mut vendor_children: Vec<CpuInfo> = vec![];

        if let Some(model_name) = find_cpuinfo_value(&contents, "model name") {
            let mut model_children: Vec<CpuInfo> = vec![];

            if let Some(family) = find_cpuinfo_value(&contents, "cpu family") {
                model_children.push(CpuInfo {
                    field: "CPU Family".to_string(),
                    data: family,
                    children: vec![],
                });
            }

            if let Some(model) = find_cpuinfo_value(&contents, "model") {
                model_children.push(CpuInfo {
                    field: "Model".to_string(),
                    data: model,
                    children: vec![],
                });
            }

            vendor_children.push(CpuInfo {
                field: "Model name".to_string(),
                data: model_name,
                children: model_children,
            });
        }
        cpu_infos.push("Vendor ID", vendor.as_str(), Some(vendor_children));
    }

    print_output(cpu_infos, output_opts);

    Ok(())
}

fn print_output(infos: CpuInfos, out_opts: OutputOptions) {
    if out_opts.json {
        println!("{}", infos.to_json());
        return;
    }

    // Recursive function to print nested CpuInfo entries
    fn print_entries(entries: Vec<CpuInfo>, depth: usize, _out_opts: &OutputOptions) {
        let indent = "  ".repeat(depth);
        for entry in entries {
            // TODO: Align `data` values to form a column
            println!("{}{}: {}", indent, entry.field, entry.data);
            print_entries(entry.children, depth + 1, _out_opts);
        }
    }

    print_entries(infos.lscpu, 0, &out_opts);
}

fn find_cpuinfo_value(contents: &str, key: &str) -> Option<String> {
    let pattern = format!(r"^{}\s+:\s+(.*)$", key);
    let re = RegexBuilder::new(pattern.as_str())
        .multi_line(true)
        .build()
        .unwrap();

    let value = re
        .captures_iter(contents)
        .next()
        .map(|cap| cap[1].to_string());
    value
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
                .short('J')
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
