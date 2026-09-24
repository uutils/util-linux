// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) EACCES classdata ioprio pgid strace

use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[cfg(target_os = "linux")]
mod linux {
    use regex::Regex;
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::process::{Child, Command};
    use uutests::at_and_ucmd;
    use uutests::new_ucmd;
    use uutests::util::{get_tests_binary, UCommand};

    /// Run our own ionice under the ionice being tested, so that the priority
    /// just set can be read back from the process that inherited it. Without
    /// this the assertions would depend on whatever priority the test runner
    /// happens to be running at.
    fn nested(options: &[&str]) -> UCommand {
        let mut command = new_ucmd!();
        command.args(options).arg(get_tests_binary()).arg("ionice");
        command
    }

    /// Any line a get can legitimately print. The priority of a process the
    /// test did not create is not fixed, so tests that read one assert only its
    /// shape.
    fn class_line() -> Regex {
        Regex::new(r"^(?:idle|(?:none|realtime|best-effort): prio \d+)\n$").unwrap()
    }

    /// A process id that cannot exist: pid_max is bounded well below this.
    const ABSENT_PID: &str = "2147483647";

    // -- reading a priority --------------------------------------------------

    #[test]
    fn reports_the_calling_process_by_default() {
        new_ucmd!()
            .succeeds()
            .no_stderr()
            .stdout_matches(&class_line());
    }

    #[test]
    fn ignore_alone_still_reads() {
        new_ucmd!()
            .arg("-t")
            .succeeds()
            .no_stderr()
            .stdout_matches(&class_line());
    }

    /// The reference accepts a repeated -t, so the flag must stay countable
    /// rather than become a boolean that rejects its second appearance.
    #[test]
    fn ignore_may_be_repeated() {
        new_ucmd!()
            .args(&["-t", "-t"])
            .succeeds()
            .no_stderr()
            .stdout_matches(&class_line());
    }

    #[test]
    fn reads_by_pid() {
        new_ucmd!()
            .args(&["-p", "1"])
            .succeeds()
            .no_stderr()
            .stdout_matches(&class_line());
    }

