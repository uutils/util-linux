// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]

use clap::{crate_version, Arg, ArgAction, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

#[cfg(target_os = "linux")]
use nix::errno::Errno;
#[cfg(target_os = "linux")]
use nix::sched::{sched_getaffinity, sched_setaffinity, CpuSet};
#[cfg(target_os = "linux")]
use nix::unistd::Pid;
#[cfg(target_os = "linux")]
use std::path::Path;

mod options {
    pub const PID: &str = "pid";
    pub const CPU_LIST: &str = "cpu-list";
    pub const ALL_TASKS: &str = "all-tasks";
    pub const ARGS: &str = "args";
}

const ABOUT: &str = help_about!("taskset.md");
const USAGE: &str = help_usage!("taskset.md");

#[cfg(target_os = "linux")]
#[derive(Debug, thiserror::Error)]
enum TasksetError {
    #[error("invalid hex mask: '{0}'")]
    InvalidHexMask(String),
    #[error("invalid CPU list: '{0}'")]
    InvalidCpuList(String),
    #[error("CPU index out of range: {0}")]
    CpuIndexOutOfRange(usize),
    #[error("invalid PID: '{0}'")]
    InvalidPid(String),
    #[error("failed to get pid {0}'s affinity: {1}")]
    GetAffinityFailed(Pid, nix::Error),
    #[error("failed to set pid {0}'s affinity: {1}")]
    SetAffinityFailed(Pid, nix::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

#[cfg(target_os = "linux")]
impl uucore::error::UError for TasksetError {
    fn code(&self) -> i32 {
        1
    }
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::PID)
                .short('p')
                .long("pid")
                .action(ArgAction::SetTrue)
                .help("operate on an existing PID"),
        )
        .arg(
            Arg::new(options::CPU_LIST)
                .short('c')
                .long("cpu-list")
                .action(ArgAction::SetTrue)
                .help("display and specify CPUs in list format"),
        )
        .arg(
            Arg::new(options::ALL_TASKS)
                .short('a')
                .long("all-tasks")
                .action(ArgAction::SetTrue)
                .help("operate on all tasks (threads) of the given PID"),
        )
        .arg(
            Arg::new(options::ARGS)
                .action(ArgAction::Append)
                .trailing_var_arg(true)
                .num_args(0..),
        )
}

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    use std::os::unix::process::CommandExt;
    use uucore::error::{FromIo, USimpleError};

    let matches = uu_app().try_get_matches_from(args)?;

    let use_list = matches.get_flag(options::CPU_LIST);
    let pid_mode = matches.get_flag(options::PID);
    let all_tasks = matches.get_flag(options::ALL_TASKS);

    let positional: Vec<String> = matches
        .get_many::<String>(options::ARGS)
        .unwrap_or_default()
        .cloned()
        .collect();

    let label = if use_list {
        "affinity list"
    } else {
        "affinity mask"
    };
    let parse = if use_list {
        parse_cpu_list
    } else {
        parse_hex_mask
    };
    let fmt = if use_list {
        format_cpu_list
    } else {
        format_hex_mask
    };

    if pid_mode {
        match positional.as_slice() {
            [] => {
                return Err(USimpleError::new(1, "missing argument: PID"));
            }
            [pid_str] => {
                let pid = parse_pid(pid_str)?;
                let pids = if all_tasks {
                    get_task_pids(pid)?
                } else {
                    vec![pid]
                };
                for p in pids {
                    let set = match sched_getaffinity(p) {
                        Ok(s) => s,
                        // When operating on all tasks, a thread may exit between the time
                        // we read its PID from /proc and the time we call sched_getaffinity
                        // or sched_setaffinity. Ignore ESRCH (No such process) in this case.
                        Err(Errno::ESRCH) if all_tasks => continue,
                        Err(e) => return Err(TasksetError::GetAffinityFailed(p, e).into()),
                    };
                    println!("pid {}'s current {}: {}", p, label, fmt(&set));
                }
            }
            [mask_str, pid_str, ..] => {
                let pid = parse_pid(pid_str)?;
                let new_set = parse(mask_str)?;
                let pids = if all_tasks {
                    get_task_pids(pid)?
                } else {
                    vec![pid]
                };
                for p in pids {
                    let old_set = match sched_getaffinity(p) {
                        Ok(s) => s,
                        Err(Errno::ESRCH) if all_tasks => continue,
                        Err(e) => return Err(TasksetError::GetAffinityFailed(p, e).into()),
                    };
                    println!("pid {}'s current {}: {}", p, label, fmt(&old_set));
                    match sched_setaffinity(p, &new_set) {
                        Ok(_) => {}
                        // Thread exited between getaffinity and setaffinity.
                        // We already printed "current" for this thread; "new"
                        // is skipped. This is an unavoidable TOCTOU race.
                        Err(Errno::ESRCH) if all_tasks => continue,
                        Err(e) => return Err(TasksetError::SetAffinityFailed(p, e).into()),
                    }
                    // Print the requested mask, not a re-read from the kernel.
                    // This matches util-linux behavior. Note: if the mask
                    // contained out-of-range CPUs, the kernel silently drops
                    // them, so the printed value may not reflect reality.
                    println!("pid {}'s new {}: {}", p, label, fmt(&new_set));
                }
            }
        }
    } else {
        // Note: --all-tasks is silently ignored in exec mode (no -p). This is
        // surprising, but matches util-linux behavior: there is no existing
        // process to enumerate threads for; the affinity is set on the current
        // process before exec, which has only one thread.
        match positional.as_slice() {
            [] | [_] => {
                return Err(USimpleError::new(1, "mask/list and command required"));
            }
            [mask_str, command, rest @ ..] => {
                let set = parse(mask_str)?;
                // Pid::from_raw(0) means "this process" to sched_setaffinity.
                // The affinity is inherited across exec, so the launched
                // command starts with it already set.
                sched_setaffinity(Pid::from_raw(0), &set)
                    .map_err(|e| TasksetError::SetAffinityFailed(Pid::from_raw(0), e))?;
                // exec() replaces the current process image and never returns
                // on success; if it returns at all, it's an error.
                let err = std::process::Command::new(command).args(rest).exec();
                return Err(err.map_err_context(|| format!("failed to execute {command}")));
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;
    Err(uucore::error::USimpleError::new(
        1,
        "`taskset` is available only on Linux.",
    ))
}

#[cfg(target_os = "linux")]
fn parse_pid(s: &str) -> Result<Pid, TasksetError> {
    // util-linux allows leading whitespace but rejects trailing whitespace
    // (probably because it uses strtol). We deviate from this
    // by rejecting both leading and trailing whitespace.
    // TODO: decide if we want to be bug/wart compatible with util-linux here.
    s.parse::<i32>()
        .map(Pid::from_raw)
        .map_err(|_| TasksetError::InvalidPid(s.to_owned()))
}

#[cfg(target_os = "linux")]
fn parse_hex_mask(s: &str) -> Result<CpuSet, TasksetError> {
    // Comma-separated groups (e.g. "00000001,00000000") are a valid format
    // produced by /proc/<pid>/status; strip commas before parsing.
    let s_clean = s.replace(',', "");
    let hex = s_clean
        .strip_prefix("0x")
        .or_else(|| s_clean.strip_prefix("0X"))
        .unwrap_or(&s_clean);

    let hex = hex.trim_start_matches('0');
    if hex.is_empty() {
        return Ok(CpuSet::new());
    }

    let mut set = CpuSet::new();
    for (i, c) in hex.chars().rev().enumerate() {
        let nibble = c
            .to_digit(16)
            .ok_or_else(|| TasksetError::InvalidHexMask(s.to_owned()))?;
        for bit in 0..4u32 {
            if nibble & (1 << bit) != 0 {
                let cpu = i * 4 + bit as usize;
                set.set(cpu)
                    .map_err(|_| TasksetError::CpuIndexOutOfRange(cpu))?;
            }
        }
    }
    Ok(set)
}

#[cfg(target_os = "linux")]
fn parse_cpu_list(s: &str) -> Result<CpuSet, TasksetError> {
    let invalid = || TasksetError::InvalidCpuList(s.to_owned());
    let mut set = CpuSet::new();
    // util-linux rejects whitespace or empty elements in CPU lists.
    for element in s.split(',') {
        let (range_part, stride) = if let Some((range, stride_str)) = element.split_once(':') {
            let stride: usize = stride_str.parse().map_err(|_| invalid())?;
            if stride == 0 {
                return Err(invalid());
            }
            (range, stride)
        } else {
            (element, 1)
        };

        if let Some((start_str, end_str)) = range_part.split_once('-') {
            let start: usize = start_str.parse().map_err(|_| invalid())?;
            let end: usize = end_str.parse().map_err(|_| invalid())?;
            if start > end {
                return Err(invalid());
            }
            for cpu in (start..=end).step_by(stride) {
                set.set(cpu)
                    .map_err(|_| TasksetError::CpuIndexOutOfRange(cpu))?;
            }
        } else {
            let cpu: usize = range_part.parse().map_err(|_| invalid())?;
            set.set(cpu)
                .map_err(|_| TasksetError::CpuIndexOutOfRange(cpu))?;
        }
    }
    Ok(set)
}

#[cfg(target_os = "linux")]
fn format_hex_mask(set: &CpuSet) -> String {
    let max_cpu = (0..CpuSet::count()).rev().find(|&i| set.is_set(i).unwrap());

    let num_nibbles = max_cpu.map(|m| m / 4 + 1).unwrap_or(1);

    // num_nibbles is at least 1 (from unwrap_or(1) above), so the loop
    // always executes at least once and result is never empty.
    let mut result = String::new();
    for i in (0..num_nibbles).rev() {
        let mut nibble: u8 = 0;
        for bit in 0..4 {
            let cpu = i * 4 + bit;
            if cpu < CpuSet::count() && set.is_set(cpu).unwrap() {
                nibble |= 1 << bit;
            }
        }
        // nibble is 0–15, always a valid hex digit
        result.push(char::from_digit(nibble as u32, 16).unwrap());
    }
    result
}

#[cfg(target_os = "linux")]
fn format_cpu_list(set: &CpuSet) -> String {
    let cpus: Vec<usize> = (0..CpuSet::count())
        .filter(|&i| set.is_set(i).unwrap())
        .collect();

    if cpus.is_empty() {
        return String::new();
    }

    let mut ranges: Vec<String> = Vec::new();
    let mut i = 0;

    while i < cpus.len() {
        let a = cpus[i];
        if i + 1 == cpus.len() {
            ranges.push(a.to_string());
            break;
        }

        let step = cpus[i + 1] - a;
        let mut j = i + 1;
        while j + 1 < cpus.len() && cpus[j + 1] - cpus[j] == step {
            j += 1;
        }
        let b = cpus[j];

        if b - a == step {
            // Two-element run: emit a as singleton and put b back for the
            // next iteration. This naturally handles the singleton-before-stride
            // case: e.g. [0,1,3,5] → "0,1-5:2" not "0-1,3,5".
            ranges.push(a.to_string());
            i += 1;
        } else if step == 1 {
            ranges.push(format!("{}-{}", a, b));
            i = j + 1;
        } else {
            ranges.push(format!("{}-{}:{}", a, b, step));
            i = j + 1;
        }
    }

    ranges.join(",")
}

/// Read task PIDs from a directory of numeric-named entries.
///
/// In production use, `task_dir` is `/proc/<pid>/task`, which the Linux kernel
/// populates with one subdirectory per thread, each named by its TID (thread
/// ID). See proc(5) for details.
///
/// Accepts any path so tests can pass a tempdir instead of /proc/<pid>/task.
#[cfg(target_os = "linux")]
fn read_task_pids(task_dir: &Path) -> Result<Vec<Pid>, TasksetError> {
    let pids = std::fs::read_dir(task_dir)?
        .map(|entry| {
            let entry = entry?;
            let name = entry.file_name();
            let tid: i32 = name
                .to_str()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| std::io::Error::other(format!("unexpected task entry: {name:?}")))?;
            Ok(Pid::from_raw(tid))
        })
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    Ok(pids)
}

#[cfg(target_os = "linux")]
fn get_task_pids(pid: Pid) -> Result<Vec<Pid>, TasksetError> {
    read_task_pids(Path::new(&format!("/proc/{}/task", pid)))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pid_plain() {
        assert!(parse_pid("123").is_ok());
        assert!(parse_pid("abc").is_err());
    }

    #[test]
    fn test_parse_pid_whitespace() {
        // util-linux allows leading whitespace but rejects trailing
        // (probably because it uses strtol).
        // We intentionally deviate and reject both (see TODO in parse_pid).
        assert!(parse_pid(" 123").is_err());
        assert!(parse_pid("123 ").is_err());
    }

    #[test]
    fn test_parse_cpu_list_whitespace() {
        // util-linux rejects whitespace within each element.
        assert!(parse_cpu_list("0, 1").is_err());
    }

    #[test]
    fn test_parse_cpu_list_empty_elements() {
        // util-linux rejects trailing/double commas.
        assert!(parse_cpu_list("0,,1").is_err());
        assert!(parse_cpu_list("0,").is_err());
        assert!(parse_cpu_list(",0").is_err());
    }

    #[test]
    fn test_parse_hex_mask_with_prefix() {
        let set = parse_hex_mask("0x3").unwrap();
        assert!(set.is_set(0).unwrap());
        assert!(set.is_set(1).unwrap());
        assert!(!set.is_set(2).unwrap());
    }

    #[test]
    fn test_parse_hex_mask_with_commas() {
        // Comma-separated groups as produced by /proc/<pid>/status
        let set = parse_hex_mask("00000001,00000000").unwrap();
        assert!(set.is_set(32).unwrap());
        assert!(!set.is_set(0).unwrap());

        // Single group with comma still works
        let set = parse_hex_mask("0,0000003").unwrap();
        assert!(set.is_set(0).unwrap());
        assert!(set.is_set(1).unwrap());
        assert!(!set.is_set(2).unwrap());
    }

    #[test]
    fn test_parse_hex_mask_without_prefix() {
        let set = parse_hex_mask("f").unwrap();
        for i in 0..4 {
            assert!(set.is_set(i).unwrap());
        }
        assert!(!set.is_set(4).unwrap());
    }

    #[test]
    fn test_parse_hex_mask_uppercase_prefix() {
        let set = parse_hex_mask("0XFF").unwrap();
        for i in 0..8 {
            assert!(set.is_set(i).unwrap());
        }
    }

    #[test]
    fn test_parse_hex_mask_zero() {
        let set = parse_hex_mask("0x0").unwrap();
        for i in 0..8 {
            assert!(!set.is_set(i).unwrap());
        }
    }

    #[test]
    fn test_parse_hex_mask_invalid() {
        assert!(parse_hex_mask("0xgg").is_err());
        assert!(parse_hex_mask("xyz").is_err());
    }

    #[test]
    fn test_parse_cpu_list_single() {
        let set = parse_cpu_list("0").unwrap();
        assert!(set.is_set(0).unwrap());
        assert!(!set.is_set(1).unwrap());
    }

    #[test]
    fn test_parse_cpu_list_multiple() {
        let set = parse_cpu_list("0,2,4").unwrap();
        assert!(set.is_set(0).unwrap());
        assert!(!set.is_set(1).unwrap());
        assert!(set.is_set(2).unwrap());
        assert!(!set.is_set(3).unwrap());
        assert!(set.is_set(4).unwrap());
    }

    #[test]
    fn test_parse_cpu_list_range() {
        let set = parse_cpu_list("2-5").unwrap();
        assert!(!set.is_set(1).unwrap());
        for i in 2..=5 {
            assert!(set.is_set(i).unwrap());
        }
        assert!(!set.is_set(6).unwrap());
    }

    #[test]
    fn test_parse_cpu_list_stride() {
        let set = parse_cpu_list("1-10:2").unwrap();
        for cpu in [1usize, 3, 5, 7, 9] {
            assert!(set.is_set(cpu).unwrap(), "expected CPU {cpu} to be set");
        }
        for cpu in [0usize, 2, 4, 6, 8, 10] {
            assert!(
                !set.is_set(cpu).unwrap(),
                "expected CPU {cpu} to not be set"
            );
        }
    }

    #[test]
    fn test_parse_cpu_list_invalid_stride_zero() {
        assert!(parse_cpu_list("0-4:0").is_err());
    }

    #[test]
    fn test_parse_cpu_list_inverted_range() {
        assert!(parse_cpu_list("5-2").is_err());
    }

    #[test]
    fn test_format_hex_mask_basic() {
        let set = parse_hex_mask("3").unwrap();
        assert_eq!(format_hex_mask(&set), "3");
    }

    #[test]
    fn test_format_hex_mask_zero() {
        assert_eq!(format_hex_mask(&CpuSet::new()), "0");
    }

    #[test]
    fn test_format_hex_mask_round_trip() {
        for s in ["1", "3", "f", "ff", "deadbeef"] {
            let set = parse_hex_mask(s).unwrap();
            assert_eq!(format_hex_mask(&set), s, "round-trip failed for '{s}'");
        }
    }

    #[test]
    fn test_format_cpu_list_empty() {
        assert_eq!(format_cpu_list(&CpuSet::new()), "");
    }

    #[test]
    fn test_format_cpu_list_round_trip() {
        for s in ["0", "0-3", "0-2,5-7", "0-4:2", "1-9:2", "0-3,5-9:2"] {
            let set = parse_cpu_list(s).unwrap();
            assert_eq!(format_cpu_list(&set), s, "round-trip failed for '{s}'");
        }
    }

    #[test]
    fn test_format_cpu_list_stride_from_individual() {
        // Individual values that form a stride should produce stride notation
        let set = parse_cpu_list("0,2,4").unwrap();
        assert_eq!(format_cpu_list(&set), "0-4:2");

        let set = parse_cpu_list("1,3,5,7,9").unwrap();
        assert_eq!(format_cpu_list(&set), "1-9:2");
    }

    #[test]
    fn test_format_cpu_list_stride_too_short() {
        // A stride run of only 2 elements should not use stride notation
        let set = parse_cpu_list("0,2").unwrap();
        assert_eq!(format_cpu_list(&set), "0,2");
    }

    #[test]
    fn test_format_cpu_list_singleton_before_stride() {
        // A singleton followed by a stride run should not be consumed into
        // a 2-element consecutive range: [0,1,3,5] → "0,1-5:2" not "0-1,3,5"
        let set = parse_cpu_list("0,1-5:2").unwrap();
        assert_eq!(format_cpu_list(&set), "0,1-5:2");
    }

    #[test]
    fn test_read_task_pids() {
        let dir = tempfile::tempdir().unwrap();
        for tid in [100i32, 200, 300] {
            std::fs::create_dir(dir.path().join(tid.to_string())).unwrap();
        }
        let mut pids = read_task_pids(dir.path()).unwrap();
        pids.sort();
        assert_eq!(
            pids,
            vec![Pid::from_raw(100), Pid::from_raw(200), Pid::from_raw(300)]
        );
    }
}
