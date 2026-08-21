// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::{at_and_ucmd, new_ucmd};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[cfg(target_os = "linux")]
#[test]
fn test_non_mountpoint_uses_path_parent() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("not-a-mountpoint");

    let path = at.plus_as_string("not-a-mountpoint");

    ucmd.current_dir("/proc/self")
        .arg(&path)
        .fails()
        .code_is(32)
        .stdout_is(format!("{path} is not a mountpoint\n"))
        .no_stderr();
}