    #[test]
    fn reads_by_pgid() {
        let group = unsafe { libc::getpgid(0) };
        new_ucmd!()
            .args(&["-P", &group.to_string()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&class_line());
    }

    #[test]
    fn reads_by_uid() {
        let user = unsafe { libc::getuid() };
        new_ucmd!()
            .args(&["-u", &user.to_string()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&class_line());
    }

    #[test]
    fn reads_one_line_per_id() {
        new_ucmd!()
            .args(&["-p", "1", "1"])
            .succeeds()
            .no_stderr()
            .stdout_matches(&Regex::new(r"^(?:.+\n){2}$").unwrap());
    }

    /// An id of 0 on the read path means the calling process, not the group or
    /// the user numbered 0. Reading it back under a known class is what makes
    /// this observable.
    #[test]
    fn a_lone_zero_id_means_the_caller() {
        for option in ["-u", "-P", "-p"] {
            nested(&["-c", "3"])
                .args(&[option, "0"])
                .succeeds()
                .no_stderr()
                .stdout_is("idle\n");
        }
    }

    #[test]
    fn a_zero_id_with_more_ids_does_not_mean_the_caller() {
        let user = unsafe { libc::getuid() };
        new_ucmd!()
            .args(&["-u", "0", &user.to_string()])
            .succeeds()
            .no_stderr()
            .stdout_matches(&Regex::new(r"^(?:.+\n){2}$").unwrap());
    }

    #[test]
    fn reading_an_absent_pid_fails() {
        new_ucmd!()
            .args(&["-p", ABSENT_PID])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("ionice: ioprio_get failed: No such process\n");
    }

    #[test]
    fn ignore_does_not_silence_a_failed_read() {
        new_ucmd!()
            .args(&["-t", "-p", ABSENT_PID])
            .fails()
            .code_is(1)
            .stderr_is("ionice: ioprio_get failed: No such process\n");
    }

    /// Each id is parsed only when its turn comes, so the ids before a bad one
    /// have already been reported.
    #[test]
    fn reading_stops_at_the_first_bad_id() {
        new_ucmd!()
            .args(&["-p", "1", "bogus"])
            .fails()
            .code_is(1)
            .stdout_matches(&class_line())
            .stderr_is("ionice: invalid PID argument: 'bogus'\n");
    }

    #[test]
    fn reading_stops_at_the_first_absent_id() {
        new_ucmd!()
            .args(&["-p", ABSENT_PID, "1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("ionice: ioprio_get failed: No such process\n");
    }

    // -- setting a priority, read back through an exec'd child ---------------

    #[test]
    fn defaults_to_best_effort_level_four() {
        nested(&[])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 4\n");
    }

    #[test]
    fn sets_class_by_number() {
        nested(&["-c", "0"])
            .succeeds()
            .no_stderr()
            .stdout_is("none: prio 0\n");
        nested(&["-c", "2"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 4\n");
        nested(&["-c", "3"])
            .succeeds()
            .no_stderr()
            .stdout_is("idle\n");
    }

    #[test]
    fn sets_class_by_name_ignoring_case() {
        for name in ["idle", "IDLE", "Idle"] {
            nested(&["-c", name])
                .succeeds()
                .no_stderr()
                .stdout_is("idle\n");
        }
        nested(&["-c", "best-effort"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 4\n");
    }

    #[test]
    fn class_names_must_be_spelled_in_full() {
        new_ucmd!()
            .args(&["-c", "be"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: unknown scheduling class: 'be'\n");
    }

    #[test]
    fn sets_level_within_the_class() {
        nested(&["-c", "2", "-n", "0"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 0\n");
        nested(&["-c", "2", "-n", "7"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 7\n");
    }

    #[test]
    fn a_level_without_a_class_means_best_effort() {
        nested(&["-n", "6"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 6\n");
    }

    /// Neither class takes a level, so one given with -n is dropped.
    #[test]
    fn idle_and_none_drop_a_given_level() {
        nested(&["-c", "3", "-n", "5"])
            .succeeds()
            .stdout_is("idle\n")
            .stderr_is("ionice: ignoring given class data for idle class\n");
        nested(&["-c", "0", "-n", "5"])
            .succeeds()
            .stdout_is("none: prio 0\n")
            .stderr_is("ionice: ignoring given class data for none class\n");
    }

    #[test]
    fn ignore_silences_the_dropped_level_warning() {
        nested(&["-t", "-c", "3", "-n", "5"])
            .succeeds()
            .no_stderr()
            .stdout_is("idle\n");
        nested(&["-t", "-c", "0", "-n", "5"])
            .succeeds()
            .no_stderr()
            .stdout_is("none: prio 0\n");
    }

    #[test]
    fn the_last_class_wins() {
        nested(&["-c", "2", "-n", "5", "-c", "3"])
            .succeeds()
            .stdout_is("idle\n")
            .stderr_is("ionice: ignoring given class data for idle class\n");
        nested(&["-c", "3", "-n", "5", "-c", "2"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 5\n");
    }

    /// ionice(1) documents -n as a level of 0 to 7 and the reference checks
    /// nothing: the value goes into the ioprio word as it stands. So `-n 8`
    /// stays best-effort but means level 0, the kernel reading the level
    /// modulo eight; `-n 8192` carries into the class field and becomes idle;
    /// and `-n -1` makes a word the kernel refuses. This port rejects all
    /// three before any syscall - see uutils/util-linux#624.
    #[test]
    fn the_level_is_range_checked() {
        for level in ["8", "8192", "-1"] {
            new_ucmd!()
                .args(&["-n", level, "true"])
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!(
                    "ionice: invalid class data argument: '{level}': must be 0-7\n"
                ));
        }
    }

    /// The level is judged as it is parsed, before the class has any say, so
    /// a class that would discard it anyway does not excuse a value outside
    /// the documented range.
    #[test]
    fn a_level_the_class_discards_is_still_range_checked() {
        new_ucmd!()
            .args(&["-c", "3", "-n", "9", "true"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("ionice: invalid class data argument: '9': must be 0-7\n");
    }

    /// ionice(1) documents -c as 0 to 3. The reference warns `unknown prio
    /// class N` and carries on regardless, letting the kernel take the class
    /// from three bits: `-c 10` runs best-effort and `-c 99` runs idle, both
    /// exiting 0, while `-c 4` wraps onto a class the kernel refuses. Only -t
    /// makes the wrap silent. This port rejects all three - see
    /// uutils/util-linux#624.
    #[test]
    fn the_class_is_range_checked() {
        for class in ["4", "10", "99"] {
            new_ucmd!()
                .args(&["-c", class, "true"])
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!(
                    "ionice: invalid class argument: '{class}': must be 0-3\n"
                ));
        }
    }

    /// -t silences a failure to set a priority, not a refusal to accept the
    /// argument: a rejected value never reaches a syscall.
    #[test]
    fn ignore_does_not_silence_a_rejected_value() {
        for (arguments, message) in [
            (
                ["-t", "-c", "99", "true"],
                "invalid class argument: '99': must be 0-3",
            ),
            (
                ["-t", "-n", "99", "true"],
                "invalid class data argument: '99': must be 0-7",
            ),
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is(format!("ionice: {message}\n"));
        }
    }

    /// -t keeps a refused set from stopping the command. Realtime is the one
    /// class an unprivileged caller cannot have, which is what makes the set
    /// fail here.
    #[test]
    fn ignore_silences_a_failed_set_and_still_runs_the_command() {
        if skipped_as_root() {
            return;
        }
        new_ucmd!()
            .args(&["-t", "-c", "1", "echo", "ok"])
            .succeeds()
            .no_stderr()
            .stdout_is("ok\n");
    }

    #[test]
    fn long_options_are_accepted() {
        nested(&["--class=idle"])
            .succeeds()
            .no_stderr()
            .stdout_is("idle\n");
        nested(&["--class", "2", "--classdata", "3"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 3\n");
    }

    // -- setting a priority by id --------------------------------------------

    /// The word ioprio_get returns for a process an idle set was applied to,
    /// spelled out from the kernel ABI so that the assertion shares nothing
    /// with the tool's own encoder: class 3 above the 13-bit level field. The
    /// level is 7 because that is what the reference packs with an idle set,
    /// observed under strace; stdout cannot see it, because an idle priority
    /// prints as the bare word "idle" whatever level it carries.
    const IDLE_WORD: libc::c_long = (3 << 13) | 7;

    /// Read the raw priority word with a direct syscall, bypassing the code
    /// under test.
    fn raw_ioprio_of(pid: u32) -> libc::c_long {
        const IOPRIO_WHO_PROCESS: libc::c_long = 1;
        // SAFETY: ioprio_get takes two integers by value and touches no memory.
        unsafe {
            libc::syscall(
                libc::SYS_ioprio_get,
                IOPRIO_WHO_PROCESS,
                pid as libc::c_long,
            )
        }
    }

    /// A set is applied to the option's id and to every trailing id, and it
    /// reports nothing.
    #[test]
    fn a_set_applies_to_every_given_id() {
        let mut children: Vec<Child> = (0..2)
            .map(|_| {
                Command::new("sleep")
                    .arg("10")
                    .spawn()
                    .expect("spawn sleep")
            })
            .collect();
        let ids: Vec<String> = children
            .iter()
            .map(|child| child.id().to_string())
            .collect();

        new_ucmd!()
            .args(&["-c", "3", "-p", ids[0].as_str(), ids[1].as_str()])
            .succeeds()
            .no_output();

        let words: Vec<_> = children
            .iter()
            .map(|child| raw_ioprio_of(child.id()))
            .collect();

        for child in &mut children {
            child.kill().expect("kill sleep");
            child.wait().expect("reap sleep");
        }

        assert_eq!(words, [IDLE_WORD, IDLE_WORD]);
    }

    // -- rejecting bad arguments ---------------------------------------------

    #[test]
    fn rejects_an_unknown_class_name() {
        for value in ["bogus", "", " 3"] {
            new_ucmd!()
                .args(&["-c", value])
                .fails()
                .code_is(1)
                .stderr_is(format!("ionice: unknown scheduling class: '{value}'\n"));
        }
    }

    /// A leading digit selects the numeric form, so a leading blank and a
    /// trailing blank are reported differently.
    #[test]
    fn rejects_a_malformed_class_number() {
        for value in ["0x3", "3 ", "3x"] {
            new_ucmd!()
                .args(&["-c", value])
                .fails()
                .code_is(1)
                .stderr_is(format!("ionice: invalid class argument: '{value}'\n"));
        }
    }

    /// A signed number has no leading digit, so it lands in the name branch.
    #[test]
    fn a_signed_class_number_is_taken_for_a_name() {
        new_ucmd!()
            .args(&["-c", "+3"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: unknown scheduling class: '+3'\n");
    }

    /// A value too wide for an i32 is reported as such, ahead of the
    /// documented range: that failure is the reference's own.
    #[test]
    fn rejects_an_oversized_class_number() {
        new_ucmd!()
            .args(&["-c", "4294967296"])
            .fails()
            .code_is(1)
            .stderr_contains("ionice: invalid class argument: '4294967296': ")
            .stderr_does_not_contain("must be ");
    }

    /// Every numeric option, not only -c, attaches a detail to an oversized
    /// value, and it is the overflow rather than the documented range.
    #[test]
    fn an_oversized_number_carries_a_detail_on_every_option() {
        for (option, name) in [
            ("-n", "class data"),
            ("-p", "PID"),
            ("-P", "PGID"),
            ("-u", "UID"),
        ] {
            new_ucmd!()
                .args(&[option, "4294967296"])
                .fails()
                .code_is(1)
                .stderr_contains(format!("ionice: invalid {name} argument: '4294967296': "))
                .stderr_does_not_contain("must be ");
        }
    }

    #[test]
    fn rejects_a_malformed_level() {
        for value in ["bogus", "0x3", "5 ", ""] {
            new_ucmd!()
                .args(&["-n", value])
                .fails()
                .code_is(1)
                .stderr_is(format!("ionice: invalid class data argument: '{value}'\n"));
        }
    }

    /// Unlike -c, -n has no name form, so a leading blank reaches the number
    /// parser and is skipped.
    #[test]
    fn a_level_may_carry_leading_blanks() {
        nested(&["-n", " 5"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 5\n");
    }

    /// The number parser takes an optional sign, so +5 is level 5.
    #[test]
    fn a_level_may_carry_a_plus_sign() {
        nested(&["-n", "+5"])
            .succeeds()
            .no_stderr()
            .stdout_is("best-effort: prio 5\n");
    }

    #[test]
    fn rejects_a_malformed_id() {
        for (option, name) in [("-p", "PID"), ("-P", "PGID"), ("-u", "UID")] {
            new_ucmd!()
                .args(&[option, "bogus"])
                .fails()
                .code_is(1)
                .stderr_is(format!("ionice: invalid {name} argument: 'bogus'\n"));
        }
    }

    #[test]
    fn does_not_resolve_user_names() {
        new_ucmd!()
            .args(&["-u", "root"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: invalid UID argument: 'root'\n");
    }

    /// The value after an id option is taken verbatim, even when it looks like
    /// another option.
    #[test]
    fn an_id_value_may_start_with_a_hyphen() {
        new_ucmd!()
            .args(&["-p", "-c", "3"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: invalid PID argument: '-c'\n");
    }

    #[test]
    fn only_one_kind_of_id_at_a_time() {
        for arguments in [
            ["-p", "1", "-P", "1"],
            ["-p", "1", "-u", "0"],
            ["-p", "1", "-p", "2"],
        ] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .stderr_is("ionice: can handle only one of pid, pgid or uid at once\n");
        }
    }

    /// The options are judged in the order they were written, so it is always
    /// the leftmost bad one that is reported.
    #[test]
    fn the_first_bad_option_is_the_one_reported() {
        new_ucmd!()
            .args(&["-P", "1", "-p", "bogus"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: can handle only one of pid, pgid or uid at once\n");

        new_ucmd!()
            .args(&["-p", "bogus", "-P", "1"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: invalid PID argument: 'bogus'\n");

        new_ucmd!()
            .args(&["-n", "8", "-p", "bogus"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: invalid class data argument: '8': must be 0-7\n");

        new_ucmd!()
            .args(&["-c", "bogus", "-p", "bogus"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: unknown scheduling class: 'bogus'\n");
    }

    #[test]
    fn a_priority_with_nothing_to_apply_it_to_is_a_usage_error() {
        for arguments in [vec!["-c", "3"], vec!["-n", "5"], vec!["-t", "-c", "3"]] {
            new_ucmd!()
                .args(&arguments)
                .fails()
                .code_is(1)
                .no_stdout()
                .stderr_is("ionice: bad usage\nTry 'ionice --help' for more information.\n");
        }
    }

    /// The options are resolved before the missing target is noticed, so a
    /// rejected value is the only thing reported.
    #[test]
    fn a_rejected_class_preempts_the_usage_error() {
        new_ucmd!()
            .args(&["-c", "4"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("ionice: invalid class argument: '4': must be 0-3\n");
    }

    /// A warning keeps that ordering too: the priority is settled, and says
    /// what it discarded, before the usage error.
    #[test]
    fn the_dropped_level_warning_comes_before_the_usage_error() {
        new_ucmd!()
            .args(&["-c", "3", "-n", "5"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is(
                "ionice: ignoring given class data for idle class\nionice: bad usage\n\
                 Try 'ionice --help' for more information.\n",
            );
    }

    /// Argument bytes that are not valid Unicode must reach the utility, so
    /// that a bad value is diagnosed here rather than refused by the parser
    /// before ionice sees it at all. Only the diagnosis is asserted; the value
    /// echoed back is rendered lossily, which is a divergence of its own.
    #[test]
    fn an_option_value_that_is_not_valid_unicode_is_diagnosed_here() {
        new_ucmd!()
            .arg("-p")
            .arg(OsStr::from_bytes(b"1\xff"))
            .fails()
            .code_is(1)
            .stderr_contains("invalid PID argument: ");
    }

    // -- running a command ---------------------------------------------------

    #[test]
    fn arguments_after_the_command_belong_to_the_command() {
        new_ucmd!()
            .args(&["-c", "3", "echo", "-p", "5"])
            .succeeds()
            .stdout_is("-p 5\n");

        new_ucmd!()
            .args(&["echo", "-c", "3"])
            .succeeds()
            .stdout_is("-c 3\n");

        new_ucmd!()
            .args(&["-c", "3", "--", "echo", "-c"])
            .succeeds()
            .stdout_is("-c\n");
    }

    #[test]
    fn the_command_exit_status_is_passed_through() {
        new_ucmd!()
            .args(&["-c", "3", "sh", "-c", "exit 42"])
            .fails()
            .code_is(42)
            .no_output();
    }

    #[test]
    fn a_command_that_does_not_exist_fails() {
        new_ucmd!()
            .args(&["-c", "3", "this-command-does-not-exist-hopefully"])
            .fails()
            .code_is(127)
            .stderr_is(
                "ionice: failed to execute this-command-does-not-exist-hopefully: \
                 No such file or directory\n",
            );
    }

    #[test]
    fn a_command_that_cannot_be_executed_fails() {
        new_ucmd!()
            .args(&["-c", "3", "."])
            .fails()
            .code_is(126)
            .stderr_is("ionice: failed to execute .: Permission denied\n");
    }

    #[test]
    fn ignore_does_not_silence_a_failed_exec() {
        new_ucmd!()
            .args(&["-t", "this-command-does-not-exist-hopefully"])
            .fails()
            .code_is(127)
            .stderr_contains("failed to execute");
    }

    /// The bytes of a command name must reach exec unchanged. A directory is
    /// the discriminator: exec refuses one with EACCES, so a 126 proves the
    /// name arrived intact, where bytes flattened into replacement characters
    /// would have named nothing and failed with 127.
    #[test]
    fn a_command_name_that_is_not_valid_unicode_reaches_exec_unchanged() {
        let (at, mut ucmd) = at_and_ucmd!();
        let name = OsStr::from_bytes(b"not-\xffunicode");
        std::fs::create_dir(at.plus(name)).unwrap();

        ucmd.args(&["-c", "3"])
            .arg(at.plus(name))
            .fails()
            .code_is(126)
            .stderr_contains("Permission denied");
    }

    // -- privilege -----------------------------------------------------------

    /// The tests that call this expect a set to be refused, which holds only
    /// while the caller lacks the privilege to make it succeed: root may take
    /// the realtime class, and root setting idle on pid 1 really does re-class
    /// init rather than earning the EPERM the test expects. Two cases this
    /// does not cover, both absent from CI: a user namespace whose pid 1 the
    /// caller owns, and a caller carrying CAP_SYS_NICE.
    fn skipped_as_root() -> bool {
        if unsafe { libc::geteuid() } == 0 {
            println!("test skipped: running as root would let the set succeed");
            return true;
        }
        false
    }

    /// pid 1 belongs to root, and it is that ownership - not the target being
    /// another process - that earns the EPERM: a caller may lower a process it
    /// owns, as `a_set_applies_to_every_given_id` shows.
    #[test]
    fn a_set_on_a_process_owned_by_another_user_is_refused() {
        if skipped_as_root() {
            return;
        }
        new_ucmd!()
            .args(&["-c", "3", "-p", "1"])
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("ionice: ioprio_set failed: Operation not permitted\n");
    }

    #[test]
    fn ignore_silences_a_refused_set() {
        if skipped_as_root() {
            return;
        }
        new_ucmd!()
            .args(&["-t", "-c", "3", "-p", "1"])
            .succeeds()
            .no_output();
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use uutests::new_ucmd;

    #[test]
    fn fails_on_unsupported_platforms() {
        new_ucmd!()
            .args(&["-p", "1"])
            .fails()
            .code_is(1)
            .stderr_is("ionice: `ionice` is available only on Linux.\n");
    }
}
