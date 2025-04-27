// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::common::util::TestScenario;

#[test]
#[cfg(target_os = "linux")]
fn test_basic_lookup() {
    new_ucmd!()
        .arg("gcc")
        .succeeds()
        .stdout_contains("/usr/bin/gcc");
}

#[test]
#[cfg(target_os = "linux")]
fn test_bin_only() {
    new_ucmd!()
        .arg("-b")
        .arg("ping")
        .succeeds()
        .stdout_contains("/usr/bin/ping");
}

#[test]
#[cfg(target_os = "linux")]
fn test_man_only() {
    new_ucmd!()
        .arg("-m")
        .arg("ls")
        .succeeds()
        .stdout_contains("/usr/share/man");
}

#[test]
#[cfg(target_os = "linux")]
fn test_src_only() {
    new_ucmd!()
        .arg("-s")
        .arg("dig")
        .succeeds()
        .stdout_is("dig:\n");
}

#[test]
#[cfg(target_os = "linux")]
fn test_output() {
    let res = new_ucmd!().arg("ping").arg("gcc").succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Non-exhaustive list of fields we expect
    // Check that 'ping' and 'gcc' have their paths listed
    assert!(stdout.contains("ping:"));
    assert!(stdout.contains("gcc:"));

    // Check that paths are printed next to the command name, as expected
    assert!(stdout.contains("/usr/bin/ping"));
    assert!(stdout.contains("/usr/bin/gcc"));

    assert!(stdout.contains("/usr/lib/gcc"));
    assert!(stdout.contains("/usr/share/gcc"));
}
