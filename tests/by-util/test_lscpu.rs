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
#[ignore = "not yet implemented"]
fn test_hex() {
    new_ucmd!().arg("--hex").succeeds().stdout_contains("0x");
}

#[test]
fn test_json() {
    new_ucmd!()
        .arg("--json")
        .succeeds()
        // ensure some fields are there, non-exhausting
        .stdout_contains("\"lscpu\": [")
        .stdout_contains("\"field\": \"Architecture\"")
        .stdout_contains("\"field\": \"CPU(s)\"");
}
