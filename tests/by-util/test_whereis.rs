// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[test]
fn test_whereis_ls() {
    new_ucmd!().arg("ls").succeeds().stdout_contains("ls:");
}

#[test]
fn test_whereis_nonexistent() {
    new_ucmd!()
        .arg("nonexistent_program_xyz")
        .succeeds()
        .stdout_is("nonexistent_program_xyz:\n");
}

#[test]
fn test_whereis_binary_only() {
    new_ucmd!()
        .args(&["-b", "ls"])
        .succeeds()
        .stdout_contains("/usr/bin/ls");
}
