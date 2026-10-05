// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
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

#[test]
#[cfg(target_family = "unix")]
fn test_non_utf8_arguments() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    new_ucmd!()
        .arg("echo")
        .arg(OsStr::from_bytes(b"\xff"))
        .succeeds()
        .stdout_is_bytes(b"\xff\n");
}

#[test]
#[cfg(all(unix, not(target_os = "macos")))]
fn test_non_utf8_program_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use uutests::at_and_ucmd;

    let (at, mut ucmd) = at_and_ucmd!();
    let program = at.plus(OsStr::from_bytes(b"sh_\xff"));
    std::os::unix::fs::symlink("/bin/sh", &program).unwrap();
    ucmd.arg(&program)
        .args(&["-c", "echo ran"])
        .succeeds()
        .stdout_is("ran\n");
}
