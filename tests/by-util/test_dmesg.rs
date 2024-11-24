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
        .run()
        .no_stderr()
        .stdout_is_templated_fixture("test_kmsg_json.expected", &[("\r\n", "\n")]);
}

#[test]
fn test_kmsg_time_format() {
    let time_formats = ["delta", "reltime", "ctime", "notime", "iso", "raw"];
    for format in time_formats {
        let time_format_arg = format!("--time-format={format}");
        let expected_output = format!("test_kmsg_time_format_{format}.expected");
        new_ucmd!()
            .arg("--kmsg-file")
            .arg("kmsg.input.1")
            .arg(time_format_arg)
            .succeeds()
            .no_stderr()
            .stdout_is_fixture(expected_output);
    }
}

#[test]
fn test_invalid_time_format() {
    new_ucmd!()
        .arg("--time-format=definitely-invalid")
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_is("dmesg: unknown time format: definitely-invalid");
}
