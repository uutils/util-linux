// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_no_args_shows_usage() {
    new_ucmd!()
        .fails()
        .code_is(1)
        .stderr_contains("configure CPUs in a multi-processor system.");
}

#[test]
fn test_actions_mutually_exclusive() {
    new_ucmd!()
        .args(&["--enable", "0", "--disable", "1"])
        .fails()
        .code_is(1)
        .stderr_contains(
            "the argument '--enable <cpu-list>' cannot be used with '--disable <cpu-list>'",
        );
}

#[test]
fn test_cpu_list_range_out_of_order() {
    new_ucmd!()
        .args(&["--enable", "3-1"])
        .fails()
        .code_is(1)
        .stderr_contains("first element of CPU list range is greater than its last element");
}

#[test]
fn test_cpu_list_not_a_number() {
    new_ucmd!()
        .args(&["--enable", "a"])
        .fails()
        .code_is(1)
        .stderr_contains("CPU list element is not a positive number");
}

/// An empty argument splits into one empty element rather than zero elements, so it
/// is rejected as an unparsable element; `ChCpuError::EmptyCpuList` is unreachable.
#[test]
fn test_cpu_list_empty() {
    new_ucmd!()
        .args(&["--enable", ""])
        .fails()
        .code_is(1)
        .stderr_contains("CPU list element is not a positive number");
}

#[test]
fn test_dispatch_mode_unknown() {
    new_ucmd!()
        .args(&["--dispatch", "bogus"])
        .fails()
        .code_is(1)
        .stderr_contains("[possible values: horizontal, vertical]");
}

#[cfg(target_os = "linux")]
mod linux {
    use uutests::new_ucmd;

    /// CPU indices no kernel can have: `CONFIG_NR_CPUS` is orders of magnitude below
    /// these, so `/sys/devices/system/cpu/cpu9999[789]` never exists and `chcpu`
    /// rejects them before it would write anything. The two named here are
    /// deliberately not adjacent: a cpu-list coalesces touching ranges and reports
    /// each resulting range once, so an adjacent pair yields one diagnostic rather
    /// than two. `ABSENT_CPU - 1` supplies that adjacent case where it is wanted.
    const ABSENT_CPU: usize = 99999;
    const ABSENT_CPU_2: usize = 99997;

    /// Whether `cpuN` exposes an `online` attribute that reads `1`. `cpu0` commonly
    /// has no such attribute, so a CPU index cannot simply be assumed.
    fn cpu_is_online(index: usize) -> bool {
        std::fs::read_to_string(format!("/sys/devices/system/cpu/cpu{index}/online"))
            .is_ok_and(|state| state.trim() == "1")
    }

    /// First CPU exposing an `online` attribute that reads `1`, or `None` where no
    /// CPU is hot-pluggable. Not simply the first online CPU: `cpu0` is online on
    /// every running system yet commonly has no such attribute, so a CPU index
    /// cannot be assumed.
    fn first_online_cpu() -> Option<usize> {
        (0..1024).find(|index| cpu_is_online(*index))
    }

    /// Highest index in `/sys/devices/system/cpu/possible`, which is where the walk
    /// stops. `None` unless every element parses, because the binary parses that
    /// file all-or-nothing: a helper that salvaged a bound from a list the binary
    /// rejects would report a stop the binary does not have, and the tests guarded
    /// on it would then walk an unbounded range.
    fn max_possible_cpu() -> Option<usize> {
        let list = std::fs::read_to_string("/sys/devices/system/cpu/possible").ok()?;
        let mut max: Option<usize> = None;

        for element in list.trim().split(',') {
            let (first, last) = element.split_once('-').unwrap_or((element, element));
            let (first, last): (usize, usize) =
                (first.trim().parse().ok()?, last.trim().parse().ok()?);

            if first > last {
                return None;
            }

            max = Some(max.map_or(last, |max| max.max(last)));
        }

        max
    }

    /// Whether the walk stops below `index`. Indices above the stop cannot exist and
    /// are collapsed into one diagnostic instead of probed one at a time; where the
    /// stop is unknown the walk is unbounded, and neither the collapse nor the
    /// constant running time it buys holds.
    fn walk_stops_below(index: usize) -> bool {
        max_possible_cpu().is_some_and(|max| max < index)
    }

