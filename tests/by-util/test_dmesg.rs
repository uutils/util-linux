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
fn test_kmsg_json() {
    new_ucmd!()
        .arg("--kmsg-file")
        .arg("kmsg.input")
        .arg("--json")
        .run()
        .no_stderr()
        .stdout_is_templated_fixture("test_kmsg_json.expected", &[("\r\n", "\n")]);
}
