// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

// Takes in a human-readable list of CPUs, and returns a list of indices parsed from that list
// These can come in the form of a plain range like `X-Y`, or a comma-separated ranges and indices ie. `1,3-4,7-8,10`
// Kernel docs with examples: https://www.kernel.org/doc/html/latest/admin-guide/cputopology.html
fn parse_cpu_list(list: &str) -> Vec<usize> {
    let mut out: Vec<usize> = vec![];

    if list.is_empty() {
        return out;
    }

    for part in list.trim().split(",") {
        if part.contains("-") {
            let bounds: Vec<_> = part.split("-").flat_map(|x| x.parse::<usize>()).collect();
            assert_eq!(bounds.len(), 2);
            for idx in bounds[0]..bounds[1] + 1 {
                out.push(idx)
            }
        } else {
            let idx = part.parse::<usize>().expect("Invalid CPU index value");
            out.push(idx);
        }
    }
    out
}

#[derive(Debug)]
struct Cpu(usize);

impl Cpu {
    fn get_path(&self) -> PathBuf {
        PathBuf::from(format!("/sys/devices/system/cpu/cpu{}", self.0))
    }

    // CPUs which are not hot-pluggable will not have the `/online` file in their directory
    fn is_hotpluggable(&self) -> bool {
        let path = self.get_path().join("online");
        path.exists()
    }

    fn is_online(&self) -> bool {
        if let Ok(state) = fs::read_to_string(self.get_path().join("online")) {
            match state.trim() {
                "0" => return false,
                "1" => return true,
                other => panic!("Unrecognized CPU online state: {}", other),
            }
        };

        // Just in case the caller forgot to check `is_hotpluggable` first,
        // instead of panicing that the file doesn't exist, return true
        // This is because a non-hotpluggable CPU is assumed to be always online
        true
    }

    fn enable(&self) {
        if !self.is_hotpluggable() {
            println!("CPU {} is not hot-pluggable", self.0);
            return;
        }

        if self.is_online() {
            println!("CPU {} is already enabled", self.0);
            return;
        }

        let result =
            File::create(self.get_path().join("online")).and_then(|mut f| f.write_all(b"1"));
        match result {
            Ok(_) => println!("CPU {} enabled", self.0),
            Err(e) => println!("CPU {} enable failed: {:#}", self.0, e.kind()),
        }
    }

    fn disable(&self) {
        if !self.is_hotpluggable() {
            println!("CPU {} is not hot-pluggable", self.0);
            return;
        }

        if !self.is_online() {
            println!("CPU {} is already disabled", self.0);
            return;
        }

        if get_online_cpus().len() == 1 {
            println!("CPU {} disable failed (last enabled CPU)", self.0);
            return;
        }

        let result =
            File::create(self.get_path().join("online")).and_then(|mut f| f.write_all(b"0"));
        match result {
            Ok(_) => println!("CPU {} disabled", self.0),
            Err(e) => println!("CPU {} disable failed: {:#}", self.0, e.kind()),
        }
    }
}

fn get_online_cpus() -> Vec<Cpu> {
    let cpu_list = fs::read_to_string("/sys/devices/system/cpu/online").unwrap();
    parse_cpu_list(&cpu_list).iter().map(|n| Cpu(*n)).collect()
}

fn enable_cpus(cpu_list: &str) {
    let to_enable = parse_cpu_list(cpu_list).into_iter().map(Cpu);

    for cpu in to_enable {
        cpu.enable();
    }
}

fn disable_cpus(cpu_list: &str) {
    let to_disable = parse_cpu_list(cpu_list).into_iter().map(Cpu);

    for cpu in to_disable {
        cpu.disable();
    }
}

fn configure_cpus(cpu_list: &str) {
    todo!("Configuring CPUs: {}", cpu_list);
}

fn deconfigure_cpus(cpu_list: &str) {
    todo!("Deconfiguring CPUs: {}", cpu_list);
}

fn set_dispatch_mode(mode: &str) {
    todo!("Setting dispatch mode to: {}", mode);
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    if let Some(cpu_list) = matches.get_one::<String>(options::ENABLE) {
        enable_cpus(cpu_list);
    }

    if let Some(cpu_list) = matches.get_one::<String>(options::DISABLE) {
        disable_cpus(cpu_list);
    }

    if let Some(cpu_list) = matches.get_one::<String>(options::CONFIGURE) {
        configure_cpus(cpu_list);
    }

    if let Some(cpu_list) = matches.get_one::<String>(options::DECONFIGURE) {
        deconfigure_cpus(cpu_list);
    }

    if let Some(mode) = matches.get_one::<String>(options::DISPATCH) {
        set_dispatch_mode(mode);
    }

    Ok(())
}

mod options {
    pub const ENABLE: &str = "enable";
    pub const DISABLE: &str = "disable";
    pub const CONFIGURE: &str = "configure";
    pub const DECONFIGURE: &str = "deconfigure";
    pub const DISPATCH: &str = "dispatch";
}

const ABOUT: &str = help_about!("chcpu.md");
const USAGE: &str = help_usage!("chcpu.md");

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::ENABLE)
                .short('e')
                .long("enable")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::DISABLE)
                .short('d')
                .long("disable")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::CONFIGURE)
                .short('c')
                .long("configure")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::DECONFIGURE)
                .short('g')
                .long("deconfigure")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::DISPATCH)
                .short('p')
                .long("dispatch")
                .value_name("mode")
                .action(ArgAction::Set),
        )
        .group(
            ArgGroup::new("action")
                .args(vec![
                    options::ENABLE,
                    options::DISABLE,
                    options::CONFIGURE,
                    options::DECONFIGURE,
                    options::DISPATCH,
                ])
                .multiple(false) // These 5 are mutually exclusive
                .required(true),
        )
}