    #[test]
    fn test_absent_cpu_is_reported_once() {
        new_ucmd!()
            .arg("--enable")
            .arg(ABSENT_CPU.to_string())
            .fails_with_code(1)
            .stderr_only(format!("chcpu: CPU {ABSENT_CPU} does not exist\n"));
    }

    #[test]
    fn test_every_absent_cpu_is_reported_once() {
        new_ucmd!()
            .arg("--enable")
            .arg(format!("{ABSENT_CPU_2},{ABSENT_CPU}"))
            .fails_with_code(1)
            .stderr_only(format!(
                "chcpu: CPU {ABSENT_CPU_2} does not exist\nchcpu: CPU {ABSENT_CPU} does not exist\n"
            ));
    }

    /// A list mixing a usable CPU with an absent one must still exit 64 (partial
    /// success) and report the failure once. Enabling an already-enabled CPU returns
    /// before writing, so no privileges are needed and no CPU state changes, barring
    /// someone racing the test by offlining that CPU between the two reads.
    #[test]
    fn test_partial_success_reports_failure_once() {
        let Some(cpu) = first_online_cpu() else {
            eprintln!("skipping test_partial_success_reports_failure_once: no hot-pluggable CPU");
            return;
        };

        new_ucmd!()
            .arg("--enable")
            .arg(format!("{cpu},{ABSENT_CPU}"))
            .fails_with_code(64)
            .stdout_is(format!("CPU {cpu} is already enabled\n"))
            .stderr_is(format!("chcpu: CPU {ABSENT_CPU} does not exist\n"));
    }

    /// A cpu-list range is bounded only by the integer type, so walking it one index
    /// at a time took about five hours for this argv. Indices above the highest
    /// possible CPU cannot exist and are reported as one range, which makes it
    /// constant time. No index is walked, so no CPU state can change even as root.
    #[test]
    fn test_absent_cpu_range_is_reported_once() {
        if !walk_stops_below(ABSENT_CPU) {
            eprintln!(
                "skipping test_absent_cpu_range_is_reported_once: the walk is unbounded here, \
                 so this argv would run for hours"
            );
            return;
        }

        new_ucmd!()
            .arg("--enable")
            .arg(format!("{ABSENT_CPU}-4294967295"))
            .fails_with_code(1)
            .stderr_only(format!(
                "chcpu: CPUs {ABSENT_CPU}-4294967295 do not exist\n"
            ));
    }

    /// Adjacent elements coalesce into one range before the walk, so a pair above the
    /// bound takes the plural wording and names both, where the non-adjacent pair in
    /// [`test_every_absent_cpu_is_reported_once`] still yields two lines. Two indices
    /// is the narrowest range that is not reported as a single CPU.
    #[test]
    fn test_adjacent_absent_cpus_are_reported_as_one_range() {
        let first = ABSENT_CPU - 1;

        if !walk_stops_below(first) {
            eprintln!(
                "skipping test_adjacent_absent_cpus_are_reported_as_one_range: the walk is \
                 unbounded here, so each index is probed and reported separately"
            );
            return;
        }

        new_ucmd!()
            .arg("--enable")
            .arg(format!("{first},{ABSENT_CPU}"))
            .fails_with_code(1)
            .stderr_only(format!("chcpu: CPUs {first}-{ABSENT_CPU} do not exist\n"));
    }

    /// A range straddling the bound walks the part at or below it and collapses the
    /// rest, so the first index named is the one just past the bound. Runs only
    /// where the boundary CPU is already online, so `enable_cpu` returns before
    /// writing and no CPU state changes even as root.
    #[test]
    fn test_range_spanning_the_bound_reports_the_remainder_once() {
        let Some(max) = max_possible_cpu().filter(|index| cpu_is_online(*index)) else {
            eprintln!(
                "skipping test_range_spanning_the_bound_reports_the_remainder_once: \
                 the highest possible CPU is unknown or not online"
            );
            return;
        };

        new_ucmd!()
            .arg("--enable")
            .arg(format!("{max}-4294967295"))
            .fails_with_code(64)
            .stdout_is(format!("CPU {max} is already enabled\n"))
            .stderr_is(format!("chcpu: CPUs {}-4294967295 do not exist\n", max + 1));
    }
}
