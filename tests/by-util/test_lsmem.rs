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
fn test_table_not_padded() {
    let result = new_ucmd!().succeeds();
    let stdout = result.code_is(0).stdout_str();
    assert!(
        !stdout.starts_with(' '),
        "Table output should not start with a space"
    );
}
