// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) EACCES EPERM getattr setattr setscheduler sched SIGPIPE tids nocapture niced setpriority getrlimit RLIMIT RTPRIO

use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[cfg(target_os = "linux")]
mod linux {
    use regex::Regex;
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::process::{CommandExt, ExitStatusExt};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::time::Duration;
    use uutests::at_and_ucmd;
    use uutests::new_ucmd;
    use uutests::util::{get_tests_binary, UCommand};

    /// A process id that cannot exist: pid_max is bounded well below this.
    const ABSENT_PID: &str = "2147483647";

    // -- targets ---------------------------------------------------------------

    /// A sleeping child this test owns and may change, never the test process:
    /// SCHED_IDLE and reset-on-fork are one-way doors for an unprivileged caller.
    struct Target(Child);

    impl Target {
        fn new() -> Self {
            let mut command = Command::new("sleep");
            command.arg("30");
            Self(
                plain(&mut command)
                    .spawn()
                    .expect("spawn sleep as SCHED_OTHER"),
            )
        }

        fn niced(nice: i32) -> Self {
            let mut command = Command::new("sleep");
            command.arg("30");
            Self(
                at_nice(plain(&mut command), nice)
                    .spawn()
                    .expect("spawn sleep as SCHED_OTHER"),
            )
        }

        fn pid(&self) -> i32 {
            self.0.id() as i32
        }

        fn arg(&self) -> String {
            self.pid().to_string()
        }
    }

