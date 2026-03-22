// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
mod linux {
    use uutests::new_ucmd;

    #[test]
    fn test_get_affinity_of_self() {
        let pid = std::process::id().to_string();
        new_ucmd!()
            .args(&["-p", &pid])
            .succeeds()
            .stdout_contains("current affinity mask");
    }

    #[test]
    fn test_get_affinity_list_format() {
        let pid = std::process::id().to_string();
        new_ucmd!()
            .args(&["-c", "-p", &pid])
            .succeeds()
            .stdout_contains("current affinity list");
    }

    #[test]
    fn test_get_affinity_all_tasks() {
        let pid = std::process::id().to_string();
        new_ucmd!()
            .args(&["-a", "-p", &pid])
            .succeeds()
            .stdout_contains("current affinity mask");
    }

    #[test]
    fn test_set_affinity_of_self() {
        let pid = std::process::id().to_string();
        // Read current mask and set it back to itself — always safe
        let output = new_ucmd!().args(&["-p", &pid]).succeeds();
        let stdout = output.stdout_str();
        let mask = stdout.trim().split_whitespace().last().unwrap().to_string();
        new_ucmd!()
            .args(&["-p", &mask, &pid])
            .succeeds()
            .stdout_contains("current affinity mask")
            .stdout_contains("new affinity mask");
    }

    #[test]
    fn test_set_affinity_all_tasks() {
        let pid = std::process::id().to_string();
        let output = new_ucmd!().args(&["-p", &pid]).succeeds();
        let stdout = output.stdout_str();
        let mask = stdout.trim().split_whitespace().last().unwrap().to_string();
        new_ucmd!()
            .args(&["-a", "-p", &mask, &pid])
            .succeeds()
            .stdout_contains("current affinity mask")
            .stdout_contains("new affinity mask");
    }

    #[test]
    fn test_exec_with_comma_hex_mask() {
        // Comma-separated hex mask format as produced by /proc/<pid>/status
        new_ucmd!()
            .args(&["0,1", "/usr/bin/true"])
            .succeeds()
            .no_output();
    }

    #[test]
    fn test_exec_with_hex_mask() {
        new_ucmd!()
            .args(&["0x1", "/usr/bin/true"])
            .succeeds()
            .no_output();
    }

    #[test]
    fn test_exec_with_cpu_list() {
        new_ucmd!()
            .args(&["-c", "0", "/usr/bin/true"])
            .succeeds()
            .no_output();
    }

    #[test]
    fn test_missing_pid() {
        new_ucmd!()
            .arg("-p")
            .fails()
            .code_is(1)
            .stderr_contains("missing argument: PID");
    }

    #[test]
    fn test_missing_command() {
        new_ucmd!()
            .arg("0x1")
            .fails()
            .code_is(1)
            .stderr_contains("mask/list and command required");
    }

    #[test]
    fn test_invalid_hex_mask() {
        new_ucmd!()
            .args(&["0xgg", "/usr/bin/true"])
            .fails()
            .code_is(1)
            .stderr_contains("invalid hex mask");
    }

    #[test]
    fn test_invalid_cpu_list() {
        new_ucmd!()
            .args(&["-c", "abc", "/usr/bin/true"])
            .fails()
            .code_is(1)
            .stderr_contains("invalid CPU list");
    }

    #[test]
    fn test_command_not_found() {
        new_ucmd!()
            .args(&["0x1", "/usr/bin/this-does-not-exist"])
            .fails()
            .code_is(1)
            .stderr_contains("failed to execute");
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use uutests::new_ucmd;

    #[test]
    fn test_unsupported_platform() {
        new_ucmd!()
            .args(&["0x1", "/usr/bin/true"])
            .fails()
            .code_is(1)
            .stderr_contains("available only on Linux");
    }
}
