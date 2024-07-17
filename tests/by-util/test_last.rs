// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) symdir somefakedir

use crate::common::util::TestScenario;

use regex::Regex;

#[test]
#[cfg(unix)]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
#[cfg(unix)]
fn test_last() {
    let regex = Regex::new("still running|still logged in").unwrap();
    TestScenario::new(util_name!())
        .ucmd()
        .succeeds()
        .stdout_matches(&regex);
}

#[test]
#[cfg(unix)]
fn test_limit_arg() {
    let line_check = |input: &str| input.lines().count() == 1;
    new_ucmd!()
        .arg("--limit=1")
        .succeeds()
        .stdout_str_check(line_check);
}

#[test]
#[cfg(unix)]
fn test_system_arg() {
    new_ucmd!().arg("-x").succeeds().stdout_contains("runlevel");
}

#[test]
#[cfg(unix)]
fn test_timestamp_format_no_time() {
    let regex = Regex::new(" [0-9][0-9]:[0-9][0-9] ").unwrap();
    new_ucmd!()
        .arg("--time-format=notime")
        .succeeds()
        .stdout_does_not_match(&regex);
}

#[test]
#[cfg(unix)]
fn test_timestamp_format_short() {
    let regex = Regex::new(" [0-9][0-9]:[0-9][0-9] ").unwrap();
    new_ucmd!()
        .arg("--time-format=short")
        .succeeds()
        .stdout_matches(&regex);
}

#[test]
#[cfg(unix)]
fn test_timestamp_format_full() {
    let regex = Regex::new(" [0-9][0-9]:[0-9][0-9]:[0-9][0-9] ").unwrap();
    new_ucmd!()
        .arg("--time-format=full")
        .succeeds()
        .stdout_matches(&regex);
}

// 2024-07-11T19:30:44+08:00
#[test]
#[cfg(unix)]
fn test_timestamp_format_iso() {
    let regex =
        Regex::new(" [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]")
            .unwrap();
    new_ucmd!()
        .arg("--time-format=iso")
        .succeeds()
        .stdout_matches(&regex);
}
