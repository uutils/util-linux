// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use regex::RegexBuilder;
use serde::Serialize;
use std::{cmp, collections::HashMap, fs};
use uucore::{error::UResult, format_usage, help_about, help_usage};

mod options {
    pub const HEX: &str = "hex";
    pub const JSON: &str = "json";
}

mod sysfs;

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

impl CpuInfo {
    fn new(field: &str, data: &str, children: Option<Vec<CpuInfo>>) -> Self {
        Self {
            field: field.to_string(),
            data: data.to_string(),
            children: children.unwrap_or_default(),
        }
    }

    fn add_child(&mut self, child: Self) {
        self.children.push(child);
    }
}

impl CpuInfos {
    fn new() -> CpuInfos {
        CpuInfos {
            lscpu: Vec::<CpuInfo>::new(),
        }
    }

    fn push(&mut self, cpu_info: CpuInfo) {
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

    let output_opts = OutputOptions {
        _hex: matches.get_flag(options::HEX),
        json: matches.get_flag(options::JSON),
    };

    let mut cpu_infos = CpuInfos::new();

    let mut arch_info = CpuInfo::new("Architecture", &get_architecture(), None);

    // TODO: We just silently ignore failures to read `/proc/cpuinfo` currently and treat it as empty
    // Perhaps a better solution should be put in place, but what?
    let contents = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();

    if let Some(addr_sizes) = find_cpuinfo_value(&contents, "address sizes") {
        arch_info.add_child(CpuInfo::new("Address sizes", &addr_sizes, None))
    }

    if let Some(byte_order) = sysfs::read_cpu_byte_order() {
        arch_info.add_child(CpuInfo::new("Byte Order", byte_order, None));
    }

    cpu_infos.push(arch_info);

    let cpu_topology = sysfs::CpuTopology::new();
    let mut cores_info = CpuInfo::new("CPU(s)", &format!("{}", cpu_topology.cpus.len()), None);

    cores_info.add_child(CpuInfo::new(
        "On-line CPU(s) list",
        &sysfs::read_online_cpus(),
        None,
    ));

    cpu_infos.push(cores_info);

    // TODO: This is currently quite verbose and doesn't strictly respect the hierarchy of `/proc/cpuinfo` contents
    // ie. the file might contain multiple sections, each with their own vendor_id/model name etc. but right now
    // we're just taking whatever our regex matches first and using that
    if let Some(vendor) = find_cpuinfo_value(&contents, "vendor_id") {
        let mut vendor_info = CpuInfo::new("Vendor ID", &vendor, None);

        if let Some(model_name) = find_cpuinfo_value(&contents, "model name") {
            let mut model_name_info = CpuInfo::new("Model name", &model_name, None);

            if let Some(family) = find_cpuinfo_value(&contents, "cpu family") {
                model_name_info.add_child(CpuInfo::new("CPU Family", &family, None));
            }

            if let Some(model) = find_cpuinfo_value(&contents, "model") {
                model_name_info.add_child(CpuInfo::new("Model", &model, None));
            }

            let socket_count = &cpu_topology.socket_count();
            let core_count = &cpu_topology.core_count();
            model_name_info.add_child(CpuInfo::new(
                "Core(s) per socket",
                &(core_count / socket_count).to_string(),
                None,
            ));
            model_name_info.add_child(CpuInfo::new("Socket(s)", &socket_count.to_string(), None));

            if let Some(freq_boost_enabled) = sysfs::read_freq_boost_state() {
                let s = if freq_boost_enabled {
                    "enabled"
                } else {
                    "disabled"
                };
                model_name_info.add_child(CpuInfo::new("Frequency boost", s, None));
            }

            vendor_info.add_child(model_name_info);
        }

        cpu_infos.push(vendor_info);
    }

    if let Some(cache_info) = calculate_cache_totals(cpu_topology.cpus) {
        cpu_infos.push(cache_info);
    }

    let vulns = sysfs::read_cpu_vulnerabilities();
    if !vulns.is_empty() {
        let mut vuln_info = CpuInfo::new("Vulnerabilities", "", None);
        for vuln in vulns {
            vuln_info.add_child(CpuInfo::new(&vuln.name, &vuln.mitigation, None));
        }
        cpu_infos.push(vuln_info);
    }

    print_output(cpu_infos, output_opts);

    Ok(())
}

fn calculate_cache_totals(cpus: Vec<sysfs::Cpu>) -> Option<CpuInfo> {
    let mut by_levels: HashMap<String, Vec<&sysfs::CpuCache>> = HashMap::new();
    let all_caches: Vec<_> = cpus.iter().flat_map(|cpu| &cpu.caches).collect();

    if all_caches.is_empty() {
        return None;
    }

    for cache in all_caches {
        let type_suffix = match cache.typ {
            sysfs::CacheType::Instruction => "i",
            sysfs::CacheType::Data => "d",
            _ => "",
        };
        let level_key = format!("L{}{}", cache.level, type_suffix);

        if let Some(caches) = by_levels.get_mut(&level_key) {
            caches.push(cache);
        } else {
            by_levels.insert(level_key, vec![cache]);
        }
    }

    let mut cache_info = CpuInfo::new("Caches (sum of all)", "", None);

    for (level, caches) in by_levels.iter_mut() {
        // Cache instances that are shared across multiple CPUs should have the same `shared_cpu_map` value
        // Deduplicating the list on a per-level basic using the CPU map ensures that we don't count any shared caches multiple times
        caches.sort_by(|a, b| a.shared_cpu_map.cmp(&b.shared_cpu_map));
        caches.dedup_by_key(|c| &c.shared_cpu_map);

        let count = caches.len();
        let size_total = caches.iter().fold(0_u64, |acc, c| acc + c.size);
        cache_info.add_child(CpuInfo::new(
            level,
            // TODO: Format sizes using `KiB`, `MiB` etc.
            &format!("{} bytes ({} instances)", size_total, count),
            None,
        ));
    }

    // Make sure caches get printed in alphabetical order
    cache_info.children.sort_by(|a, b| a.field.cmp(&b.field));

    Some(cache_info)
}

fn print_output(infos: CpuInfos, out_opts: OutputOptions) {
    if out_opts.json {
        println!("{}", infos.to_json());
        return;
    }

    fn indentation(depth: usize) -> usize {
        // Indentation is 2 spaces per level, used in a few places, hence its own helper function
        depth * 2
    }

    // Recurses down the tree of entries and find the one with the "widest" field name (taking into account tree depth)
    fn get_max_field_width(info: &CpuInfo, depth: usize) -> usize {
        let max_child_width = info
            .children
            .iter()
            .map(|entry| get_max_field_width(entry, depth + 1))
            .max()
            .unwrap_or_default();

        let own_width = indentation(depth) + info.field.len();
        cmp::max(own_width, max_child_width)
    }

    fn print_entries(
        entries: &Vec<CpuInfo>,
        depth: usize,
        max_field_width: usize,
        _out_opts: &OutputOptions,
    ) {
        for entry in entries {
            let margin = indentation(depth);
            let padding = cmp::max(max_field_width - margin - entry.field.len(), 0);
            println!(
                "{}{}:{} {}",
                " ".repeat(margin),
                entry.field,
                " ".repeat(padding),
                entry.data
            );
            print_entries(&entry.children, depth + 1, max_field_width, _out_opts);
        }
    }

    // Used to align all values to the same column
    let max_field_width = infos
        .lscpu
        .iter()
        .map(|info| get_max_field_width(info, 0))
        .max()
        .unwrap();

    print_entries(&infos.lscpu, 0, max_field_width, &out_opts);
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

// TODO: This is non-exhaustive and assumes that compile-time arch is the same as runtime
// This is not always guaranteed to be the case, ie. you can run a x86 binary on a x86_64 machine
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
