// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
use crate::common::util::TestScenario;

#[test]
#[cfg(target_os = "linux")]
fn test_column_headers() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    let header_line = stdout.lines().next().unwrap();
    let cols: Vec<_> = header_line.split_whitespace().collect();

    assert_eq!(cols.len(), 7);
    assert_eq!(
        cols,
        vec!["COMMAND", "PID", "TYPE", "MODE", "M", "START", "END"]
    );
}
