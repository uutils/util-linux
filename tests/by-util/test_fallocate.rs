// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::{at_and_ucmd, new_ucmd};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_missing_length() {
    new_ucmd!().arg("testfile").fails().code_is(1);
}

#[test]
fn test_basic_allocate() {
    let (at, mut ucmd) = at_and_ucmd!();

    ucmd.args(&["-l", "1M", "testfile"]).succeeds().no_output();

    let metadata = at.metadata("testfile");
    assert_eq!(metadata.len(), 1048576);
}

#[test]
fn test_allocate_with_offset() {
    let (at, mut ucmd) = at_and_ucmd!();

    ucmd.args(&["-l", "4096", "-o", "4096", "testfile"])
        .succeeds()
        .no_output();

    let metadata = at.metadata("testfile");
    assert_eq!(metadata.len(), 8192);
}

#[test]
fn test_keep_size() {
    let (at, mut ucmd) = at_and_ucmd!();

    ucmd.args(&["-n", "-l", "1M", "testfile"])
        .succeeds()
        .no_output();

    let metadata = at.metadata("testfile");
    // With keep-size, the apparent file size should be 0 since the file was new
    assert_eq!(metadata.len(), 0);
}

#[test]
fn test_verbose() {
    let (at, mut ucmd) = at_and_ucmd!();

    ucmd.args(&["-v", "-l", "1M", "testfile"])
        .succeeds()
        .stderr_contains("1 MiB (1048576 bytes) allocated.");

    let metadata = at.metadata("testfile");
    assert_eq!(metadata.len(), 1048576);
}

#[test]
fn test_posix_mode() {
    let (at, mut ucmd) = at_and_ucmd!();

    ucmd.args(&["-x", "-l", "1M", "testfile"])
        .succeeds()
        .no_output();

    let metadata = at.metadata("testfile");
    assert_eq!(metadata.len(), 1048576);
}

#[test]
fn test_mutually_exclusive_modes() {
    new_ucmd!()
        .args(&["-c", "-p", "-l", "4096", "testfile"])
        .fails()
        .code_is(1);
}

#[test]
fn test_punch_hole() {
    let (at, mut ucmd) = at_and_ucmd!();

    // Create a 1M file using standard fs
    let path = at.plus("testfile");
    std::fs::write(&path, &vec![0xFFu8; 1048576]).unwrap();

    ucmd.args(&["-p", "-o", "4096", "-l", "4096", "testfile"])
        .succeeds()
        .no_output();

    // Size should remain the same (punch-hole implies keep-size)
    let metadata = at.metadata("testfile");
    assert_eq!(metadata.len(), 1048576);
}

#[test]
fn test_size_suffixes() {
    let (at, mut ucmd) = at_and_ucmd!();

    ucmd.args(&["-l", "1KiB", "testfile"])
        .succeeds()
        .no_output();

    let metadata = at.metadata("testfile");
    assert_eq!(metadata.len(), 1024);
}

#[test]
fn test_nonexistent_file_for_punch() {
    new_ucmd!()
        .args(&["-p", "-l", "4096", "nonexistent"])
        .fails()
        .code_is(1)
        .stderr_contains("cannot open");
}
