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
fn test_kmsg_nonexistent_file() {
    new_ucmd!()
        .arg("--kmsg-file")
        .arg("definitely-nonexistent-file")
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_is("dmesg: cannot open definitely-nonexistent-file: No such file or directory\n");
}

#[test]
fn test_kmsg_json() {
    new_ucmd!()
        .arg("--kmsg-file")
        .arg("kmsg.input")
        .arg("--json")
        .succeeds()
        .no_stderr()
        .stdout_is_templated_fixture("test_kmsg_json.expected", &[("\r\n", "\n")]);
}

#[test]
fn test_kmsg_time_format_delta() {
    test_kmsg_time_format("delta");
}

#[test]
fn test_kmsg_time_format_reltime() {
    test_kmsg_time_format("reltime");
}

#[test]
fn test_kmsg_time_format_ctime() {
    test_kmsg_time_format("ctime");
}

#[test]
fn test_kmsg_time_format_notime() {
    test_kmsg_time_format("notime");
}

#[test]
fn test_kmsg_time_format_iso() {
    test_kmsg_time_format("iso");
}

#[test]
fn test_kmsg_time_format_raw() {
    test_kmsg_time_format("raw");
}

fn test_kmsg_time_format(format: &str) {
    let time_format_arg = format!("--time-format={format}");
    let expected_output = format!("test_kmsg_time_format_{format}.expected");
    new_ucmd!()
        .arg("--kmsg-file")
        .arg("kmsg.input.1")
        .arg(time_format_arg)
        .succeeds()
        .no_stderr()
        .stdout_is_templated_fixture(expected_output, &[("\r\n", "\n")]);
}

#[test]
fn test_invalid_time_format() {
    new_ucmd!()
        .arg("--time-format=definitely-invalid")
        .fails()
        .code_is(1)
        .stderr_only("dmesg: unknown time format: definitely-invalid\n");
}

#[test]
fn test_filter_facility() {
    let facilities = [
        "kern", "user", "mail", "daemon", "auth", "syslog", "lpr", "news", "uucp", "cron",
        "authpriv", "ftp", "local0", "local1", "local2", "local3", "local4", "local5", "local6",
        "local7",
    ];
    for facility in facilities {
        let facility_filter_arg = format!("--facility={facility}");
        let mut cmd = new_ucmd!();
        let result = cmd
            .arg("--kmsg-file")
            .arg("kmsg.input")
            .arg(facility_filter_arg)
            .succeeds();
        let stdout = result.no_stderr().stdout_str();
        assert_eq!(stdout.lines().count(), 8);
        let expected = format!("LOG_{}", facility.to_uppercase());
        stdout
            .lines()
            .for_each(|line| assert!(line.contains(&expected)));
    }
}

#[test]
fn test_filter_levels() {
    let levels = [
        "emerg", "alert", "crit", "err", "warn", "notice", "info", "debug",
    ];
    for level in levels {
        let level_filter_arg = format!("--level={level}");
        let mut cmd = new_ucmd!();
        let result = cmd
            .arg("--kmsg-file")
            .arg("kmsg.input")
            .arg(level_filter_arg)
            .succeeds();
        let stdout = result.no_stderr().stdout_str();
        assert_eq!(stdout.lines().count(), 20);
        let expected = format!("LOG_{}", level.to_uppercase());
        stdout
            .lines()
            .for_each(|line| assert!(line.contains(&expected)));
    }
}

#[test]
fn test_invalid_facility_argument() {
    new_ucmd!()
        .arg("--facility=definitely-invalid")
        .fails()
        .code_is(1)
        .stderr_only("dmesg: unknown facility 'definitely-invalid'\n");
}

#[test]
fn test_invalid_level_argument() {
    new_ucmd!()
        .arg("--level=definitely-invalid")
        .fails()
        .code_is(1)
        .stderr_only("dmesg: unknown level 'definitely-invalid'\n");
}

#[test]
fn test_filter_multiple() {
    let mut cmd = new_ucmd!();
    let result = cmd
        .arg("--kmsg-file")
        .arg("kmsg.input")
        .arg("--facility=kern,user")
        .arg("--level=emerg,alert")
        .succeeds();
    let stdout = result.no_stderr().stdout_str();
    assert_eq!(stdout.lines().count(), 4);
    stdout.lines().for_each(|line| {
        assert!(
            (line.contains("LOG_KERN") || line.contains("LOG_USER"))
                && (line.contains("LOG_EMERG") || line.contains("LOG_ALERT"))
        )
    });
}

#[test]
fn test_since_until() {
    new_ucmd!()
        .arg("--kmsg-file")
        .arg("kmsg.input")
        .arg("--since=2024-11-19 17:47:32 +0700")
        .arg("--until=2024-11-19 18:55:52 +0700")
        .succeeds()
        .no_stderr()
        .stdout_is_templated_fixture("test_since_until.expected", &[("\r\n", "\n")]);
}

#[test]
fn test_since_until_invalid_time() {
    let options = ["--since", "--until"];
    for option in options {
        new_ucmd!()
            .arg(format!("{option}=definitely-invalid"))
            .fails()
            .stderr_only("dmesg: invalid time value \"definitely-invalid\"\n");
    }
}
