// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::io::Write;

use tempfile::NamedTempFile;

use uutests::new_ucmd;

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
fn test_seed_files_and_max_size_raw() {
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

    let stdout_no_limit = res_no_limit.no_stderr().stdout_str().trim_end();
    assert_eq!(stdout_no_limit.len(), 32);
    assert!(stdout_no_limit.chars().all(|c| c.is_ascii_hexdigit()));

    let res_verbose = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg("/dev/zero")
        .succeeds();

    res_verbose.stderr_contains("Got 4096 bytes from /dev/zero");
    res_verbose.stderr_contains("Got 128 bytes from randomness source"); // Ensure internal randomness is still added

    let stdout_verbose = res_verbose.stdout_str().trim_end();
    assert_eq!(stdout_verbose.len(), 32);
    assert!(stdout_verbose.chars().all(|c| c.is_ascii_hexdigit()));

    assert_ne!(stdout_no_limit, stdout_verbose);
}

#[test]
fn test_seed_files_and_max_size_human_readable() {
    let mut file = NamedTempFile::new().unwrap();
    const CONTENT: [u8; 4096] = [1; 4096];
    file.write_all(&CONTENT).unwrap();

    let res = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg(file.path())
        .arg("-m")
        .arg("2KiB")
        .succeeds();

    // Ensure we only read up to 2KiB (2048 bytes)
    res.stderr_contains(format!(
        "Got 2048 bytes from {}",
        file.path().to_str().unwrap()
    ));
}

#[test]
fn test_invalid_size_format() {
    let file = NamedTempFile::new().unwrap();

    let res = new_ucmd!()
        .arg("-f")
        .arg(file.path())
        .arg("-m")
        .arg("invalid")
        .fails();

    res.stderr_contains("Failed to parse max-size value");
}

#[test]
fn test_stdin_input() {
    const INPUT_DATA: &str = "some test data for stdin";
    let res = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg("-")
        .pipe_in(INPUT_DATA)
        .succeeds();

    res.stderr_contains(format!("Got {} bytes from stdin", INPUT_DATA.len()));

    let stdout = res.stdout_str().trim_end();
    assert_eq!(stdout.len(), 32);
    assert!(stdout.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_not_existing_file() {
    let mut file1 = NamedTempFile::new().unwrap();
    const CONTENT1: &str = "Some seed data";
    file1.write_all(CONTENT1.as_bytes()).unwrap();

    let file_not_existing = file1.path().to_str().unwrap().to_owned() + "_extra";

    let mut file2 = NamedTempFile::new().unwrap();
    const CONTENT2: [u8; 2048] = [1; 2048];
    file2.write_all(&CONTENT2).unwrap();

    let res = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg(file1.path())
        .arg("-f")
        .arg(&file_not_existing)
        .arg("-f")
        .arg(file2.path())
        .succeeds();

    res.stderr_contains(format!(
        "Got {} bytes from {}",
        CONTENT1.len(),
        file1.path().to_str().unwrap()
    ));

    res.stderr_contains(format!("mcookie: cannot open {file_not_existing}"));

    // Ensure we only read up to the limit of bytes, despite the file being bigger
    res.stderr_contains(format!(
        "Got 2048 bytes from {}",
        file2.path().to_str().unwrap()
    ));
}

#[test]
fn test_max_size_limits() {
    let mut file = NamedTempFile::new().unwrap();
    const CONTENT: [u8; 5500] = [1; 5500];
    file.write_all(&CONTENT).unwrap();

    let res_default = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg(file.path())
        .succeeds();

    // Ensure we only read up to 4096 bytes
    res_default.stderr_contains(format!(
        "Got 4096 bytes from {}",
        file.path().to_str().unwrap()
    ));

    let res_zero = new_ucmd!()
        .arg("--verbose")
        .arg("-f")
        .arg(file.path())
        .arg("-m")
        .arg("0")
        .succeeds();

    // Ensure we read up 4096 bytes
    res_zero.stderr_contains(format!(
        "Got 4096 bytes from {}",
        file.path().to_str().unwrap()
    ));
}