    impl Drop for Target {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Start the command as SCHED_OTHER whatever the runner passes on; a runner
    /// under SCHED_IDLE cannot leave it unprivileged, so there the spawn fails.
    fn plain(command: &mut Command) -> &mut Command {
        // SAFETY: sched_setscheduler(2) is a plain syscall on the child
        // itself, safe between fork and exec; it reads `priority` only.
        unsafe {
            command.pre_exec(|| {
                let priority: libc::c_int = 0;
                let param = &priority as *const libc::c_int;
                if libc::syscall(libc::SYS_sched_setscheduler, 0, libc::SCHED_OTHER, param) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
        }
    }

    /// Set before the exec, so the process is niced by the time `spawn` returns.
    fn at_nice(command: &mut Command, nice: i32) -> &mut Command {
        // SAFETY: setpriority(2) is a plain syscall on the child itself,
        // safe between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if libc::setpriority(libc::PRIO_PROCESS, 0, nice) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            })
        }
    }

    fn raised_nice_or_skip() -> Option<i32> {
        let nice = raw_attr(0).nice + 5;
        if nice > 19 {
            println!("test skipped: the runner's nice value leaves no room above it");
            return None;
        }
        Some(nice)
    }

    /// This test binary re-run on `thread_helper`, which reports its own pid: under
    /// panic=abort each test runs in a child, so it is not the process spawned here.
    struct ThreadedTarget {
        child: Child,
        helper: Option<i32>,
        tids: Vec<i32>,
    }

    const HELPER_THREADS: &str = "CHRT_TEST_HELPER_THREADS";

    impl ThreadedTarget {
        fn new() -> Self {
            Self::with_threads(2)
        }

        fn with_threads(count: usize) -> Self {
            let exe = std::env::current_exe().expect("the test binary has a path");
            let mut child = plain(&mut Command::new(exe))
                .args([
                    "--exact",
                    "test_chrt::linux::thread_helper",
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(HELPER_THREADS, count.to_string())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("re-run the test binary as SCHED_OTHER");
            let stdout = child.stdout.take().expect("piped stdout");
            // Owned from here on, so that a failed check below kills it.
            let mut target = Self {
                child,
                helper: None,
                tids: Vec::new(),
            };
            target.helper = BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
                // The harness has already started the line.
                .find_map(|line| line.rsplit_once("ready ")?.1.parse().ok());
            assert!(
                target.helper.is_some(),
                "the helper ended before its threads were up"
            );
            target.tids = task_ids(target.pid());
            assert!(
                target.tids.len() > count,
                "expected the helper's threads: {:?}",
                target.tids
            );
            target
        }

        fn pid(&self) -> i32 {
            self.helper.expect("the helper said which process it is")
        }

        fn arg(&self) -> String {
            self.pid().to_string()
        }
    }

    impl Drop for ThreadedTarget {
        fn drop(&mut self) {
            if let Some(pid) = self.helper {
                // SAFETY: a plain syscall on two integers. The helper sleeps
                // until it is killed here, so its pid is still its own.
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    #[ignore = "a target with sleeping threads, run by ThreadedTarget::new()"]
    fn thread_helper() {
        let count = std::env::var(HELPER_THREADS)
            .ok()
            .and_then(|count| count.parse().ok())
            .unwrap_or(2);
        let threads: Vec<_> = (0..count)
            .map(|_| std::thread::spawn(|| std::thread::sleep(Duration::from_secs(60))))
            .collect();
        println!("ready {}", std::process::id());
        for thread in threads {
            let _ = thread.join();
        }
    }

    fn task_ids(pid: i32) -> Vec<i32> {
        std::fs::read_dir(format!("/proc/{pid}/task"))
            .expect("/proc/<pid>/task")
            .map(|entry| {
                entry
                    .expect("a task entry")
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse().ok())
                    .expect("a tid")
            })
            .collect()
    }

    // -- the read-back that bypasses the code under test -----------------------

    /// `struct sched_attr` as the kernel defines it, spelled out here so that
    /// the read-back shares nothing with the tool's own definition.
    #[repr(C)]
    #[derive(Debug, Default, Clone, Copy)]
    struct RawAttr {
        size: u32,
        policy: u32,
        flags: u64,
        nice: i32,
        priority: u32,
        runtime: u64,
        deadline: u64,
        period: u64,
        util_min: u32,
        util_max: u32,
    }

    const RAW_ATTR_SIZE: u32 = std::mem::size_of::<RawAttr>() as u32;
    const RESET_ON_FORK_FLAG: u64 = 1;
    const SCHED_EXT: u32 = 7;

    fn raw_attr(tid: i32) -> RawAttr {
        let mut attr = RawAttr::default();
        // SAFETY: the kernel writes at most RAW_ATTR_SIZE bytes into `attr`,
        // which is exactly that large.
        let result = unsafe {
            libc::syscall(
                libc::SYS_sched_getattr,
                tid,
                &mut attr as *mut RawAttr,
                RAW_ATTR_SIZE,
                0u32,
            )
        };
        assert_eq!(
            result,
            0,
            "sched_getattr({tid}): {}",
            std::io::Error::last_os_error()
        );
        attr
    }

    fn policy_of(tid: i32) -> u32 {
        raw_attr(tid).policy
    }

    fn is_other(tid: i32) -> bool {
        policy_of(tid) == libc::SCHED_OTHER as u32
    }

    fn is_idle(tid: i32) -> bool {
        policy_of(tid) == libc::SCHED_IDLE as u32
    }

    // -- guards ----------------------------------------------------------------

    fn skipped_as_root() -> bool {
        if uucore::process::geteuid() == 0 {
            println!("test skipped: running as root would let the set succeed");
            return true;
        }
        false
    }

    /// Root and an RLIMIT_RTPRIO soft limit above 0 let a realtime set through;
    /// a non-root caller with CAP_SYS_NICE is not detected.
    fn skipped_with_realtime_privilege() -> bool {
        if skipped_as_root() {
            return true;
        }
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: getrlimit(2) writes one rlimit into `limit` and reads
        // nothing else.
        let result = unsafe { libc::getrlimit(libc::RLIMIT_RTPRIO, &mut limit) };
        if result != 0 || limit.rlim_cur > 0 {
            println!("test skipped: RLIMIT_RTPRIO would let a realtime set succeed");
            return true;
        }
        false
    }

    /// The tests that set pid 1 expect a refusal for want of ownership, which
    /// holds only while someone else owns it: in a container the runner may.
    fn skipped_unless_someone_else_owns_pid_1() -> bool {
        use std::os::unix::fs::MetadataExt;
        let euid = uucore::process::geteuid();
        let owner = std::fs::metadata("/proc/1").map(|metadata| metadata.uid());
        if owner.map_or(true, |uid| uid == euid) {
            println!("test skipped: pid 1 is not visibly someone else's");
            return true;
        }
        false
    }

    /// SCHED_EXT exists since Linux 6.12 and only when the kernel was built
    /// with it; a kernel without it refuses to state the policy's bounds.
    fn has_sched_ext() -> bool {
        // SAFETY: takes an integer and touches no memory.
        unsafe { libc::sched_get_priority_min(SCHED_EXT as libc::c_int) != -1 }
    }

    fn skipped_without_sched_ext() -> bool {
        if !has_sched_ext() {
            println!("test skipped: this kernel has no SCHED_EXT");
            return true;
        }
        false
    }

    /// The custom slice exists since Linux 6.12; probed on a throwaway target,
    /// without the code under test.
    fn skipped_without_custom_slice() -> bool {
        let target = Target::new();
        // The target's nice is restated: 0 would lower it under a niced
        // runner, an EPERM that says nothing about the slice.
        let mut attr = RawAttr {
            size: RAW_ATTR_SIZE,
            policy: libc::SCHED_OTHER as u32,
            nice: raw_attr(target.pid()).nice,
            runtime: 3_000_000,
            ..RawAttr::default()
        };
        // SAFETY: the kernel reads `attr.size` bytes, exactly the struct, and
        // on E2BIG writes the size it supports back into `size`.
        let result = unsafe {
            libc::syscall(
                libc::SYS_sched_setattr,
                target.pid(),
                &mut attr as *mut RawAttr,
                0u32,
            )
        };
        if result != 0 || raw_attr(target.pid()).runtime != 3_000_000 {
            println!("test skipped: this kernel does not honor a custom slice");
            return true;
        }
        false
    }

    // -- output shapes ---------------------------------------------------------

    /// The block chrt prints for one task, as a regex: the runtime line's value
    /// is the kernel's default slice, which differs by kernel and CPU count.
    fn block(pid: i32, phase: &str, policy: &str, priority: u32) -> String {
        let policy = regex::escape(policy);
        let mut text = format!(
            "pid {pid}'s {phase} scheduling policy: {policy}\n\
             pid {pid}'s {phase} scheduling priority: {priority}\n"
        );
        if policy.starts_with("SCHED_OTHER") || policy.starts_with("SCHED_BATCH") {
            text.push_str(&format!("pid {pid}'s {phase} runtime parameter: \\d+\n"));
        }
        text
    }

    fn whole(text: &str) -> Regex {
        Regex::new(&format!("^{text}$")).expect("a valid regex")
    }

    fn current(pid: i32, policy: &str) -> Regex {
        whole(&block(pid, "current", policy, 0))
    }

    /// Exec `chrt -p 0` under the chrt being tested, to read the policy back from
    /// the process that inherited it.
    fn nested(options: &[&str]) -> UCommand {
        let mut command = new_ucmd!();
        command
            .args(options)
            .args(&[get_tests_binary(), "chrt", "-p", "0"]);
        command
    }

    fn inherited(policy: &str, priority: u32) -> Regex {
        whole(&block(0, "current", policy, priority).replace("pid 0's", "pid \\d+'s"))
    }

    fn set_failed(id: i32, reason: &str) -> String {
        format!("chrt: failed to set pid {id}'s policy: {reason}\n")
    }

    const TOO_FEW: &str = "chrt: too few arguments\n";
    const NO_TASK_LIST: &str = "chrt: cannot obtain the list of tasks: No such file or directory\n";
    const RUNTIME_NOT_SUPPORTED: &str =
        "chrt: --sched-runtime option is supported for SCHED_OTHER, SCHED_BATCH and SCHED_DEADLINE\n";
    const DEADLINE_ONLY: &str =
        "chrt: --sched-{deadline,period} options are supported for SCHED_DEADLINE only\n";

    fn requires_priority(policy: &str) -> String {
        format!("chrt: policy {policy} requires a priority argument\n")
    }

    fn invalid_timing(name: &str, value: &str) -> String {
        format!("error: invalid value '{value}' for '--sched-{name} <ns>'")
    }

    fn unsupported_priority(value: &str) -> String {
        format!(
            "chrt: unsupported priority value for the policy: {value}: see --max for valid range\n"
        )
    }

    // -- reading ---------------------------------------------------------------

    #[test]
    fn a_pid_token_may_carry_blanks_a_sign_or_leading_zeros() {
        let target = Target::new();
        let pid = target.arg();
        for token in [
            format!(" {pid}"),
            format!("\t\x0b{pid}"),
            format!("+{pid}"),
            format!("00{pid}"),
        ] {
            new_ucmd!()
                .args(&["-p", &token])
                .succeeds()
                .no_stderr()
                .stdout_matches(&current(target.pid(), "SCHED_OTHER"));
        }
    }

    #[test]
    fn rejects_a_malformed_pid() {
        let target = Target::new();
        let pid = target.arg();
        for token in [
            format!("{pid} "),
            format!("{pid}x"),
            "0x10".to_owned(),
            "abc".to_owned(),
            String::new(),
            "1.5".to_owned(),
            " ".to_owned(),
            "+".to_owned(),
            "-".to_owned(),
        ] {
            new_ucmd!()
                .args(&["-p", &token])
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!("chrt: invalid PID argument: '{token}'\n"));
        }
        for token in ["2147483648", "4294967296", "99999999999999999999"] {
            new_ucmd!()
                .args(&["-p", token])
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: invalid PID argument: '{token}': Numerical result out of range\n"
                ));
        }
        // Bytes that are not valid Unicode reach chrt rather than being
        // refused by the parser; the value is echoed lossily.
        new_ucmd!()
            .arg("-p")
            .arg(OsStr::from_bytes(b"1\xff"))
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_contains("chrt: invalid PID argument: ");
    }

    #[test]
    fn reading_an_absent_pid_fails() {
        new_ucmd!()
            .args(&["-p", ABSENT_PID])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(format!(
                "chrt: failed to get pid {ABSENT_PID}'s policy: No such process\n"
            ));
    }

    /// The shell prints its own pid and then becomes chrt, which keeps it.
    #[test]
    fn reading_pid_zero_names_the_reading_process() {
        let output = plain(Command::new("sh").args(["-c", "echo $$; exec \"$0\" chrt -p 0"]))
            .arg(get_tests_binary())
            .output()
            .expect("run chrt through sh");
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
        let (pid, blocks) = stdout.split_once('\n').expect("the shell's pid");
        let pid = pid.parse().expect("a pid");
        assert!(current(pid, "SCHED_OTHER").is_match(blocks), "{stdout:?}");
    }

    #[test]
    fn a_negative_pid_reaches_the_kernel_as_given() {
        new_ucmd!()
            .args(&["-p", "--", "-1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: failed to get pid -1's policy: Invalid argument\n");
    }

    #[test]
    fn one_operand_without_a_policy_is_a_read_whatever_else_is_given() {
        let target = Target::new();
        for options in [
            vec!["-p"],
            vec!["--pid"],
            vec!["--pi"],
            vec!["--p"],
            vec!["-p", "--"],
            vec!["-p", "-v"],
            vec!["-v", "-p"],
            vec!["-p", "-R"],
            vec!["-p", "-T", "1000000"],
            vec!["-p", "-P", "1000000"],
            vec!["-p", "-D", "1000000"],
            vec!["-p", "-p"],
            vec!["-a", "-a", "-p"],
            vec!["-v", "-v", "-p"],
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_stderr()
                .stdout_matches(&current(target.pid(), "SCHED_OTHER"));
            assert!(is_other(target.pid()), "{options:?} changed the target");
        }
    }

    #[test]
    fn all_tasks_reads_every_thread_in_task_list_order() {
        let target = ThreadedTarget::new();
        let blocks: String = target
            .tids
            .iter()
            .map(|&tid| block(tid, "current", "SCHED_OTHER", 0))
            .collect();
        let expected = whole(&blocks);
        new_ucmd!()
            .args(&["--all-tasks", "-p", &target.arg()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&expected);
        // A tid names the whole thread group under -a, and one thread without.
        let last = target.tids.last().expect("a tid").to_string();
        new_ucmd!()
            .args(&["-a", "-p", &last])
            .succeeds()
            .no_stderr()
            .stdout_matches(&expected);
        new_ucmd!()
            .args(&["-p", &last])
            .succeeds()
            .no_stderr()
            .stdout_matches(&current(last.parse().expect("a tid"), "SCHED_OTHER"));
    }

    /// /proc/0, /proc/-1 and an absent pid's directory do not exist.
    #[test]
    fn all_tasks_reads_nothing_when_there_is_no_task_list() {
        for options in [
            vec!["-p", "-a", "0"],
            vec!["-a", "-p", ABSENT_PID],
            vec!["-a", "-p", "--", "-1"],
            vec!["-a", "-v", "-p", ABSENT_PID],
        ] {
            new_ucmd!().args(&options).succeeds().no_output();
        }
    }

    #[test]
    fn all_tasks_set_judges_the_priority_before_taking_the_task_list() {
        for options in [
            vec!["-a", "-p", "-o", "0", ABSENT_PID],
            vec!["-a", "-p", "-o", ABSENT_PID],
            vec!["-a", "-v", "-p", "-o", "0", ABSENT_PID],
            vec!["-a", "-p", "-o", "0", "--", "-5"],
        ] {
            new_ucmd!()
                .args(&options)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(NO_TASK_LIST);
        }
        for (options, value) in [
            (vec!["-a", "-p", "-o", "1", ABSENT_PID], "1"),
            (vec!["-a", "-p", "100", ABSENT_PID], "100"),
            (vec!["-a", "-v", "-p", "-o", "1", ABSENT_PID], "1"),
        ] {
            new_ucmd!()
                .args(&options)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(unsupported_priority(value));
        }
    }

    /// Anyone may read a realtime task: the kernel's migration/0 thread is
    /// SCHED_FIFO at 99 wherever /proc shows kernel threads.
    #[test]
    fn reads_a_realtime_kernel_thread() {
        let Some(pid) = kernel_thread("migration/0") else {
            println!("test skipped: no migration/0 thread visible in /proc");
            return;
        };
        new_ucmd!()
            .args(&["-p", &pid.to_string()])
            .succeeds()
            .no_stderr()
            .stdout_is(format!(
                "pid {pid}'s current scheduling policy: SCHED_FIFO\n\
                 pid {pid}'s current scheduling priority: 99\n"
            ));
    }

    fn kernel_thread(name: &str) -> Option<i32> {
        std::fs::read_dir("/proc")
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
            .find(|pid| {
                std::fs::read_to_string(format!("/proc/{pid}/comm"))
                    .is_ok_and(|comm| comm.trim_end() == name)
            })
    }

    #[test]
    fn a_read_that_cannot_be_written_is_an_error() {
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", &target.arg()])
            .set_stdout(File::create("/dev/full").expect("/dev/full"))
            .fails()
            .code_is(1)
            .stderr_is("chrt: write error: No space left on device\n");
    }

    /// Stdout on a pipe whose reader is gone, made in the child: made here, a
    /// child another test forks meanwhile would hold the read end until its exec.
    fn chrt_into_a_gone_reader(arguments: &[&str]) -> ExitStatus {
        let mut command = Command::new(get_tests_binary());
        command.arg("chrt").args(arguments).stderr(Stdio::null());
        // SAFETY: pipe2(2), dup2(2) and close(2) are async-signal-safe and
        // touch only the child's own descriptors.
        unsafe {
            command.pre_exec(|| {
                let mut fds = [0; 2];
                if libc::pipe2(fds.as_mut_ptr(), 0) == -1
                    || libc::close(fds[0]) == -1
                    || libc::dup2(fds[1], libc::STDOUT_FILENO) == -1
                    || libc::close(fds[1]) == -1
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command.status().expect("run chrt")
    }

    #[test]
    fn a_read_into_a_closed_pipe_dies_of_sigpipe() {
        let target = Target::new();
        let status = chrt_into_a_gone_reader(&["-p", &target.arg()]);
        assert_eq!(status.signal(), Some(libc::SIGPIPE), "{status:?}");
    }

    // -- setting a policy on a pid ---------------------------------------------

    #[test]
    fn sets_a_policy_that_needs_no_priority_with_or_without_a_zero() {
        for (options, expected) in [
            (vec!["-p", "-o"], libc::SCHED_OTHER),
            (vec!["-p", "-o", "0"], libc::SCHED_OTHER),
            (vec!["-p", "-b"], libc::SCHED_BATCH),
            (vec!["-p", "-b", "0"], libc::SCHED_BATCH),
            (vec!["-p", "-i"], libc::SCHED_IDLE),
            (vec!["-p", "-i", "0"], libc::SCHED_IDLE),
            (vec!["--pid", "--batch"], libc::SCHED_BATCH),
            (vec!["--pi", "--b"], libc::SCHED_BATCH),
            (vec!["--pid", "--oth", "0"], libc::SCHED_OTHER),
            (vec!["-po"], libc::SCHED_OTHER),
            (vec!["-pio"], libc::SCHED_OTHER),
            (vec!["-pa", "-i"], libc::SCHED_IDLE),
            (vec!["-p", "-i", "--"], libc::SCHED_IDLE),
            (vec!["-p", "-o", "0", "--"], libc::SCHED_OTHER),
        ] {
            let target = Target::new();
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_output();
            assert_eq!(policy_of(target.pid()), expected as u32, "{options:?}");
        }
    }

    #[test]
    fn a_set_keeps_the_nice_value() {
        let Some(nice) = raised_nice_or_skip() else {
            return;
        };
        for (options, expected) in [
            (vec!["-p", "-o", "0"], libc::SCHED_OTHER),
            (vec!["-p", "-b"], libc::SCHED_BATCH),
            (vec!["-p", "-i"], libc::SCHED_IDLE),
        ] {
            let target = Target::niced(nice);
            assert_eq!(raw_attr(target.pid()).nice, nice, "nice did not take");
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_output();
            let attr = raw_attr(target.pid());
            assert_eq!(attr.policy, expected as u32, "{options:?}");
            assert_eq!(attr.nice, nice, "{options:?}");
        }
    }

    #[test]
    fn a_niced_command_keeps_the_nice_value() {
        let Some(nice) = raised_nice_or_skip() else {
            return;
        };
        let mut command = Command::new(get_tests_binary());
        command
            .args(["chrt", "-b", "0", "sh", "-c", "\"$0\" chrt -p 0 && nice"])
            .arg(get_tests_binary());
        let output = at_nice(plain(&mut command), nice)
            .output()
            .expect("run chrt niced");
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
        let nice_line = format!("{nice}\n");
        assert!(stdout.ends_with(&nice_line), "{stdout:?}");
        let block = &stdout[..stdout.len() - nice_line.len()];
        assert!(inherited("SCHED_BATCH", 0).is_match(block), "{stdout:?}");
    }

    #[test]
    fn sets_sched_ext() {
        if skipped_without_sched_ext() {
            return;
        }
        for options in [
            vec!["-p", "-e"],
            vec!["-p", "--ext", "0"],
            vec!["-p", "--e"],
        ] {
            let target = Target::new();
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_output();
            assert_eq!(policy_of(target.pid()), SCHED_EXT, "{options:?}");
        }
        let target = Target::new();
        new_ucmd!().args(&["-p", "-e", &target.arg()]).succeeds();
        new_ucmd!()
            .args(&["-p", &target.arg()])
            .succeeds()
            .stdout_matches(&current(target.pid(), "SCHED_EXT"));
    }

    /// On a kernel without SCHED_EXT the policy's bounds are -1, and the
    /// priority is held to them all the same: 0 is refused before any set.
    #[test]
    fn a_policy_the_kernel_lacks_admits_no_priority() {
        if has_sched_ext() {
            println!("test skipped: this kernel has SCHED_EXT");
            return;
        }
        let target = Target::new();
        for arguments in [
            vec!["-e", "true"],
            vec!["-e", "0", "true"],
            vec!["-p", "-e", &target.arg()],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(unsupported_priority("0"));
        }
        assert!(is_other(target.pid()));
    }

    #[test]
    fn a_pid_mode_priority_is_read_like_a_pid() {
        for token in ["00", "+0", " 0"] {
            let target = Target::new();
            new_ucmd!()
                .args(&["-p", "-i", token, &target.arg()])
                .succeeds()
                .no_output();
            assert!(is_idle(target.pid()), "{token:?}");
        }
        let target = Target::new();
        for token in ["0x0", "", "abc", "0.0"] {
            new_ucmd!()
                .args(&["-p", "-i", token, &target.arg()])
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!("chrt: invalid priority argument: '{token}'\n"));
        }
        new_ucmd!()
            .args(&["-p", "-i", "2147483648", &target.arg()])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(
                "chrt: invalid priority argument: '2147483648': Numerical result out of range\n",
            );
        assert!(is_other(target.pid()));
    }

    #[test]
    fn the_last_policy_option_wins() {
        for (options, expected) in [
            (vec!["-p", "-o", "-b"], libc::SCHED_BATCH),
            (vec!["-p", "-b", "-o"], libc::SCHED_OTHER),
            (vec!["-p", "-f", "-i", "0"], libc::SCHED_IDLE),
            (vec!["-p", "-ob"], libc::SCHED_BATCH),
            (vec!["-p", "-o", "-o"], libc::SCHED_OTHER),
        ] {
            let target = Target::new();
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_output();
            assert_eq!(policy_of(target.pid()), expected as u32, "{options:?}");
        }
    }

    /// A -f or -r demands a priority, even when a later policy option wins, and
    /// the message names the winner.
    #[test]
    fn a_realtime_option_demands_a_priority() {
        let target = Target::new();
        for (options, policy) in [
            (vec!["-p", "-f"], "SCHED_FIFO"),
            (vec!["-p", "-r"], "SCHED_RR"),
            (vec!["-p", "--fifo"], "SCHED_FIFO"),
            (vec!["-p", "-f", "-i"], "SCHED_IDLE"),
            (vec!["-r", "-b", "-p"], "SCHED_BATCH"),
            (vec!["-p", "-o", "-f", "-o"], "SCHED_OTHER"),
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(requires_priority(policy));
        }
        assert!(is_other(target.pid()));
        // The check precedes the pid-zero exec fallback.
        new_ucmd!()
            .args(&["-p", "-f", "0"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(requires_priority("SCHED_FIFO"));
        new_ucmd!()
            .args(&["-f", "-o", "echo", "hi"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(requires_priority("SCHED_OTHER"));
        new_ucmd!()
            .args(&["-r", "-o", "0"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: no command or priority specified\n");
        // With the priority given, the winner takes it.
        new_ucmd!()
            .args(&["-f", "-o", "0", "echo", "hi"])
            .succeeds()
            .stdout_is("hi\n");
    }

    /// The bounds come from the kernel: 1-99 for the realtime policies, 0 for
    /// every other one, and SCHED_RR is the default when no policy is named.
    #[test]
    fn the_priority_is_held_to_the_policy_before_anything_is_set() {
        let target = Target::new();
        for (options, value) in [
            (vec!["-p", "0"], "0"),
            (vec!["-p", "100"], "100"),
            (vec!["-p", "-f", "0"], "0"),
            (vec!["-p", "-r", "100"], "100"),
            (vec!["-p", "-o", "1"], "1"),
            (vec!["-p", "-b", "99"], "99"),
            (vec!["-p", "-i", "007"], "7"),
            (vec!["-p", "-d", "1"], "1"),
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(unsupported_priority(value));
        }
        assert!(is_other(target.pid()));
    }

    /// A bad pid is reported over an out-of-range priority, and an out-of-range
    /// priority before any pid is touched.
    #[test]
    fn both_operands_are_parsed_before_the_range_is_judged() {
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", &target.arg(), "extra"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid PID argument: 'extra'\n");
        new_ucmd!()
            .args(&["-p", "-o", "0", &target.arg(), "extra"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid PID argument: 'extra'\n");
        assert!(is_other(target.pid()));
        new_ucmd!()
            .args(&["-p", ABSENT_PID, "1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(unsupported_priority(ABSENT_PID));
    }

    #[test]
    fn sets_the_reset_on_fork_flag() {
        for options in [
            vec!["-p", "-o", "-R", "0"],
            vec!["-p", "--reset-on-fork", "-i"],
        ] {
            let target = Target::new();
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_output();
            assert_ne!(raw_attr(target.pid()).flags & RESET_ON_FORK_FLAG, 0);
        }
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", "-R", "-b", &target.arg()])
            .succeeds();
        new_ucmd!()
            .args(&["-p", &target.arg()])
            .succeeds()
            .stdout_matches(&current(target.pid(), "SCHED_BATCH|SCHED_RESET_ON_FORK"));
    }

    #[test]
    fn options_after_the_first_operand_are_operands() {
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", "-o", "0", "-R", &target.arg()])
            .succeeds()
            .no_output();
        assert_eq!(raw_attr(target.pid()).flags & RESET_ON_FORK_FLAG, 0);
        new_ucmd!()
            .args(&["-p", &target.arg(), "-o"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid PID argument: '-o'\n");
        new_ucmd!()
            .args(&["-p", &target.arg(), "-h"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid PID argument: '-h'\n");
        new_ucmd!()
            .args(&["-p", &target.arg(), "-m"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid PID argument: '-m'\n");
    }

    #[test]
    fn a_custom_slice_is_applied_and_read_back() {
        if skipped_without_custom_slice() {
            return;
        }
        for (options, expected) in [
            (vec!["-p", "-o", "-T", "3000000"], 3_000_000),
            (vec!["-p", "-b", "-T", "3000000"], 3_000_000),
            (vec!["-p", "-o", "--sched-runtime=3000000"], 3_000_000),
            (vec!["-p", "-oT3000000"], 3_000_000),
            (
                vec!["-p", "-o", "-T", "3000000", "-T", "4000000"],
                4_000_000,
            ),
            // The kernel clamps the slice to its own bounds.
            (vec!["-p", "-o", "-T", "100"], 100_000),
        ] {
            let target = Target::new();
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .succeeds()
                .no_output();
            assert_eq!(raw_attr(target.pid()).runtime, expected, "{options:?}");
        }
        // Written after the priority, -T is an operand and no slice is set.
        let target = Target::new();
        let before = raw_attr(target.pid()).runtime;
        new_ucmd!()
            .args(&["-p", "-o", "0", "-T", "3000000", &target.arg()])
            .succeeds()
            .no_output();
        assert_eq!(raw_attr(target.pid()).runtime, before);
    }

    #[test]
    fn a_custom_slice_shows_in_the_verbose_new_block_and_in_a_later_read() {
        if skipped_without_custom_slice() {
            return;
        }
        let target = Target::new();
        let pid = target.pid();
        new_ucmd!()
            .args(&["-p", "-v", "-o", "-T", "3000000", &target.arg()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&whole(&format!(
                "{}pid {pid}'s new scheduling policy: SCHED_OTHER\n\
                 pid {pid}'s new scheduling priority: 0\n\
                 pid {pid}'s new runtime parameter: 3000000\n",
                block(pid, "current", "SCHED_OTHER", 0)
            )));
        new_ucmd!()
            .args(&["-p", &target.arg()])
            .succeeds()
            .stdout_is(format!(
                "pid {pid}'s current scheduling policy: SCHED_OTHER\n\
                 pid {pid}'s current scheduling priority: 0\n\
                 pid {pid}'s current runtime parameter: 3000000\n"
            ));
    }

    #[test]
    fn timing_options_are_held_to_the_policies_that_take_them() {
        let target = Target::new();
        let pid = target.arg();
        for arguments in [
            vec!["-p", "-i", "-T", "3000000", &pid],
            vec!["-p", "-f", "-T", "3000000", "10", &pid],
            vec!["-p", "-r", "-T", "3000000", "10", &pid],
            vec!["-p", "-T", "3000000", "10", &pid],
            vec!["-i", "-T", "3000000", "true"],
            vec!["-f", "-T", "3000000", "10", "true"],
            vec!["-r", "-T", "3000000", "10", "true"],
            vec!["-T", "3000000", "10", "true"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(RUNTIME_NOT_SUPPORTED);
        }
        for arguments in [
            vec!["-p", "-o", "-P", "1000000", &pid],
            vec!["-p", "-o", "-D", "1000000", &pid],
            vec!["-p", "-b", "-P", "1000000", "-D", "1000000", &pid],
            vec!["-p", "-i", "--sched-period", "1000000", &pid],
            vec!["-o", "-P", "1000000", "true"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(DEADLINE_ONLY);
        }
        assert!(is_other(target.pid()));
    }

    #[test]
    fn a_zero_timing_value_counts_as_not_given() {
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", "-i", "-T", "0", &target.arg()])
            .succeeds()
            .no_output();
        assert!(is_idle(target.pid()));
        for arguments in [
            vec!["-i", "-T", "0", "true"],
            vec!["-i", "-T", "-0", "true"],
            vec!["-i", "-T", "5", "-T", "0", "true"],
            vec!["-i", "-T", "5", "-T", "+0", "true"],
            vec!["-i", "-T", "-000000000000000000000000", "true"],
            vec!["-o", "-P", "0", "true"],
            vec!["-o", "-D", "00", "true"],
            vec!["-b", "-T", "0", "-P", "0", "-D", "0", "true"],
        ] {
            new_ucmd!().args(&arguments).succeeds().no_output();
        }
        new_ucmd!()
            .args(&["-i", "-T", "0", "-T", "5", "true"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(RUNTIME_NOT_SUPPORTED);
    }

    /// A value that parses is proven by a SCHED_DEADLINE set reaching the
    /// kernel, which then refuses it for lack of privilege or for its relations.
    #[test]
    fn timing_values_are_unsigned_64_bit() {
        if skipped_as_root() {
            return;
        }
        let target = Target::new();
        for options in [
            vec!["-p", "-d", "-T", "+5000000", "-P", "10000000"],
            vec!["-p", "-d", "-T", " 5000000", "-P", "10000000"],
            vec!["-p", "-d", "-T", "9223372036854775807", "-P", "10000000"],
            vec!["-p", "-d", "-T", "9223372036854775808", "-P", "10000000"],
            vec!["-p", "-d", "-P", "18446744073709551615"],
            vec!["-p", "-d", "-D", "1000000"],
            vec!["-p", "-d", "-P", "-0"],
            vec!["-p", "-d", "-P", " -00"],
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains(format!(
                    "chrt: failed to set pid {}'s policy: ",
                    target.pid()
                ));
        }
        for (option, name) in [("-T", "runtime"), ("-P", "period"), ("-D", "deadline")] {
            for value in [
                "abc",
                "",
                "0x10",
                "1.5",
                "3000000x",
                "-1",
                "18446744073709551616",
                "-18446744073709551616",
                "-",
                "-+0",
            ] {
                new_ucmd!()
                    .args(&["-p", "-d", option, value, &target.arg()])
                    .fails()
                    .code_is(1)
                    .no_stdout()
                    .stderr_contains(invalid_timing(name, value));
            }
        }
        assert!(is_other(target.pid()));
        // Negative zero is zero, the kernel's default slice.
        new_ucmd!()
            .args(&["-p", "-b", "-T", "-0", &target.arg()])
            .succeeds()
            .no_output();
        assert_eq!(policy_of(target.pid()), libc::SCHED_BATCH as u32);
    }

    /// The kernel judges the relations before it looks at privilege.
    #[test]
    fn deadline_relations_are_the_kernels_to_judge() {
        let target = Target::new();
        for options in [
            vec!["-p", "-d"],
            vec!["-p", "-d", "-T", "500000"],
            vec!["-p", "-d", "-T", "2000000", "-P", "1000000"],
            vec!["-p", "-d", "-P", "100"],
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(set_failed(target.pid(), "Invalid argument"));
        }
        assert!(is_other(target.pid()));
    }

    #[test]
    fn setting_an_absent_pid_fails() {
        new_ucmd!()
            .args(&["-p", "-o", "0", ABSENT_PID])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(set_failed(
                ABSENT_PID.parse().expect("a pid"),
                "No such process",
            ));
    }

    #[test]
    fn verbose_set_prints_the_current_block_then_the_new_one() {
        let target = Target::new();
        let pid = target.pid();
        new_ucmd!()
            .args(&["-p", "-v", "-i", &target.arg()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&whole(&format!(
                "{}{}",
                block(pid, "current", "SCHED_OTHER", 0),
                block(pid, "new", "SCHED_IDLE", 0)
            )));
        assert!(is_idle(pid));
        // Written after the priority, -v is an operand: nothing is printed.
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", "-i", "0", "-v", &target.arg()])
            .succeeds()
            .no_output();
    }

    #[test]
    fn a_verbose_set_shows_the_target_before_judging_the_rest() {
        let target = Target::new();
        for (options, error) in [
            (vec!["-v", "-p", "-f"], requires_priority("SCHED_FIFO")),
            (
                vec!["-v", "-p", "abc"],
                "chrt: invalid priority argument: 'abc'\n".to_owned(),
            ),
            (
                vec!["-v", "-p", "-i", "-T", "3000000"],
                RUNTIME_NOT_SUPPORTED.to_owned(),
            ),
            (
                vec!["-v", "-p", "-o", "-P", "1000000"],
                DEADLINE_ONLY.to_owned(),
            ),
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .fails()
                .code_is(1)
                .stderr_is(error)
                .stdout_matches(&current(target.pid(), "SCHED_OTHER"));
        }
        assert!(is_other(target.pid()));
        // A target that cannot be shown is reported in place of the rest.
        new_ucmd!()
            .args(&["-v", "-p", "-f", ABSENT_PID])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(format!(
                "chrt: failed to get pid {ABSENT_PID}'s policy: No such process\n"
            ));
        // Pid 0 is chrt itself, named by its pid, before and after the set.
        // It starts as SCHED_OTHER, so the runner's policy does not show.
        let own =
            |phase: &str, policy: &str| block(0, phase, policy, 0).replace("pid 0's", "pid \\d+'s");
        for (arguments, code, error, expected) in [
            (
                vec!["-v", "-p", "-o", "1", "0"],
                1,
                unsupported_priority("1"),
                own("current", "SCHED_OTHER"),
            ),
            (
                vec!["-v", "-p", "-i", "0"],
                127,
                "chrt: failed to execute 0: No such file or directory\n".to_owned(),
                format!(
                    "{}{}",
                    own("current", "SCHED_OTHER"),
                    own("new", "SCHED_IDLE")
                ),
            ),
        ] {
            let output = plain(
                Command::new(get_tests_binary())
                    .arg("chrt")
                    .args(&arguments),
            )
            .output()
            .expect("run chrt");
            assert_eq!(output.status.code(), Some(code), "{arguments:?}");
            assert_eq!(String::from_utf8_lossy(&output.stderr), error);
            let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
            assert!(whole(&expected).is_match(&stdout), "{stdout:?}");
        }
    }

    #[test]
    fn a_verbose_set_that_cannot_be_written_still_sets() {
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", "-v", "-i", &target.arg()])
            .set_stdout(File::create("/dev/full").expect("/dev/full"))
            .fails()
            .code_is(1)
            .stderr_is("chrt: write error: No space left on device\n");
        assert!(is_idle(target.pid()));
    }

    #[test]
    fn all_tasks_sets_every_thread() {
        let target = ThreadedTarget::new();
        new_ucmd!()
            .args(&["-a", "-p", "-i", &target.arg()])
            .succeeds()
            .no_output();
        for &tid in &target.tids {
            assert!(is_idle(tid), "tid {tid} was not changed");
        }
    }

    #[test]
    fn all_tasks_verbose_prints_all_current_blocks_then_all_new_blocks() {
        let target = ThreadedTarget::new();
        let current: String = target
            .tids
            .iter()
            .map(|&tid| block(tid, "current", "SCHED_OTHER", 0))
            .collect();
        let new: String = target
            .tids
            .iter()
            .map(|&tid| block(tid, "new", "SCHED_BATCH", 0))
            .collect();
        new_ucmd!()
            .args(&["-a", "-v", "-p", "-b", &target.arg()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&whole(&format!("{current}{new}")));
    }

    /// Off a terminal stdout leaves in pieces of up to 4096 bytes, so a gone
    /// reader kills chrt at exit with a few threads, but before any set with forty.
    #[test]
    fn a_gone_reader_stops_a_verbose_walk_at_the_first_full_piece() {
        let few = ThreadedTarget::new();
        let status = chrt_into_a_gone_reader(&["-a", "-v", "-p", "-b", &few.arg()]);
        assert_eq!(status.signal(), Some(libc::SIGPIPE), "{status:?}");
        for &tid in &few.tids {
            assert_eq!(policy_of(tid), libc::SCHED_BATCH as u32, "tid {tid}");
        }
        let many = ThreadedTarget::with_threads(40);
        let status = chrt_into_a_gone_reader(&["-a", "-v", "-p", "-b", &many.arg()]);
        assert_eq!(status.signal(), Some(libc::SIGPIPE), "{status:?}");
        for &tid in &many.tids {
            assert!(is_other(tid), "tid {tid} was changed");
        }
    }

    #[test]
    fn without_all_tasks_a_tid_changes_that_thread_only() {
        let target = ThreadedTarget::new();
        let last = *target.tids.last().expect("a tid");
        new_ucmd!()
            .args(&["-p", "-i", &last.to_string()])
            .succeeds()
            .no_output();
        assert!(is_idle(last));
        assert!(is_other(target.pid()));
    }

    #[test]
    fn pid_zero_and_minus_one_turn_the_set_into_a_command() {
        for (options, command) in [
            (vec!["-p", "-o", "0", "0"], "0"),
            (vec!["-p", "-o", "0", "-1"], "-1"),
            (vec!["-p", "-o", "--", "-1"], "-1"),
            (vec!["-p", "-o", "0"], "0"),
            (vec!["-p", "-o", "0", "-a", "0"], "-a"),
            (vec!["-p", "-o", "0", "--", "-00001"], "-00001"),
        ] {
            new_ucmd!()
                .args(&options)
                .fails()
                .code_is(127)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: failed to execute {command}: No such file or directory\n"
                ));
        }
    }

    /// Under -v nothing is shown for such a pid first.
    #[test]
    fn a_pid_below_minus_one_is_set_and_refused() {
        for options in [
            vec!["-p", "-o", "0", "--", "-5"],
            vec!["-p", "-o", "--", "-2"],
            vec!["-p", "-i", "--", "-2147483648"],
            vec!["-p", "-f", "10", "--", "-2"],
            vec!["-p", "-b", "0", "extra", "--", "-3"],
            vec!["-v", "-p", "-o", "0", "--", "-5"],
        ] {
            let pid = options.last().expect("a pid");
            new_ucmd!()
                .args(&options)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: failed to set pid {pid}'s policy: Invalid argument\n"
                ));
        }
    }

    #[test]
    fn a_negative_pid_reads_the_operands_as_exec_mode_does() {
        for (arguments, output) in [
            (vec!["-p", "-o", "echo", "hi", "-1"], "hi -1\n"),
            (vec!["-p", "-o", "--", "--", "echo", "-1"], "-1\n"),
            (vec!["-p", "-o", "0", "echo", "-1"], "-1\n"),
            (vec!["-p", "-o", "echo", "--", "-1"], "-- -1\n"),
        ] {
            new_ucmd!().args(&arguments).succeeds().stdout_is(output);
        }
        for (arguments, command) in [
            (vec!["-p", "-o", "+0", "echo", "-1"], "+0"),
            (vec!["-p", "-o", "--", "-00001", "-00001"], "-00001"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(127)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: failed to execute {command}: No such file or directory\n"
                ));
        }
        new_ucmd!()
            .args(&["-p", "-f", "true", "-1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(requires_priority("SCHED_FIFO"));
        new_ucmd!()
            .args(&["-p", "-o", "+5", "-2"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: failed to set pid -2's policy: Invalid argument\n");
    }

    // -- running a command -----------------------------------------------------

    #[test]
    fn a_command_inherits_the_policy() {
        for (options, policy) in [
            (vec!["-o", "0"], "SCHED_OTHER"),
            (vec!["--other"], "SCHED_OTHER"),
            (vec!["-b", "0"], "SCHED_BATCH"),
            (vec!["-b"], "SCHED_BATCH"),
            (vec!["-i", "0"], "SCHED_IDLE"),
            (vec!["--idle"], "SCHED_IDLE"),
            (vec!["-o", "-R", "0"], "SCHED_OTHER|SCHED_RESET_ON_FORK"),
            (
                vec!["-i", "--reset-on-fork"],
                "SCHED_IDLE|SCHED_RESET_ON_FORK",
            ),
            (vec!["-o", "0", "--"], "SCHED_OTHER"),
            (vec!["-v", "-i"], "SCHED_IDLE"),
        ] {
            nested(&options)
                .succeeds()
                .no_stderr()
                .stdout_matches(&inherited(policy, 0));
        }
    }

    #[test]
    fn a_command_inherits_sched_ext() {
        if skipped_without_sched_ext() {
            return;
        }
        nested(&["-e"])
            .succeeds()
            .no_stderr()
            .stdout_matches(&inherited("SCHED_EXT", 0));
    }

    #[test]
    fn a_command_inherits_a_custom_slice() {
        if skipped_without_custom_slice() {
            return;
        }
        nested(&["-o", "-T", "3000000"])
            .succeeds()
            .no_stderr()
            .stdout_matches(&whole(
                "pid \\d+'s current scheduling policy: SCHED_OTHER\n\
                 pid \\d+'s current scheduling priority: 0\n\
                 pid \\d+'s current runtime parameter: 3000000\n",
            ));
    }

    #[test]
    fn the_command_exit_status_is_passed_through() {
        new_ucmd!()
            .args(&["-o", "0", "sh", "-c", "exit 42"])
            .fails()
            .code_is(42)
            .no_output();
        new_ucmd!()
            .args(&["-o", "0", "sh", "-c", "exit 0"])
            .succeeds()
            .no_output();
        new_ucmd!()
            .args(&["-o", "0", "sh", "-c", "kill -TERM $$"])
            .fails()
            .signal_is(libc::SIGTERM);
    }

    #[test]
    fn a_command_that_cannot_be_run_fails() {
        for (name, code, reason) in [
            (
                "this-command-does-not-exist-hopefully",
                127,
                "No such file or directory",
            ),
            ("/nonexistent", 127, "No such file or directory"),
            ("", 127, "No such file or directory"),
            (".", 126, "Permission denied"),
        ] {
            new_ucmd!()
                .args(&["-o", "0", name])
                .fails()
                .code_is(code)
                .no_stdout()
                .stderr_is(format!("chrt: failed to execute {name}: {reason}\n"));
        }
    }

    /// A directory makes exec fail with EACCES (126), proving the name arrived
    /// intact; bytes flattened to replacement characters would name nothing (127).
    #[test]
    fn a_command_name_that_is_not_valid_unicode_reaches_exec_unchanged() {
        let (at, mut ucmd) = at_and_ucmd!();
        let name = OsStr::from_bytes(b"not-\xffunicode");
        std::fs::create_dir(at.plus(name)).expect("create the directory");

        ucmd.args(&["-o", "0"])
            .arg(at.plus(name))
            .fails()
            .code_is(126)
            .stderr_contains("Permission denied");
    }

    #[test]
    fn everything_from_the_command_on_belongs_to_the_command() {
        for (arguments, output) in [
            (vec!["-o", "0", "echo", "-p", "1"], "-p 1\n"),
            (vec!["-o", "0", "echo", "-o", "-i"], "-o -i\n"),
            (vec!["-o", "0", "echo", "-n", "hello"], "hello"),
            (vec!["-o", "0", "--", "echo", "-o"], "-o\n"),
            (vec!["-o", "0", "echo", "--", "x"], "-- x\n"),
            (vec!["-o", "echo", "--", "x"], "-- x\n"),
            (vec!["-i", "echo", "-m"], "-m\n"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .succeeds()
                .no_stderr()
                .stdout_is(output);
        }
        new_ucmd!()
            .args(&["-o", "0", "-m", "true"])
            .fails()
            .code_is(127)
            .no_stdout()
            .stderr_is("chrt: failed to execute -m: No such file or directory\n");
    }

    #[test]
    fn the_command_inherits_stdin() {
        new_ucmd!()
            .args(&["-o", "0", "cat"])
            .pipe_in("hello\n")
            .succeeds()
            .no_stderr()
            .stdout_is("hello\n");
    }

    /// Under a policy that needs no priority anything else is the command; under
    /// one that does, it is a missing priority.
    #[test]
    fn the_first_operand_is_a_priority_only_when_all_digits() {
        for command in ["0x0", "+0", " 0", "0 ", "1.5", "abc", "3x", "", "-"] {
            new_ucmd!()
                .args(&["-o", command, "true"])
                .fails()
                .code_is(127)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: failed to execute {command}: No such file or directory\n"
                ));
        }
        for (arguments, policy) in [
            (vec!["+5", "true"], "SCHED_RR"),
            (vec!["-f", "abc", "true"], "SCHED_FIFO"),
            (vec!["-r", "", "true"], "SCHED_RR"),
            // The operands are judged before -T is matched to the policy.
            (vec!["-f", "-T", "1", "true"], "SCHED_FIFO"),
            (vec!["-"], "SCHED_RR"),
            (vec!["--", "-o", "0", "true"], "SCHED_RR"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(requires_priority(policy));
        }
        new_ucmd!()
            .arg("-f")
            .arg(OsStr::from_bytes(b"1\xff"))
            .arg("true")
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(requires_priority("SCHED_FIFO"));
        // Two digit operands: the second is the command.
        new_ucmd!()
            .args(&["-o", "0", "0", "true"])
            .fails()
            .code_is(127)
            .stderr_is("chrt: failed to execute 0: No such file or directory\n");
    }

    /// However large: a lone number is never judged as a priority.
    #[test]
    fn a_lone_number_is_a_command_only_when_no_priority_is_needed() {
        for arguments in [
            vec!["-f", "10"],
            vec!["10"],
            vec!["-r", "1"],
            vec!["-f", "99999999999"],
            vec!["2147483648"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is("chrt: no command or priority specified\n");
        }
        for number in ["0", "99999999999", "2147483648"] {
            new_ucmd!()
                .args(&["-o", number])
                .fails()
                .code_is(127)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: failed to execute {number}: No such file or directory\n"
                ));
        }
    }

    #[test]
    fn an_exec_mode_priority_is_range_checked_before_the_command_runs() {
        for (arguments, value) in [
            (vec!["-o", "1", "true"], "1"),
            (vec!["-f", "0", "true"], "0"),
            (vec!["-f", "100", "true"], "100"),
            (vec!["0", "true"], "0"),
            (vec!["2147483647", "true"], "2147483647"),
            (vec!["-o", "007", "true"], "7"),
            (vec!["-o", "1", "/nonexistent"], "1"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(unsupported_priority(value));
        }
        // With anything after it, if only a `--`, a number is the priority, so
        // an overflow is reported rather than run.
        for (arguments, value) in [
            (vec!["2147483648", "true"], "2147483648"),
            (vec!["99999999999999999999", "true"], "99999999999999999999"),
            (vec!["-o", "99999999999", "--"], "99999999999"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: invalid priority argument: '{value}': Numerical result out of range\n"
                ));
        }
    }

    /// The priority is still judged first.
    #[test]
    fn all_tasks_makes_no_sense_for_a_command() {
        new_ucmd!()
            .args(&["-a", "-o", "0", "true"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(NO_TASK_LIST);
        new_ucmd!()
            .args(&["-a", "-o", "1", "true"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(unsupported_priority("1"));
    }

    #[test]
    fn a_verbose_command_shows_the_new_block_only_if_chrt_exits() {
        new_ucmd!()
            .args(&["-v", "-o", "0", "true"])
            .succeeds()
            .no_output();
        let new_block =
            || whole(&block(0, "new", "SCHED_OTHER", 0).replace("pid 0's", "pid \\d+'s"));
        for (arguments, code, error) in [
            (
                vec!["-v", "-o", "0", "/nonexistent"],
                127,
                "chrt: failed to execute /nonexistent: No such file or directory\n",
            ),
            (
                vec!["-v", "-o", "0", "--"],
                1,
                "chrt: no command specified\n",
            ),
            (
                vec!["-v", "-p", "-o", "/nonexistent", "-1"],
                127,
                "chrt: failed to execute /nonexistent: No such file or directory\n",
            ),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(code)
                .stderr_is(error)
                .stdout_matches(&new_block());
        }
        // The block cannot be written: that is reported last, and it decides
        // the status.
        new_ucmd!()
            .args(&["-v", "-o", "0", "/nonexistent"])
            .set_stdout(File::create("/dev/full").expect("/dev/full"))
            .fails()
            .code_is(1)
            .stderr_is(
                "chrt: failed to execute /nonexistent: No such file or directory\n\
                 chrt: write error: No space left on device\n",
            );
    }

    #[test]
    fn a_verbose_command_on_a_terminal_shows_the_new_block_first() {
        let block = block(0, "new", "SCHED_OTHER", 0)
            .replace("pid 0's", "pid \\d+'s")
            .replace('\n', "\r\n");
        new_ucmd!()
            .args(&["-v", "-o", "0", "echo", "x"])
            .terminal_simulation(true)
            .succeeds()
            .stdout_matches(&whole(&format!("{block}x\r\n")));
    }

    // -- the immediate options and the argument grammar ------------------------

    #[test]
    fn nothing_to_act_on_is_a_usage_error() {
        for arguments in [
            vec![],
            vec!["-p"],
            vec!["--pid"],
            vec!["-p", "-o"],
            vec!["-o"],
            vec!["-R"],
            vec!["-T", "1000000"],
            vec!["-a"],
            vec!["-v"],
            vec!["-p", "-a"],
            vec!["-d", "-P", "1000000"],
            vec!["--"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(TOO_FEW);
        }
    }

    /// A kernel without SCHED_EXT refuses to state its bounds, and the row says so.
    fn max_table() -> Regex {
        whole(
            "SCHED_OTHER min/max priority\t: 0/0\n\
             SCHED_FIFO min/max priority\t: 1/99\n\
             SCHED_RR min/max priority\t: 1/99\n\
             SCHED_BATCH min/max priority\t: 0/0\n\
             SCHED_IDLE min/max priority\t: 0/0\n\
             SCHED_DEADLINE min/max priority\t: 0/0\n\
             SCHED_EXT (?:min/max priority\t: 0/0|not supported\\?)\n",
        )
    }

    #[test]
    fn max_prints_the_table_and_ignores_the_rest_of_the_line() {
        for arguments in [
            vec!["-m"],
            vec!["--max"],
            vec!["--ma"],
            vec!["-m", "extra"],
            vec!["-m", "-p", "1"],
            vec!["-m", "-o", "0", "true"],
            vec!["-m", "-f", "0", "true"],
            vec!["-mp"],
            vec!["-m", "-m"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .succeeds()
                .no_stderr()
                .stdout_matches(&max_table());
        }
    }

    #[test]
    fn help_and_version_act_the_moment_they_are_parsed() {
        for arguments in [
            vec!["-h"],
            vec!["--help"],
            vec!["-h", "-x"],
            vec!["-h", "-p", "1"],
            vec!["--help", "--version"],
            vec!["-hV"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .succeeds()
                .no_stderr()
                .stdout_contains("Usage:");
        }
        for arguments in [
            vec!["-V"],
            vec!["--version"],
            vec!["--vers"],
            vec!["-V", "-x"],
            vec!["-V", "extra"],
            vec!["-Vh"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .succeeds()
                .no_stderr()
                .stdout_contains("chrt ");
        }
    }

    #[test]
    fn help_and_version_that_cannot_be_written_are_errors() {
        for option in ["-h", "-V", "-m"] {
            new_ucmd!()
                .arg(option)
                .set_stdout(File::create("/dev/full").expect("/dev/full"))
                .fails()
                .code_is(1)
                .stderr_is("chrt: write error: No space left on device\n");
        }
    }

    /// The wording is clap's; what binds is empty stdout and status 1, never 2. An
    /// error before -h or -V wins over them, and one anywhere wins over -m.
    #[test]
    fn an_option_error_fails_with_status_one() {
        let unexpected = |argument: &str| format!("error: unexpected argument '{argument}' found");
        let no_value = |value: &str, option: &str| {
            format!("error: unexpected value '{value}' for '{option}' found")
        };
        let missing = || "error: a value is required for '--sched-runtime <ns>'".to_owned();
        for (arguments, error) in [
            (vec!["-x", "-m"], unexpected("-x")),
            (vec!["-m", "-x"], unexpected("-x")),
            (vec!["-m", "-T"], missing()),
            (vec!["-x", "-h"], unexpected("-x")),
            (vec!["--bogus", "-V"], unexpected("--bogus")),
            (vec!["--max=1"], no_value("1", "--max")),
            (vec!["--help=1"], no_value("1", "--help")),
            (vec!["-x"], unexpected("-x")),
            (vec!["--bogus"], unexpected("--bogus")),
            (vec!["-x", "-p", "1"], unexpected("-x")),
            (vec!["--ver"], unexpected("--ver")),
            (vec!["--r", "10", "true"], unexpected("--r")),
            (vec!["--s"], unexpected("--s")),
            (vec!["--sched", "1"], unexpected("--sched")),
            (vec!["--sched-", "1", "true"], unexpected("--sched-")),
            (vec!["--pid=1"], no_value("1", "--pid")),
            (
                vec!["--all-tasks=1", "-p", "1"],
                no_value("1", "--all-tasks"),
            ),
            (vec!["--verbose=1", "-p", "1"], no_value("1", "--verbose")),
            (vec!["--other=0", "true"], no_value("0", "--other")),
            (vec!["-p", "-o", "-T"], missing()),
            (vec!["-o0", "true"], unexpected("-0")),
            (vec!["-p", "-1"], unexpected("-1")),
            (vec!["---pid", "1"], unexpected("---pid")),
            // clap reports an unknown option before it judges a value it
            // holds, so `-x` is named where chrt(1) names `abc`.
            (vec!["-T", "abc", "-x"], unexpected("-x")),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains(error);
        }
    }

    /// Where the command starts, one `--` is dropped even when no priority
    /// comes first; a number behind it is then the command.
    #[test]
    fn a_terminator_where_the_command_starts() {
        for policy in ["-o", "-b"] {
            new_ucmd!()
                .args(&[policy, "--", "--", "echo", "hi"])
                .succeeds()
                .stdout_is("hi\n");
        }
        new_ucmd!()
            .args(&["-o", "--", "--"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: no command specified\n");
        for (arguments, command) in [
            (vec!["-o", "--", "--", "0", "echo"], "0"),
            (vec!["-o", "--", "--", "--", "echo"], "--"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(127)
                .no_stdout()
                .stderr_is(format!(
                    "chrt: failed to execute {command}: No such file or directory\n"
                ));
        }
    }

    /// A `--` that starts the command is dropped, under --pid too; one in the
    /// pid's or the priority's place is read as that operand.
    #[test]
    fn a_terminator_after_the_priority() {
        new_ucmd!()
            .args(&["-o", "0", "--"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: no command specified\n");
        new_ucmd!()
            .args(&["-o", "0", "--", "--"])
            .fails()
            .code_is(127)
            .no_stdout()
            .stderr_is("chrt: failed to execute --: No such file or directory\n");
        for arguments in [vec!["-p", "-o", "0", "--"], vec!["-p", "0", "--"]] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is("chrt: invalid PID argument: '--'\n");
        }
        new_ucmd!()
            .args(&["-p", "-o", "0", "--", "0"])
            .fails()
            .code_is(127)
            .no_stdout()
            .stderr_is("chrt: failed to execute 0: No such file or directory\n");
        new_ucmd!()
            .args(&["-p", "--", "--", "1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid priority argument: '--'\n");
    }

    /// With a good pid the priority comes next, then the timing options are
    /// matched to the policy.
    #[test]
    fn the_pid_is_parsed_before_the_priority() {
        for arguments in [
            vec!["-p", "abc", "xyz"],
            vec!["-p", "-f", "abc", "xyz"],
            vec!["-p", "-T", "1", "abc", "xyz"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is("chrt: invalid PID argument: 'xyz'\n");
        }
        // Under -v nothing is shown before the pid is parsed.
        for arguments in [
            vec!["-p", "-f", "abc"],
            vec!["-p", "-i", "-T", "3000000", "abc"],
            vec!["-p", "-o", "-P", "1000000", "abc"],
            vec!["-v", "-p", "-f", "abc"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is("chrt: invalid PID argument: 'abc'\n");
        }
        new_ucmd!()
            .args(&["-p", "-f", "-T", "1", "abc", "1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("chrt: invalid priority argument: 'abc'\n");
        new_ucmd!()
            .args(&["-p", "-f", "-T", "1", "100", "1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(RUNTIME_NOT_SUPPORTED);
    }

    /// The next token whatever it looks like, judged before the operands are
    /// counted, before a later -h, -V or -m, and despite a later good value.
    #[test]
    fn a_timing_value_is_judged_as_it_is_read() {
        for (arguments, name, value) in [
            (vec!["-T", "-o", "true"], "runtime", "-o"),
            (vec!["-p", "--sched-runtime", "-1", "1"], "runtime", "-1"),
            (vec!["-T", "abc"], "runtime", "abc"),
            (vec!["-f", "-T", "abc"], "runtime", "abc"),
            (vec!["-p", "-T", "abc"], "runtime", "abc"),
            (
                vec!["-p", "-o", "-P", "xyz", "-T", "abc", "1"],
                "period",
                "xyz",
            ),
            (
                vec!["-p", "-o", "-T", "abc", "-P", "xyz", "1"],
                "runtime",
                "abc",
            ),
            (
                vec!["-p", "-d", "-D", "xyz", "-P", "abc", "1"],
                "deadline",
                "xyz",
            ),
            (vec!["-p", "-T", "abc", "-T", "1", "1"], "runtime", "abc"),
            (
                vec!["-T", "abc", "-T", "1", "-o", "0", "true"],
                "runtime",
                "abc",
            ),
            (vec!["-T", "abc", "-h"], "runtime", "abc"),
            (vec!["-T", "abc", "-m"], "runtime", "abc"),
            (vec!["-P", "abc", "-V"], "period", "abc"),
            (vec!["-Dx", "-m"], "deadline", "x"),
            (vec!["--sched-runtime=abc", "-m"], "runtime", "abc"),
            (vec!["-T", "-1", "-h"], "runtime", "-1"),
            (vec!["-p", "-T", "abc", "-h", "1"], "runtime", "abc"),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains(invalid_timing(name, value));
        }
        new_ucmd!()
            .args(&["-T", "1000000", "-m"])
            .succeeds()
            .stdout_matches(&max_table());
    }

    #[test]
    fn an_option_byte_that_is_not_valid_unicode_is_an_error() {
        for arguments in [
            vec![OsStr::from_bytes(b"-T\xff"), OsStr::new("-h")],
            vec![OsStr::from_bytes(b"--sched-runtime=\xff"), OsStr::new("-V")],
            vec![
                OsStr::new("-T"),
                OsStr::from_bytes(b"1\xff"),
                OsStr::new("-m"),
            ],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains("invalid UTF-8");
        }
        for arguments in [
            vec![OsStr::from_bytes(b"-p\xff"), OsStr::new("1")],
            vec![OsStr::from_bytes(b"-m\xff")],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_contains("error: unexpected argument '-\u{fffd}' found");
        }
    }

    // -- privilege -------------------------------------------------------------

    #[test]
    fn a_realtime_set_on_a_pid_is_refused_without_privilege() {
        if skipped_with_realtime_privilege() {
            return;
        }
        let target = Target::new();
        for options in [
            vec!["-p", "-f", "10"],
            vec!["-p", "-r", "10"],
            vec!["-p", "10"],
            vec!["-p", "-f", "-R", "1"],
            vec!["-p", "-d", "-P", "1000000"],
            vec!["-p", "-d", "-T", "500000", "-D", "1000000", "-P", "2000000"],
            vec!["-p", "-d", "-D", "1000000"],
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(set_failed(target.pid(), "Operation not permitted"));
        }
        assert!(is_other(target.pid()));
    }

    #[test]
    fn a_set_on_a_process_owned_by_another_user_is_refused() {
        if skipped_as_root() || skipped_unless_someone_else_owns_pid_1() {
            return;
        }
        for options in [vec!["-p", "-o", "0"], vec!["-p", "-i"]] {
            new_ucmd!()
                .args(&options)
                .arg("1")
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(set_failed(1, "Operation not permitted"));
        }
    }

    #[test]
    fn a_verbose_set_that_is_refused_still_shows_the_current_block() {
        if skipped_with_realtime_privilege() {
            return;
        }
        let target = Target::new();
        new_ucmd!()
            .args(&["-p", "-v", "-f", "10", &target.arg()])
            .fails()
            .code_is(1)
            .stdout_matches(&current(target.pid(), "SCHED_OTHER"))
            .stderr_is(set_failed(target.pid(), "Operation not permitted"));
    }

    #[test]
    fn off_a_terminal_an_error_precedes_the_output_before_it() {
        if skipped_with_realtime_privilege() {
            return;
        }
        let target = Target::new();
        let (at, mut ucmd) = at_and_ucmd!();
        let log = at.make_file("log");
        ucmd.args(&["-p", "-v", "-f", "10", &target.arg()])
            .set_stdout(log.try_clone().expect("a second handle on the log"))
            .set_stderr(log)
            .fails()
            .code_is(1);
        let expected = whole(&format!(
            "{}{}",
            regex::escape(&set_failed(target.pid(), "Operation not permitted")),
            block(target.pid(), "current", "SCHED_OTHER", 0)
        ));
        let log = at.read("log");
        assert!(expected.is_match(&log), "{log:?}");
    }

    #[test]
    fn a_verbose_set_that_fails_and_cannot_be_written_reports_both() {
        if skipped_with_realtime_privilege() {
            return;
        }
        let target = Target::new();
        for (options, first) in [
            (
                vec!["-p", "-v", "-f", "10"],
                set_failed(target.pid(), "Operation not permitted"),
            ),
            (vec!["-p", "-v", "-o", "1"], unsupported_priority("1")),
        ] {
            new_ucmd!()
                .args(&options)
                .arg(target.arg())
                .set_stdout(File::create("/dev/full").expect("/dev/full"))
                .fails()
                .code_is(1)
                .stderr_is(format!(
                    "{first}chrt: write error: No space left on device\n"
                ));
        }
        // With nothing to write, the full disk goes unnoticed.
        new_ucmd!()
            .args(&["-p", "-o", "1", &target.arg()])
            .set_stdout(File::create("/dev/full").expect("/dev/full"))
            .fails()
            .code_is(1)
            .stderr_is(unsupported_priority("1"));
        assert!(is_other(target.pid()));
    }

    /// The threads are changed in task list order, the leader first, and the
    /// walk stops at the first refusal, which is reported as a tid.
    #[test]
    fn all_tasks_stops_at_the_first_refused_thread() {
        if skipped_with_realtime_privilege() {
            return;
        }
        let target = ThreadedTarget::new();
        new_ucmd!()
            .args(&["-a", "-p", "-f", "10", &target.arg()])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(format!(
                "chrt: failed to set tid {}'s policy: Operation not permitted\n",
                target.pid()
            ));
        for &tid in &target.tids {
            assert!(is_other(tid), "tid {tid} was changed");
        }
    }

    /// The pid in the message is chrt's own, spelled 0, and a missing command is
    /// not what is reported.
    #[test]
    fn a_refused_set_keeps_the_command_from_running() {
        if skipped_with_realtime_privilege() {
            return;
        }
        for arguments in [
            vec!["-f", "10", "true"],
            vec!["-r", "10", "true"],
            vec!["10", "true"],
            vec!["-f", "10", "/nonexistent"],
            vec!["-d", "-P", "1000000", "true"],
            vec!["-v", "-f", "10", "true"],
            // A number followed by `--` is a priority: the set comes first.
            vec!["-f", "10", "--"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(set_failed(0, "Operation not permitted"));
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use uutests::new_ucmd;

    /// The command's own options are the command's here too: they must not
    /// turn the platform message into a parse error.
    #[test]
    fn fails_on_unsupported_platforms() {
        for arguments in [vec!["-p", "1"], vec!["-o", "0", "ls", "-l"], vec!["-m"]] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .stderr_is("chrt: `chrt` is available only on Linux.\n");
        }
        new_ucmd!().arg("-h").succeeds().stdout_contains("Usage:");
    }
}
