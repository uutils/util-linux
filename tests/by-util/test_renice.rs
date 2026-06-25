// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) symdir somefakedir

use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_pid_option_after_priority() {
    new_ucmd!()
        .args(&["19", "-p", "not-a-pid"])
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_is("Invalid process ID\n");
}

#[test]
fn test_priority_option_before_pid() {
    new_ucmd!()
        .args(&["-n", "19", "-p", "not-a-pid"])
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_is("Invalid process ID\n");
}

#[test]
fn test_pgrp_option_after_priority() {
    new_ucmd!()
        .args(&["19", "-g", "not-a-pgrp"])
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_is("Invalid process group ID\n");
}
