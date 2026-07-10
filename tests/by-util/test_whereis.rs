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
    let output = new_ucmd!().args(&["-b", "ls"]).succeeds().stdout_move_str();
    assert!(output.contains("ls:"), "output should contain 'ls:'");
    assert!(
        output.lines().next().unwrap().contains("ls:"),
        "output should have 'ls:' prefix"
    );
}

#[test]
fn test_whereis_man_only() {
    let output = new_ucmd!().args(&["-m", "ls"]).succeeds().stdout_move_str();
    assert!(output.contains("ls:"), "output should contain 'ls:'");
    assert!(
        output.contains("man"),
        "output should contain man page path"
    );
}

#[test]
fn test_whereis_multiple_programs() {
    let output = new_ucmd!()
        .args(&["ls", "cat"])
        .succeeds()
        .stdout_move_str();
    assert!(output.contains("ls:"), "output should contain 'ls:'");
    assert!(output.contains("cat:"), "output should contain 'cat:'");
}

#[test]
fn test_whereis_default_searches_all() {
    let output = new_ucmd!().arg("ls").succeeds().stdout_move_str();
    let first_line = output.lines().next().unwrap();
    assert!(first_line.starts_with("ls:"), "output should start with 'ls:'");
    assert!(
        first_line.contains("/"),
        "output should contain at least one path"
    );
}
