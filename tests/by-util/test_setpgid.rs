// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use regex::Regex;
use uutests::new_ucmd;

#[test]
#[cfg(target_family = "unix")]
fn test_nonexistent_program() {
    new_ucmd!()
        .arg("does_not_exist")
        .fails()
        .stderr_contains("failed to execute");
}

#[test]
#[cfg(target_os = "linux")]
fn test_pgid_changed() {
    let our_pgid = unsafe { libc::getpgid(0) };
    // Gets pgid of the 'cut' process from /proc
    new_ucmd!()
        .args(&["cut", "-d", " ", "-f", "5", "/proc/self/stat"])
        .succeeds()
        .stdout_does_not_match(&Regex::new(&format!("^{}$", our_pgid)).unwrap());
}

#[test]
#[cfg(target_family = "unix")]
fn test_flag_after_command() {
    new_ucmd!()
        .arg("echo")
        .arg("-f")
        .succeeds()
        .stdout_is("-f\n");
}
