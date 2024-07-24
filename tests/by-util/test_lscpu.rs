// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::common::util::TestScenario;
use serde_json::{self, Value};

#[test]
#[ignore]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_json() {
    let result = new_ucmd!().arg("--json").succeeds();
    let stdout_bytes = result.stdout();
    let res: Result<Value, _> = serde_json::from_slice(&stdout_bytes);
    assert!(res.is_ok(), "invalid json output");
}
