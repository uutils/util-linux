// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) symdir somefakedir

#[cfg(target_os = "linux")]
use crate::common::util::TestScenario;

#[test]
#[cfg(target_os = "linux")]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}
