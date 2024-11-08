// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) symdir somefakedir

#[cfg(unix)]
use crate::common::util::TestScenario;

#[cfg(unix)]
use regex::Regex;
use std::fs;
use std::io::Write;

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
    let line_check = |input: &str| input.lines().count() == 3;
    new_ucmd!()
        .arg("--limit=1")
        .succeeds()
        .stdout_str_check(line_check);
}

#[test]
// The -x flag generally adds two rows "shutdown" and "runlevel"
// "shutdown" cannot be checked for since not every machine will have shutdown
// "runlevel" only makes sense for Linux systems, so only Linux is included for
// this test.
#[cfg(target_os = "linux")]
#[ignore = "fails on Arch Linux"]
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

#[test]
#[cfg(unix)]
fn test_short_invalid_utmp_file() {
    let filepath = "/tmp/testfile";
    let testfile = fs::File::create(filepath);
    // Random bytes
    let data: Vec<u8> = vec![
        4, 5, 6, 16, 8, 13, 2, 12, 5, 3, 11, 5, 1, 13, 1, 1, 0, 9, 5, 5, 2, 8, 4,
    ];
    let _ = testfile.unwrap().write_all(&data);

    let regex = Regex::new(r"\n\S*\sbegins\s*(Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s*(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s*[0-9][0-9]?\s*[0-9][0-9]:[0-9][0-9]:[0-9][0-9]\s*[0-9]*")
            .unwrap();

    new_ucmd!()
        .arg(format!("--file={filepath}"))
        .succeeds()
        .stdout_matches(&regex);
}
