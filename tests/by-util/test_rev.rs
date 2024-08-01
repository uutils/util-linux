// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::common::util::TestScenario;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_piped_in_data() {
    new_ucmd!().pipe_in("a test").succeeds().stdout_is("tset a");
}

#[test]
fn test_existing_file() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "line A\nline B");

    ucmd.arg("a.txt").succeeds().stdout_is("A enil\nB enil");
}

#[test]
fn test_zero() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "line A\0line B");

    ucmd.arg("a.txt")
        .arg("--zero")
        .succeeds()
        .stdout_is("A enil\0B enil");
}

#[test]
fn test_multiple_files() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "file A\n");
    at.write("b.txt", "file B\n");

    ucmd.args(&["a.txt", "b.txt"])
        .succeeds()
        .stdout_is("A elif\nB elif\n");
}

#[test]
fn test_empty_file() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.touch("empty.txt");

    ucmd.arg("empty.txt").succeeds().no_output();
}

#[test]
fn test_non_existing_file() {
    new_ucmd!()
        .arg("non_existing_file")
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_contains("cannot open non_existing_file: No such file or directory");
}

#[test]
fn test_non_existing_and_existing_file() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "file A");

    ucmd.arg("non_existing_file")
        .arg("a.txt")
        .fails()
        .code_is(1)
        .stderr_contains("cannot open non_existing_file: No such file or directory")
        .stdout_is("A elif");
}
