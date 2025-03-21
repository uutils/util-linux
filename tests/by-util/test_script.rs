// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
mod linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_help() {
        new_ucmd!().arg("--help").succeeds();
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use crate::common::util::TestScenario;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .arg("-c")
            .arg("echo test")
            .fails()
            .code_is(1)
            .stderr_is("script: The 'script' utility is only available on Linux systems\n");
    }
}
