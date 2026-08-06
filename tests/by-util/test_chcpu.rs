// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
mod linux {
    use uutests::new_ucmd;

    /// CPU indices no kernel can have: `CONFIG_NR_CPUS` is orders of magnitude below
    /// these, so `/sys/devices/system/cpu/cpu9999[89]` never exists and `chcpu`
    /// rejects them before it would write anything.
    const ABSENT_CPU: &str = "99999";
    const ABSENT_CPU_2: &str = "99998";

    /// First CPU exposing an `online` attribute that reads `1`, or `None` where no
    /// CPU is hot-pluggable. `cpu0` commonly has no such attribute, so a CPU index
    /// cannot simply be assumed.
    fn first_online_cpu() -> Option<usize> {
        (0..1024).find(|index| {
            std::fs::read_to_string(format!("/sys/devices/system/cpu/cpu{index}/online"))
                .is_ok_and(|state| state.trim() == "1")
        })
    }

    #[test]
    fn test_absent_cpu_is_reported_once() {
        new_ucmd!()
            .arg("--enable")
            .arg(ABSENT_CPU)
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
}
