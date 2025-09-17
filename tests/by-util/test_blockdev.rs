// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_report_mutually_exclusive_with_others() {
    new_ucmd!()
        .arg("--report")
        .arg("--getalignoff")
        .arg("/foo")
        .fails()
        .code_is(1)
        .stderr_contains("the argument '--report' cannot be used with '--getalignoff'");
}

#[cfg(target_os = "linux")]
mod linux {
    use regex::Regex;

    use uutests::new_ucmd;

    #[test]
    fn test_fails_on_first_error() {
        new_ucmd!()
            .arg("-v")
            .arg("--getalignoff")
            .arg("--getbsz")
            .arg("/dev/null")
            .fails()
            .code_is(1)
            .stdout_is("get alignment offset in bytes failed.\n")
            .stderr_contains("Inappropriate ioctl for device");
    }

    #[test]
    fn test_report_continues_on_errors() {
        new_ucmd!()
            .arg("--report")
            .arg("/dev/null")
            .arg("/non/existing")
            .fails()
            .code_is(1)
            .stderr_matches(
                &Regex::new("(?ms)Inappropriate ioctl for device.*No such file or directory")
                    .unwrap(),
            );
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use uutests::new_ucmd;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .arg("--report")
            .arg("/dev/null")
            .fails()
            .code_is(1)
            .stderr_is("blockdev: `blockdev` is available only on Linux.\n");
    }
}
