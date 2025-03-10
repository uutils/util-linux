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
fn test_invalid_cpu_range() {
    new_ucmd!()
        .arg("-e")
        .arg("non-numeric-range")
        .fails()
        .code_is(1);
}

#[test]
fn test_invalid_cpu_index() {
    new_ucmd!()
        .arg("-e")
        .arg("10000") // Assuming no test environment will ever have 10000 CPUs
        .fails()
        .code_is(1)
        .stderr_contains("CPU 10000 does not exist");
}

#[test]
fn test_invalid_dispatch_mode() {
    new_ucmd!()
        .arg("-p")
        .arg("not-horizontal-or-vertical")
        .fails()
        .code_is(1)
        .stderr_contains("Unsupported dispatching mode");
}

// TODO: Find a way to implement "happy-case" tests that doesn't rely on the host `/sys/` filesystem
