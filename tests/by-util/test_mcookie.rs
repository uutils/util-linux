// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::io::Write;

use tempfile::NamedTempFile;

use crate::common::util::TestScenario;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_basic_usage() {
    let res = new_ucmd!().succeeds();

    let stdout = res.no_stderr().stdout_str();

    // Expect 32 hex characters for the MD5 hash (after trimming the newline)
    assert_eq!(stdout.trim_end().len(), 32);
    assert!(stdout.trim_end().chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_verbose() {
    let res = new_ucmd!().arg("--verbose").succeeds();
    res.stderr_contains("Got 128 bytes from randomness source");
}

#[test]
fn test_seed_files_and_max_size() {
    let mut file1 = NamedTempFile::new().unwrap();
    const CONTENT1: &str = "Some seed data";
    file1.write_all(CONTENT1.as_bytes()).unwrap();

    let mut file2 = NamedTempFile::new().unwrap();
    const CONTENT2: [u8; 2048] = [1; 2048];
    file2.write_all(&CONTENT2).unwrap();

    let res = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg(file1.path())
        .arg("-f")
        .arg(file2.path())
        .arg("-m")
        .arg("1337")
        .succeeds();

    res.stderr_contains(format!(
        "Got {} bytes from {}",
        CONTENT1.len(),
        file1.path().to_str().unwrap()
    ));

    // Ensure we only read up to the limit of bytes, despite the file being bigger
    res.stderr_contains(format!(
        "Got 1337 bytes from {}",
        file2.path().to_str().unwrap()
    ));
}

#[test]
#[cfg(unix)] // Character devices like /dev/zero are a Unix concept
fn test_char_device_input() {
    let res_no_limit = new_ucmd!().arg("-f").arg("/dev/zero").succeeds();

    let stdout_no_limit = res_no_limit.no_stderr().stdout_str();
    assert_eq!(stdout_no_limit.trim_end().len(), 32);
    assert!(stdout_no_limit
        .trim_end()
        .chars()
        .all(|c| c.is_ascii_hexdigit()));

    let res_verbose = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg("/dev/zero")
        .succeeds();

    res_verbose.stderr_contains("Got 1024 bytes from /dev/zero");
    res_verbose.stderr_contains("Got 128 bytes from randomness source"); // Ensure internal randomness is still added

    let stdout_verbose = res_verbose.stdout_str();
    assert_eq!(stdout_verbose.trim_end().len(), 32);
    assert!(stdout_verbose
        .trim_end()
        .chars()
        .all(|c| c.is_ascii_hexdigit()));

    assert_ne!(stdout_no_limit, stdout_verbose);
}
