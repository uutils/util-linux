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

#[cfg(target_os = "linux")]
#[test]
fn test_invalid_file() {
    new_ucmd!().arg("not_existing_file.not_existing_extension"); // Should print non-file name as broadcast
}

#[cfg(target_os = "macos")]
#[test]
fn test_invalid_file() {
    new_ucmd!()
        .arg("not_existing_file.not_existing_extension")
        .fails()
        .code_is(1); // On macOS, file not existing is an error
}

// wall does not print the content of the file in the stdout, it sends it to the tty(s).
