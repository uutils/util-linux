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
#[cfg(target_os = "linux")]
fn test_basic_output() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Check for header columns
    assert!(stdout.contains("NS"));
    assert!(stdout.contains("TYPE"));
    assert!(stdout.contains("NPROCS"));
    assert!(stdout.contains("PID"));
    assert!(stdout.contains("USER"));
    assert!(stdout.contains("COMMAND"));
}

#[test]
#[cfg(target_os = "linux")]
fn test_namespace_types() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // We should see at least some common namespace types
    // Note: Not all may be present on all systems, so we check for at least one
    let has_namespace = stdout.contains("mnt")
        || stdout.contains("net")
        || stdout.contains("pid")
        || stdout.contains("uts")
        || stdout.contains("ipc")
        || stdout.contains("user")
        || stdout.contains("cgroup");

    assert!(
        has_namespace,
        "Expected to see at least one namespace type in output"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn test_output_has_processes() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // The output should have at least one process (the test process itself)
    // Count lines (excluding header)
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 2,
        "Expected at least header line and one namespace entry"
    );
}

#[test]
#[cfg(not(target_os = "linux"))]
fn test_unsupported_platform() {
    // On non-Linux platforms, lsns should fail with an appropriate error
    new_ucmd!().fails();
}

#[test]
#[cfg(target_os = "linux")]
fn test_output_format() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Verify the output has proper table format
    /*
        Each line should have multiple columns separated by whitespace.We check
        for at least 4 columns as the minimum (NS, TYPE, NPROCS, PID). These
        fields are always present. For mnt namespaces the PID and COMMAND column
        will be empty.
    */
    for line in stdout.lines().skip(1) {
        // Skip header
        if !line.is_empty() {
            let columns: Vec<&str> = line.split_whitespace().collect();
            assert!(
                columns.len() >= 4,
                "Each namespace entry should have at least 4 columns"
            );
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_noheadings() {
    let res = new_ucmd!().arg("-n").succeeds();
    let stdout = res.no_stderr().stdout_str();

    let headers = ["NS", "TYPE", "NPROCS", "PID", "USER", "COMMAND"];

    for header in headers {
        let msg = format!("{} header should not be present when -n is used", header);
        assert!(!stdout.contains(header), "{}", msg);
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_namespace_ids_are_numeric() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    /*
        The first column of each line should be the namespace ID which is an inod number
        and it should be numeric.
    */

    // Skip the header line and check that namespace IDs are numeric
    for line in stdout.lines().skip(1) {
        if !line.is_empty() {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if !columns.is_empty() {
                let ns_id = columns[0];
                assert!(
                    ns_id.chars().all(|c| c.is_ascii_digit()),
                    "Namespace ID should be numeric: {}",
                    ns_id
                );
            }
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_user_column_not_empty() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Check that USER column (5th column) is not empty for entries with processes
    for line in stdout.lines().skip(1) {
        if !line.is_empty() {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() >= 5 {
                // If there's a PID (4th column is not empty), there should be a user
                if !columns[3].is_empty() && columns[3] != "0" {
                    assert!(
                        !columns[4].is_empty(),
                        "User column should not be empty when PID is present"
                    );
                }
            }
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_persistent_flag() {
    let res = new_ucmd!().arg("--persistent").succeeds();
    let stdout = res.no_stderr().stdout_str();

    // With --persistent flag, only namespaces without processes should be shown
    // These are persistent (bind-mounted) namespaces
    for line in stdout.lines().skip(1) {
        if !line.is_empty() {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() >= 3 {
                // NPROCS column (3rd column) should be 0 for persistent namespaces
                let nprocs = columns[2];
                assert_eq!(
                    nprocs, "0",
                    "With --persistent flag, NPROCS should be 0 (no processes)"
                );
            }
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_type_option() {
    let res = new_ucmd!().arg("--type").arg("mnt").succeeds();
    let stdout = res.no_stderr().stdout_str();

    // With --type mnt, only mount namespace should be shown
    for line in stdout.lines().skip(1) {
        if !line.is_empty() {
            let columns: Vec<&str> = line.split_whitespace().collect();
            if columns.len() >= 2 {
                assert_eq!(
                    columns[1], "mnt",
                    "With --type mnt, only mnt namespace should be shown"
                );
            }
        }
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_invalid_namespace_type() {
    let res = new_ucmd!()
        .arg("--type")
        .arg("invalid-namespace-type")
        .fails();
    let stdout = res.stderr_str();

    assert!(stdout.contains("lsns: unknown namespace type: invalid-namespace-type"));
}

#[test]
#[cfg(target_os = "linux")]
fn test_invalid_task_pid() {
    let res = new_ucmd!().arg("--task").arg("not_a_number").fails();
    let stderr = res.stderr_str();

    assert!(stderr.contains("invalid PID argument: 'not_a_number'"));
}

// Write a test to verify that -p 0 part.
#[test]
#[cfg(target_os = "linux")]
fn test_task_pid_0() {
    let res = new_ucmd!().arg("--task").arg("0").succeeds();
    let stdout = res.no_stderr().stdout_str();

    // With -p 0, all namespaces should be shown
    let line = stdout.lines().next().unwrap();

    // Check for all regular headers
    let columns = line.split_whitespace().count();
    assert!(
        columns == 6,
        "All 6 columns should be displayed when -p 0 is used"
    );

    // Check for at least one entry
    let entry_count = stdout.lines().count();
    assert!(
        entry_count > 1,
        "There should be at least one entry when -p 0 is used"
    );
}
