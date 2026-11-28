// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[cfg(target_family = "unix")]
#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[cfg(not(target_family = "unix"))]
#[test]
fn unsupported_feature() {
    new_ucmd!().arg("Cargo.toml").fails().code_is(1);
}

// wall does not print the content of the file in the stdout, it sends it to the tty(s).

#[cfg(target_family = "unix")]
#[test]
fn test_help_arg() {
    new_ucmd!().arg("--help").succeeds().code_is(0);
}

#[cfg(target_family = "unix")]
#[test]
fn test_null_timeout() {
    new_ucmd!().arg("-t").arg("0").fails().code_is(1);
}

#[cfg(target_family = "unix")]
#[test]
fn test_invalid_timeout() {
    new_ucmd!().arg("-t").arg("notanumber").fails().code_is(1);
}

#[cfg(target_family = "unix")]
#[test]
fn test_no_timeout() {
    new_ucmd!().arg("-t").arg("--help").fails().code_is(1);
}

#[cfg(target_family = "unix")]
#[test]
fn test_non_utf8_arg() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let non_utf = vec![2, 213, 255, 13, 12];
    new_ucmd!()
        .arg(OsString::from_vec(non_utf))
        .fails()
        .code_is(1);
}
