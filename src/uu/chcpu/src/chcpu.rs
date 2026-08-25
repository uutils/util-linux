// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.
#![cfg_attr(not(unix), allow(dead_code))]

mod errors;
#[cfg(unix)]
mod sysfs;

use std::str::FromStr;
use std::{fmt, str};

use clap::builder::{EnumValueParser, PossibleValue};
use clap::{Arg, ArgAction, ArgGroup, Command, ValueEnum, crate_version};
use rangemap::RangeInclusiveSet;
use uucore::{error::UResult, format_usage, help_about, help_usage};

use crate::errors::ChCpuError;

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

    if args.contains_id(options::ENABLE) {
        let cpu_list = args
            .get_one::<CpuList>(options::ENABLE)
            .expect("cpu-list is required");

        enable_cpu(cpu_list, true)?;
    } else if args.contains_id(options::DISABLE) {
        let cpu_list = args
            .get_one::<CpuList>(options::DISABLE)
            .expect("cpu-list is required");

        enable_cpu(cpu_list, false)?;
    } else if args.contains_id(options::CONFIGURE) {
        let cpu_list = args
            .get_one::<CpuList>(options::CONFIGURE)
            .expect("cpu-list is required");

        configure_cpu(cpu_list, true)?;
    } else if args.contains_id(options::DECONFIGURE) {
        let cpu_list = args
            .get_one::<CpuList>(options::DECONFIGURE)
            .expect("cpu-list is required");

        configure_cpu(cpu_list, false)?;
    } else if args.contains_id(options::DISPATCH) {
        let dispatch_mode = args
            .get_one::<DispatchMode>(options::DISPATCH)
            .expect("mode is required");

        set_dispatch_mode(*dispatch_mode)?;
    } else if args.get_flag(options::RESCAN) {
        rescan_cpus()?;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
enum DispatchMode {
    Horizontal = 0,
    Vertical = 1,
}

impl fmt::Display for DispatchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Horizontal => write!(f, "horizontal"),
            Self::Vertical => write!(f, "vertical"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpuList(RangeInclusiveSet<usize>);

impl CpuList {
    /// The highest index in the list. `RangeInclusiveSet` keeps its ranges
    /// coalesced and ordered, so the last one holds it.
    pub(crate) fn max_index(&self) -> Option<usize> {
        self.0.last().map(|range| *range.end())
    }

    /// A failure on one CPU must not stop the remaining ones, so failures are
    /// reported here and reflected in the exit code instead of being returned:
    /// returning one would let `uucore` print it a second time.
    ///
    /// `max_cpu_index` bounds the walk. A cpu-list range is only as wide as the
    /// integer type, so without it `--enable 0-4294967295` spends hours calling
    /// `f` on indices no kernel can have. Indices above the bound cannot exist, so
    /// they are reported one range at a time rather than one index at a time.
    /// `None` walks everything, as it did before the bound existed; call through
    /// `walk_cpu_list` rather than passing it, so the bound cannot be dropped.
    fn run(
        &self,
        max_cpu_index: Option<usize>,
        f: &mut dyn FnMut(usize) -> Result<(), ChCpuError>,
    ) {
        let mut success_occurred = false;
        let mut failure_occurred = false;

        for range in self.0.iter() {
            let (first, last) = (*range.start(), *range.end());
            let walked_last = max_cpu_index.map_or(last, |max| max.min(last));

            // Empty when the whole range sits above the bound.
            for cpu_index in first..=walked_last {
                match f(cpu_index) {
                    Ok(()) => success_occurred = true,
                    Err(err) => {
                        uucore::show!(err);
                        failure_occurred = true;
                    }
                }
            }

            if walked_last < last {
                // The comparison guarantees the increment stays in range.
                uucore::show!(ChCpuError::absent_cpus(first.max(walked_last + 1), last));
                failure_occurred = true;
            }
        }

        if success_occurred && failure_occurred {
            uucore::error::set_exit_code(64); // Partial success.
        }
    }
}

impl TryFrom<&[u8]> for CpuList {
    type Error = ChCpuError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let set: RangeInclusiveSet<usize> = bytes
            .split(|&b| b == b',')
            .map(|element| {
                // Parsing: ...,element,...
                let mut iter = element.splitn(2, |&b| b == b'-').map(<[u8]>::trim_ascii);
                let first = iter.next();
                (first, iter.next())
            })
            .map(|(first, last)| {
                let first = first.ok_or(ChCpuError::EmptyCpuList)?;
                let first: usize = str::from_utf8(first)
                    .map_err(|_r| ChCpuError::CpuSpecNotPositiveInteger)?
                    .parse()
                    .map_err(|_r| ChCpuError::CpuSpecNotPositiveInteger)?;

                if let Some(last) = last {
                    // Parsing: ...,first-last,...
                    let last = str::from_utf8(last)
                        .map_err(|_r| ChCpuError::CpuSpecNotPositiveInteger)?
                        .parse()
                        .map_err(|_r| ChCpuError::CpuSpecNotPositiveInteger)?;

                    if first <= last {
                        Ok(first..=last)
                    } else {
                        Err(ChCpuError::CpuSpecFirstAfterLast)
                    }
                } else {
                    Ok(first..=first) // Parsing: ...,first,...
                }
            })
            .collect::<Result<_, _>>()?;

        if set.is_empty() {
            Err(ChCpuError::EmptyCpuList)
        } else {
            Ok(Self(set))
        }
    }
}

impl FromStr for CpuList {
    type Err = ChCpuError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.as_bytes())
    }
}

