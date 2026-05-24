// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use uutests::util::{TestScenario, UCommand};

fn new_pivot_root_cmd() -> UCommand {
    TestScenario::new("pivot_root").ucmd()
}

#[test]
fn test_invalid_arg() {
    new_pivot_root_cmd()
        .arg("--definitely-invalid")
        .fails()
        .code_is(1);
}

#[test]
fn test_help() {
    new_pivot_root_cmd()
        .arg("--help")
        .succeeds()
        .stdout_contains("pivot_root")
        .stdout_contains("NEW_ROOT")
        .stdout_contains("PUT_OLD");
}

#[test]
fn test_version() {
    new_pivot_root_cmd().arg("--version").succeeds();
}

#[test]
fn test_missing_arguments() {
    new_pivot_root_cmd()
        .fails()
        .code_is(1)
        .stderr_contains("required arguments");
}

#[test]
fn test_missing_put_old_argument() {
    new_pivot_root_cmd()
        .arg("/new_root")
        .fails()
        .code_is(1)
        .stderr_contains("required");
}

#[test]
fn test_too_many_arguments() {
    new_pivot_root_cmd()
        .arg("/new_root")
        .arg("/put_old")
        .arg("/extra")
        .fails()
        .code_is(1);
}

// Tests that require elevated privileges (CAP_SYS_ADMIN)
// These tests are ignored by default and should be run manually with:
// cargo test pivot_root -- --ignored
//
// Prerequisites:
// - Must be run as root or with CAP_SYS_ADMIN capability
// - Requires proper filesystem setup with new root and put_old directories

#[test]
#[ignore]
fn test_pivot_root_non_existing_paths() {
    // This test requires root but tests error handling for non-existing paths
    new_pivot_root_cmd()
        .arg("/non_existing_new_root")
        .arg("/non_existing_put_old")
        .fails()
        .code_is(1)
        .stderr_contains("failed to change root");
}

#[test]
#[ignore]
fn test_pivot_root_without_privileges() {
    // This test should be run without root privileges to verify proper error handling
    // Note: This may not fail with the expected error if run as root
    new_pivot_root_cmd()
        .arg("/tmp")
        .arg("/tmp")
        .fails()
        .code_is(1);
}

// Tests for non-UTF8 path acceptance
// These tests verify that pivot_root properly handles paths containing non-UTF8 bytes,
// which are valid on Unix filesystems (any byte sequence except null is allowed).

#[test]
#[cfg(unix)]
#[ignore]
fn test_non_utf8_new_root_path() {
    // Test that pivot_root accepts a non-UTF8 path for new_root
    // The path contains bytes 0x80-0xFF which are invalid UTF-8
    let non_utf8_bytes: &[u8] = b"/tmp/test_\x80\x81\x82_new_root";
    let new_root = OsStr::from_bytes(non_utf8_bytes);
    let put_old = OsStr::new("/tmp/put_old");

    new_pivot_root_cmd()
        .arg(new_root)
        .arg(put_old)
        .fails()
        .code_is(1)
        // The command should fail due to path not existing, not due to encoding issues
        .stderr_contains("failed to change root");
}

#[test]
#[cfg(unix)]
#[ignore]
fn test_non_utf8_put_old_path() {
    // Test that pivot_root accepts a non-UTF8 path for put_old
    let new_root = OsStr::new("/tmp/new_root");
    let non_utf8_bytes: &[u8] = b"/tmp/test_\xff\xfe\xfd_put_old";
    let put_old = OsStr::from_bytes(non_utf8_bytes);

    new_pivot_root_cmd()
        .arg(new_root)
        .arg(put_old)
        .fails()
        .code_is(1)
        // The command should fail due to path not existing, not due to encoding issues
        .stderr_contains("failed to change root");
}

#[test]
#[cfg(unix)]
#[ignore]
fn test_non_utf8_both_paths() {
    // Test that pivot_root accepts non-UTF8 paths for both arguments
    let new_root_bytes: &[u8] = b"/tmp/\xc0\xc1_new";
    let put_old_bytes: &[u8] = b"/tmp/\xe0\xe1_old";
    let new_root = OsStr::from_bytes(new_root_bytes);
    let put_old = OsStr::from_bytes(put_old_bytes);

    new_pivot_root_cmd()
        .arg(new_root)
        .arg(put_old)
        .fails()
        .code_is(1)
        // The command should fail due to path not existing, not due to encoding issues
        .stderr_contains("failed to change root");
}
