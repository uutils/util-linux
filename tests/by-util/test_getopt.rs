#[cfg(target_os = "linux")]
mod linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_invalid_arg() {
        new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
    }

    #[test]
    fn test_short_options() {
        new_ucmd!()
            .args(&[
                "-o", "abc:d:", "--", "-a", "-b", "-cfoo", "-d", "bar", "a1", "a2",
            ])
            .succeeds()
            .stdout_is("-a -b -c 'foo' -d 'bar' -- 'a1' 'a2' ");
    }

    #[test]
    fn test_long_options() {
        new_ucmd!()
            .args(&[
                "-o",
                "x",
                "-l",
                "condition:,output-file:,testing",
                "--",
                "--condition=foo",
                "--testing",
                "--output-file",
                "abc.def",
                "-x",
                "a1",
                "a2",
            ])
            .succeeds()
            .stdout_is("--condition 'foo' --testing --output-file 'abc.def' -x -- 'a1' 'a2' ");
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .args(&["-o", "abc", "--", "-a"])
            .fails()
            .code_is(1)
            .stderr_contains("`getopt` is fully supported only on Linux");
    }
}
