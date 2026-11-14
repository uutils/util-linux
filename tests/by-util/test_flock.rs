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
fn test_missing_args() {
    new_ucmd!()
        .fails()
        .code_is(1)
        .stderr_contains("required arguments were not provided");
}

#[test]
fn test_short_v_is_invalid() {
    new_ucmd!()
        .args(&["-v", "file", "true"])
        .fails()
        .code_is(1)
        .stderr_contains("unexpected argument '-v'");
}

#[test]
fn test_command_flag_and_positional_conflict() {
    new_ucmd!()
        .args(&["file", "-c", "echo hi", "extra_arg"])
        .fails()
        .stderr_contains("cannot be used with");
}

#[test]
fn test_conflict_exit_code_out_of_range() {
    new_ucmd!()
        .args(&["-E", "300", "lockfile", "true"])
        .fails()
        .code_is(1)
        .stderr_contains("300 is not in 0..=255");
}

#[cfg(unix)]
mod unix {
    use std::fs::File;
    use std::os::fd::AsRawFd;
    use uutests::new_ucmd;
    use uutests::util::{get_tests_binary, TestScenario};
    use uutests::util_name;

    fn scenario_with_lockfile() -> (TestScenario, String) {
        let ts = TestScenario::new(util_name!());
        let lockfile = ts.fixtures.plus_as_string("lockfile");
        (ts, lockfile)
    }

    #[test]
    fn test_lock_mode_override() {
        let (ts, lockfile) = scenario_with_lockfile();
        ts.ucmd().args(&["-s", "-x", &lockfile, "true"]).succeeds();
        ts.ucmd().args(&["-u", "-s", &lockfile, "true"]).succeeds();
        ts.ucmd().args(&["-s", "-s", &lockfile, "true"]).succeeds();
    }

    #[test]
    fn test_executes_command_and_creates_lockfile() {
        let ts = TestScenario::new(util_name!());
        let lockfile = ts.fixtures.plus("new_lockfile");

        ts.ucmd()
            .args(&[lockfile.to_str().unwrap(), "echo", "hello", "world"])
            .succeeds()
            .stdout_is("hello world\n");

        assert!(lockfile.exists());
    }

    #[test]
    fn test_dash_c_command() {
        let (ts, lockfile) = scenario_with_lockfile();

        ts.ucmd()
            .args(&[&lockfile, "-c", "echo from_shell"])
            .succeeds()
            .stdout_is("from_shell\n");
    }

    #[test]
    fn test_propagates_child_exit_code() {
        let (ts, lockfile) = scenario_with_lockfile();

        ts.ucmd()
            .args(&[&lockfile, "-c", "exit 42"])
            .fails()
            .code_is(42);
    }

    #[test]
    fn test_exclusive_lock_conflict_and_custom_exit_code() {
        let (ts, lockfile) = scenario_with_lockfile();
        let file = File::create(&lockfile).unwrap();

        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );

        ts.ucmd()
            .args(&["-n", &lockfile, "true"])
            .fails()
            .code_is(1)
            .no_output();

        ts.ucmd()
            .args(&["-n", "-E", "123", &lockfile, "true"])
            .fails()
            .code_is(123)
            .no_output();
    }

    #[test]
    fn test_shared_locks_do_not_conflict() {
        let (ts, lockfile) = scenario_with_lockfile();
        let file = File::create(&lockfile).unwrap();

        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) },
            0
        );

        ts.ucmd().args(&["-s", "-n", &lockfile, "true"]).succeeds();

        ts.ucmd()
            .args(&["-x", "-n", &lockfile, "true"])
            .fails()
            .code_is(1);
    }

    #[test]
    fn test_lock_directory() {
        let ts = TestScenario::new(util_name!());
        ts.ucmd()
            .args(&["-n", &ts.fixtures.as_string(), "true"])
            .succeeds();
    }

    #[test]
    fn test_invalid_fd() {
        new_ucmd!()
            .arg("77")
            .fails()
            .code_is(65)
            .stderr_contains("77: Bad file descriptor");
    }

    #[test]
    fn test_unparseable_fd() {
        new_ucmd!()
            .arg("not_a_fd")
            .fails()
            .code_is(64)
            .stderr_contains("bad file descriptor");
    }

    #[test]
    fn test_unopenable_file() {
        new_ucmd!()
            .args(&["/nonexistent_dir_for_flock/lockfile", "true"])
            .fails()
            .code_is(66)
            .stderr_contains("cannot open lock file");
    }

    #[test]
    fn test_missing_command_binary() {
        let (ts, lockfile) = scenario_with_lockfile();
        ts.ucmd()
            .args(&[&lockfile, "/this_binary_does_not_exist_12345"])
            .fails()
            .code_is(69)
            .stderr_is(
                "flock: failed to execute /this_binary_does_not_exist_12345: No such file or directory\n",
            );
    }

    #[test]
    fn test_fd_form_and_unlock() {
        let (ts, lockfile) = scenario_with_lockfile();
        let bin = get_tests_binary();

        let script = format!(
            "exec 9>'{lockfile}'; \
             '{bin}' flock -n 9 && \
             ! '{bin}' flock -n '{lockfile}' true && \
             '{bin}' flock -u 9 && \
             '{bin}' flock -n '{lockfile}' true"
        );

        ts.cmd("sh").args(&["-c", &script]).succeeds();
    }

    #[test]
    fn test_child_killed_by_signal() {
        let (ts, lockfile) = scenario_with_lockfile();

        ts.ucmd()
            .args(&[&lockfile, "sh", "-c", "kill -9 $$"])
            .fails()
            .code_is(137);
    }

    #[test]
    fn test_timeout_expires() {
        let (ts, lockfile) = scenario_with_lockfile();
        let file = File::create(&lockfile).unwrap();
        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );

        ts.ucmd()
            .args(&["-w", "0.05", "-E", "77", &lockfile, "true"])
            .fails()
            .code_is(77);
    }

    #[test]
    fn test_timeout_succeeds_when_lock_released() {
        use std::time::Duration;

        let (ts, lockfile) = scenario_with_lockfile();
        let file = File::create(&lockfile).unwrap();
        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );

        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(file);
        });

        ts.ucmd().args(&["-w", "5", &lockfile, "true"]).succeeds();
        handle.join().unwrap();
    }

    #[test]
    fn test_invalid_timeouts() {
        let (ts, lockfile) = scenario_with_lockfile();

        for invalid in ["-1", "nan", "1e20", "inf"] {
            ts.ucmd()
                .args(&["-w", invalid, &lockfile, "true"])
                .fails()
                .code_is(71)
                .stderr_contains("cannot set up timer: Invalid argument");
        }
    }

    #[test]
    fn test_nonblocking_long_and_alias() {
        let (ts, lockfile) = scenario_with_lockfile();

        let file = File::create(&lockfile).unwrap();
        assert_eq!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        for flag in ["--nonblocking", "--nonblock", "--nb"] {
            ts.ucmd()
                .args(&[flag, &lockfile, "true"])
                .fails()
                .code_is(1)
                .no_output();
        }
    }
}

#[cfg(not(unix))]
mod non_unix {
    use uutests::new_ucmd;

    #[test]
    fn test_unsupported_platform() {
        new_ucmd!()
            .args(&["lockfile", "true"])
            .fails()
            .code_is(1)
            .stderr_contains("`flock` is unavailable on this platform");
    }
}
