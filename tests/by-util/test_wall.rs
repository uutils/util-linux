#[cfg(unix)]
mod tests {
    use uutests::new_ucmd;

    #[test]
    fn test_invalid_arg() {
        new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
    }

    #[test]
    fn test_fails_on_invalid_group() {
        new_ucmd!()
            .arg("-g")
            .arg("fooblywoobly") // assuming this group doesnt exist
            .fails()
            .code_is(1)
            .stderr_is("wall: fooblywoobly: unknown group\n");
    }

    #[test]
    fn test_fails_on_invalid_gid() {
        new_ucmd!()
            .arg("-g")
            .arg("99999") // assuming this group doesnt exist
            .fails()
            .code_is(1)
            .stderr_is("wall: 99999: unknown group\n");
    }

    #[test]
    fn test_warns_on_nobanner() {
        new_ucmd!()
            .arg("-n")
            .arg("some text to wall")
            .succeeds()
            .code_is(0)
            .stderr_is("wall: --nobanner is available only for root\n");
    }

    #[test]
    fn test_fails_on_invalid_timeout() {
        new_ucmd!()
            .arg("-t")
            .arg("0")
            .fails()
            .code_is(1)
            .stderr_is("wall: invalid timeout argument: 0\n");
    }

    #[test]
    fn test_succeeds_no_stdout() {
        new_ucmd!().pipe_in("pipe me").succeeds().stdout_is("");
    }
}
