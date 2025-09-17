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
#[ignore = "not yet implemented"]
fn test_hex() {
    new_ucmd!().arg("--hex").succeeds().stdout_contains("0x");
}

#[test]
#[cfg(target_os = "linux")]
fn test_json() {
    let res = new_ucmd!().arg("--json").succeeds();

    let stdout = res.no_stderr().stdout_str();
    assert!(stdout.starts_with("{"));
    assert!(stdout.ends_with("}\n"));

    res.stdout_contains("\"lscpu\": [")
        .stdout_contains("\"field\": \"Architecture\"")
        .stdout_contains("\"field\": \"CPU(s)\"")
        .stdout_contains("\"children\": [");
}

#[test]
#[cfg(target_os = "linux")]
fn test_output() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Non-exhaustive list of fields we expect
    // This also checks that fields which should be indented, are indeed indented as excepted
    assert!(stdout.contains("Architecture:"));
    assert!(stdout.contains("\n  Address sizes:"));
    assert!(stdout.contains("\n  Byte Order:"));
    assert!(stdout.contains("\nCPU(s):"));
    assert!(stdout.contains("\nVendor ID:"));
    assert!(stdout.contains("\n  Model name:"));
    assert!(stdout.contains("\n    CPU Family:"));
}
