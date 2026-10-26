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
#[cfg(unix)]
fn test_root_is_a_mountpoint() {
    new_ucmd!()
        .arg("/")
        .succeeds()
        .stdout_is("/ is a mountpoint\n");
}

#[test]
#[cfg(unix)]
fn test_directory_is_not_a_mountpoint() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("dir");
    ucmd.arg(at.plus_as_string("dir"))
        .current_dir("/")
        .fails()
        .code_is(32)
        .stdout_is(format!(
            "{} is not a mountpoint\n",
            at.plus_as_string("dir")
        ));
}

#[test]
#[cfg(unix)]
fn test_regular_file_is_not_a_mountpoint() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.touch("file");
    ucmd.arg(at.plus_as_string("file"))
        .fails()
        .code_is(32)
        .stdout_is(format!(
            "{} is not a mountpoint\n",
            at.plus_as_string("file")
        ));
}

#[test]
#[cfg(unix)]
fn test_nonexistent_path() {
    new_ucmd!()
        .arg("does-not-exist")
        .fails()
        .code_is(1)
        .stderr_contains("does-not-exist: No such file or directory");
}

#[test]
#[cfg(target_os = "linux")]
fn test_proc_is_a_mountpoint() {
    new_ucmd!()
        .arg("/proc/")
        .succeeds()
        .stdout_is("/proc/ is a mountpoint\n");
}
