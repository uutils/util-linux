// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_family = "unix")]
mod unix {
    use uutests::new_ucmd;
    use uutests::util::get_tests_binary;
    use uutests::util::UCommand;

    #[test]
    fn test_invalid_arg() {
        new_ucmd!().arg("--definitely-invalid").fails().code_is(1);

        new_ucmd!()
            .arg("-f")
            .fails()
            .code_is(1)
            .stderr_is("setsid: no command specified\n");
    }

    #[test]
    fn fork_isolates_child_exit_code() {
        new_ucmd!()
            .arg("-f")
            .arg("/usr/bin/false")
            .succeeds()
            .no_output();
    }

    #[test]
    fn non_fork_returns_child_exit_code() {
        new_ucmd!()
            .arg("/usr/bin/false")
            .fails()
            .code_is(1)
            .no_output();
    }

    #[test]
    fn fork_wait_returns_child_exit_code() {
        new_ucmd!()
            .arg("-f")
            .arg("-w")
            .arg("/usr/bin/false")
            .fails()
            .code_is(1)
            .no_output();
    }

    #[test]
    fn non_fork_returns_not_found_error() {
        new_ucmd!()
        .arg("/usr/bin/this-tool-does-not-exist-hopefully")
        .fails()
        .code_is(127)
        .stderr_is("setsid: failed to execute /usr/bin/this-tool-does-not-exist-hopefully: No such file or directory\n");
    }

    #[test]
    fn non_fork_on_non_executable_returns_permission_denied_error() {
        new_ucmd!()
            .arg("/etc/passwd")
            .fails()
            .code_is(126)
            .stderr_is("setsid: failed to execute /etc/passwd: Permission denied\n");
    }

    #[test]
    fn fork_isolates_not_found_error() {
        new_ucmd!()
            .arg("-f")
            .arg("/usr/bin/this-tool-does-not-exist-hopefully")
            .succeeds();
        // no test for output, as it's a race whether the not found error gets printed
        // quickly enough, potential flakyness
    }

    #[test]
    fn unprivileged_user_cannot_steal_controlling_tty() {
        let shell_cmd = format!(
            "{} setsid -w -c {} setsid -w -c /b/usrin/true",
            get_tests_binary(),
            get_tests_binary()
        );
        UCommand::new()
            .terminal_simulation(true)
            .arg(&shell_cmd)
            .fails()
            .code_is(1)
            .no_stdout()
            .stderr_is("setsid: failed to set the controlling terminal: Permission denied\r\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unprivileged_user_can_take_new_controlling_tty() {
        let shell_cmd = format!(
            "/usr/bin/cat /proc/self/stat; {} setsid -w -c /usr/bin/cat /proc/self/stat",
            get_tests_binary()
        );

        let cmd_result = UCommand::new()
            .terminal_simulation(true)
            .arg(&shell_cmd)
            .succeeds();

        let output = cmd_result.code_is(0).no_stderr().stdout_str();

        // /proc/self/stat format has controlling terminal as the 7th
        // space-separated item; if we managed to change the controlling
        // terminal, we should see a difference there
        let (before, after) = output
            .split_once('\n')
            .expect("expected 2 lines of output at least");
        let before = before
            .split_whitespace()
            .nth(6)
            .expect("unexpected stat format");
        let after = after
            .split_whitespace()
            .nth(6)
            .expect("unexpected stat format");

        assert_ne!(before, after);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn setsid_takes_session_leadership() {
        let shell_cmd = format!(
            "/usr/bin/cat /proc/self/stat; {} setsid /usr/bin/cat /proc/self/stat",
            get_tests_binary()
        );

        let cmd_result = UCommand::new()
            .terminal_simulation(true)
            .arg(&shell_cmd)
            .succeeds();

        let output = cmd_result.no_stderr().stdout_str();

        // /proc/self/stat format has session ID as the 6th space-separated
        // item; if we managed to get session leadership, we should see a
        // difference there...
        let (before, after) = output
            .split_once('\n')
            .expect("expected 2 lines of output at least");
        let before = before
            .split_whitespace()
            .nth(5)
            .expect("unexpected stat format");
        let after = after
            .split_whitespace()
            .nth(5)
            .expect("unexpected stat format");

        assert_ne!(before, after);

        // ...and it should actually be the PID of our child! We take the child
        // PID here to avoid differences in handling by different shells or
        // distributions.
        let pid = after.split_whitespace().next().unwrap();
        assert_eq!(after, pid);
    }
}

#[cfg(not(target_family = "unix"))]
mod non_unix {
    use uutests::new_ucmd;

    #[test]
    fn unsupported_platforms() {
        new_ucmd!()
            .arg("/usr/bin/true")
            .fails()
            .code_is(1)
            .stderr_is("setsid: `setsid` is unavailable on non-UNIX-like platforms.\n");
    }
}
