// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
mod linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_invalid_path() {
        new_ucmd!()
            .arg("-d")
            .arg("/foo/bar/baz")
            .fails()
            .code_is(2)
            .stderr_contains("failed to open /foo/bar/baz: No such file or directory");
    }

    #[test]
    fn test_invalid_arg() {
        new_ucmd!().arg("foo").fails().code_is(1);
    }
    #[test]
    fn test_empty_args() {
        new_ucmd!().fails().code_is(2).stderr_contains("Usage:");
    }

    #[test]
    fn test_empty_file() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.touch("empty");
        ucmd.arg("-d")
            .arg("empty")
            .fails()
            .stderr_contains("swap space needs to be at least");
    }

    #[test]
    fn test_min_size() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 4096]);
        ucmd.arg("-d")
            .arg("swap")
            .fails()
            .stderr_contains("swap space needs to be at least");
    }

    #[test]
    fn test_swapfile() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.write_bytes("swap", &[0; 65536]);
        ucmd.arg("-d")
            .arg("swap")
            .succeeds()
            .code_is(0)
            .stdout_contains("Setting up swapspace version 1")
            .stdout_contains("insecure file owner");
    }

    #[test]
    fn test_directory_err() {
        let (at, mut ucmd) = at_and_ucmd!();
        at.mkdir("foo");
        ucmd.arg("-d")
            .arg("foo")
            .fails()
            .code_is(2)
            .stderr_contains("failed to open foo: Is a directory");
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .fails()
            .code_is(1)
            .stderr_is("mkswap: `mkswap` is available only on Linux.\n");
    }
}