/// Walks `cpu_list`, bounded by what the machine can have. The bound is taken here
/// rather than passed in so that no operation can be added that omits it: a walk
/// given no bound steps through every index the integer type allows, which is the
/// hours-long walk the bound exists to prevent.
#[cfg(unix)]
fn walk_cpu_list(
    sysfs_cpu: &sysfs::SysFSCpu,
    cpu_list: &CpuList,
    f: &mut dyn FnMut(usize) -> Result<(), ChCpuError>,
) {
    cpu_list.run(sysfs_cpu.max_possible_cpu_index(), f);
}

#[cfg(unix)]
fn enable_cpu(cpu_list: &CpuList, enable: bool) -> Result<(), ChCpuError> {
    let sysfs_cpu = sysfs::SysFSCpu::open()?;

    let mut enabled_cpu_list = sysfs_cpu.enabled_cpu_list().ok();

    walk_cpu_list(&sysfs_cpu, cpu_list, &mut |cpu_index| {
        sysfs_cpu.enable_cpu(enabled_cpu_list.as_mut(), cpu_index, enable)
    });

    Ok(())
}

#[cfg(not(unix))]
fn enable_cpu(_cpu_list: &CpuList, _enable: bool) -> Result<(), ChCpuError> {
    unimplemented!()
}

#[cfg(unix)]
fn configure_cpu(cpu_list: &CpuList, configure: bool) -> Result<(), ChCpuError> {
    let sysfs_cpu = sysfs::SysFSCpu::open()?;

    let enabled_cpu_list = sysfs_cpu.enabled_cpu_list().ok();

    walk_cpu_list(&sysfs_cpu, cpu_list, &mut |cpu_index| {
        sysfs_cpu.configure_cpu(enabled_cpu_list.as_ref(), cpu_index, configure)
    });

    Ok(())
}

#[cfg(not(unix))]
fn configure_cpu(_cpu_list: &CpuList, _configure: bool) -> Result<(), ChCpuError> {
    unimplemented!()
}

#[cfg(unix)]
fn set_dispatch_mode(dispatch_mode: DispatchMode) -> Result<(), ChCpuError> {
    sysfs::SysFSCpu::open()?.set_dispatch_mode(dispatch_mode)
}

#[cfg(not(unix))]
fn set_dispatch_mode(_dispatch_mode: DispatchMode) -> Result<(), ChCpuError> {
    unimplemented!()
}

#[cfg(unix)]
fn rescan_cpus() -> Result<(), ChCpuError> {
    sysfs::SysFSCpu::open()?.rescan_cpus()
}

#[cfg(not(unix))]
fn rescan_cpus() -> Result<(), ChCpuError> {
    unimplemented!()
}
