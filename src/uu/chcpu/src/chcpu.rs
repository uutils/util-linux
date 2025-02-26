// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod errors;
mod sysfs;

use core::str;
use std::str::FromStr;

use clap::builder::{EnumValueParser, PossibleValue};
use clap::{Arg, ArgAction, ArgGroup, Command, ValueEnum, crate_version};
use uucore::{error::UResult, format_usage, help_about, help_usage};

use crate::sysfs::{CpuList, DispatchMode};

mod options {
    pub static ENABLE: &str = "enable";
    pub static DISABLE: &str = "disable";
    pub static CONFIGURE: &str = "configure";
    pub static DECONFIGURE: &str = "deconfigure";
    pub static CPU_LIST: &str = "cpu-list";
    pub static DISPATCH: &str = "dispatch";
    pub static MODE: &str = "mode";
    pub static RESCAN: &str = "rescan";
}

const ABOUT: &str = help_about!("chcpu.md");
const USAGE: &str = help_usage!("chcpu.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args = uu_app().try_get_matches_from_mut(args)?;

    let sysfs_cpu = sysfs::SysFSCpu::open()?;

    if args.contains_id(options::ENABLE) {
        let cpu_list = args
            .get_one::<CpuList>(options::ENABLE)
            .expect("cpu-list is required");

        let mut enabled_cpu_list = sysfs_cpu.enabled_cpu_list().ok();

        cpu_list.run(&mut move |cpu_index| {
            sysfs_cpu.enable_cpu(enabled_cpu_list.as_mut(), cpu_index, true)
        })?;
    } else if args.contains_id(options::DISABLE) {
        let cpu_list = args
            .get_one::<CpuList>(options::DISABLE)
            .expect("cpu-list is required");

        let mut enabled_cpu_list = sysfs_cpu.enabled_cpu_list().ok();

        cpu_list.run(&mut move |cpu_index| {
            sysfs_cpu.enable_cpu(enabled_cpu_list.as_mut(), cpu_index, false)
        })?;
    } else if args.contains_id(options::CONFIGURE) {
        let cpu_list = args
            .get_one::<CpuList>(options::CONFIGURE)
            .expect("cpu-list is required");

        let enabled_cpu_list = sysfs_cpu.enabled_cpu_list().ok();

        cpu_list.run(&mut move |cpu_index| {
            sysfs_cpu.configure_cpu(enabled_cpu_list.as_ref(), cpu_index, true)
        })?;
    } else if args.contains_id(options::DECONFIGURE) {
        let cpu_list = args
            .get_one::<CpuList>(options::DECONFIGURE)
            .expect("cpu-list is required");

        let enabled_cpu_list = sysfs_cpu.enabled_cpu_list().ok();

        cpu_list.run(&mut move |cpu_index| {
            sysfs_cpu.configure_cpu(enabled_cpu_list.as_ref(), cpu_index, false)
        })?;
    } else if args.contains_id(options::DISPATCH) {
        let dispatch_mode = args
            .get_one::<DispatchMode>(options::DISPATCH)
            .expect("mode is required");

        sysfs_cpu.set_dispatch_mode(*dispatch_mode)?;
    } else if args.get_flag(options::RESCAN) {
        sysfs_cpu.rescan_cpus()?;
    } else {
        unimplemented!();
    }

    Ok(())
}

impl ValueEnum for DispatchMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Horizontal, Self::Vertical]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::Horizontal => {
                PossibleValue::new("horizontal").help("workload spread across all available CPUs")
            }
            Self::Vertical => {
                PossibleValue::new("vertical").help("workload concentrated on few CPUs")
            }
        })
    }
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new(options::ENABLE)
                .short('e')
                .long(options::ENABLE)
                .value_name(options::CPU_LIST)
                .value_parser(CpuList::from_str)
                .action(ArgAction::Set)
                .help("enable CPUs"),
        )
        .arg(
            Arg::new(options::DISABLE)
                .short('d')
                .long(options::DISABLE)
                .value_name(options::CPU_LIST)
                .value_parser(CpuList::from_str)
                .action(ArgAction::Set)
                .help("disable CPUs"),
        )
        .arg(
            Arg::new(options::CONFIGURE)
                .short('c')
                .long(options::CONFIGURE)
                .value_name(options::CPU_LIST)
                .value_parser(CpuList::from_str)
                .action(ArgAction::Set)
                .help("configure CPUs"),
        )
        .arg(
            Arg::new(options::DECONFIGURE)
                .short('g')
                .long(options::DECONFIGURE)
                .value_name(options::CPU_LIST)
                .value_parser(CpuList::from_str)
                .action(ArgAction::Set)
                .help("deconfigure CPUs"),
        )
        .arg(
            Arg::new(options::DISPATCH)
                .short('p')
                .long(options::DISPATCH)
                .value_name(options::MODE)
                .value_parser(EnumValueParser::<DispatchMode>::new())
                .action(ArgAction::Set)
                .help("set dispatching mode"),
        )
        .arg(
            Arg::new(options::RESCAN)
                .short('r')
                .long(options::RESCAN)
                .action(ArgAction::SetTrue)
                .help("trigger rescan of CPUs"),
        )
        .group(
            ArgGroup::new("control-group")
                .args([
                    options::ENABLE,
                    options::DISABLE,
                    options::CONFIGURE,
                    options::DECONFIGURE,
                ])
                .multiple(false)
                .conflicts_with_all(["dispatch-group", "rescan-group"]),
        )
        .group(
            ArgGroup::new("dispatch-group")
                .args([options::DISPATCH])
                .multiple(false)
                .conflicts_with_all(["control-group", "rescan-group"]),
        )
        .group(
            ArgGroup::new("rescan-group")
                .args([options::RESCAN])
                .multiple(false)
                .conflicts_with_all(["control-group", "dispatch-group"]),
        )
        .after_help(
            "<cpu-list> is one or more elements separated by commas. \
             Each element is either a positive integer (e.g., 3), \
             or an inclusive range of positive integers (e.g., 0-5). \
             For example, 0,2,7,10-13 refers to CPUs whose addresses are: 0, 2, 7, 10, 11, 12, and 13.",
        )
}
