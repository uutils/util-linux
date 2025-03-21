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
fn test_help() {
    new_ucmd!().arg("--help").succeeds();
}

#[test]
fn test_version() {
    new_ucmd!().arg("--version").succeeds();
}

#[test]
fn test_mutually_exclusive_args() {
    new_ucmd!()
        .arg("--log-io")
        .arg("io.log")
        .arg("--log-out")
        .arg("out.log")
        .fails()
        .code_is(1)
        .stderr_contains("the argument '--log-io' cannot be used with '--log-out'");
}

#[cfg(target_family = "unix")]
mod unix {
    use crate::common::util::TestScenario;
    use regex::Regex;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_basic_script() {
        let ts_file = "typescript_test";
        let result = new_ucmd!()
            .arg("-c")
            .arg("echo hello world")
            .arg(ts_file)
            .run();

        result.success();

        // Check that the typescript file was created
        assert!(Path::new(ts_file).exists());

        // Check the content of the typescript file
        let content = fs::read_to_string(ts_file).unwrap();
        assert!(content.contains("hello world"));

        // Clean up
        fs::remove_file(ts_file).unwrap();
    }

    #[test]
    fn test_append_mode() {
        let ts_file = "typescript_append";

        // First run
        new_ucmd!()
            .arg("-c")
            .arg("echo first run")
            .arg(ts_file)
            .succeeds();

        // Second run with append
        new_ucmd!()
            .arg("-a")
            .arg("-c")
            .arg("echo second run")
            .arg(ts_file)
            .succeeds();

        // Check the content of the typescript file
        let content = fs::read_to_string(ts_file).unwrap();
        assert!(content.contains("first run"));
        assert!(content.contains("second run"));

        // Clean up
        fs::remove_file(ts_file).unwrap();
    }

    #[test]
    fn test_return_exit_status() {
        // Test with successful command
        new_ucmd!()
            .arg("-e")
            .arg("-c")
            .arg("true")
            .succeeds()
            .code_is(0);

        // Test with failing command
        new_ucmd!()
            .arg("-e")
            .arg("-c")
            .arg("false")
            .fails()
            .code_is(1);
    }

    #[test]
    fn test_timing_file() {
        let ts_file = "typescript_timing";
        let timing_file = "timing.log";

        new_ucmd!()
            .arg("-c")
            .arg("echo timing test")
            .arg("-T")
            .arg(timing_file)
            .arg(ts_file)
            .succeeds();

        // Check that both files were created
        assert!(Path::new(ts_file).exists());
        assert!(Path::new(timing_file).exists());

        // Check that the timing file has the expected format
        let timing_content = fs::read_to_string(timing_file).unwrap();
        let re = Regex::new(r"^\d+\.\d+ \d+$").unwrap();
        assert!(re.is_match(timing_content.lines().next().unwrap()));

        // Clean up
        fs::remove_file(ts_file).unwrap();
        fs::remove_file(timing_file).unwrap();
    }

    #[test]
    fn test_advanced_logging_format() {
        let ts_file = "typescript_advanced";
        let timing_file = "timing_advanced.log";
        let io_file = "io.log";

        new_ucmd!()
            .arg("-c")
            .arg("echo advanced test")
            .arg("-T")
            .arg(timing_file)
            .arg("-B")
            .arg(io_file)
            .arg("-m")
            .arg("advanced")
            .arg(ts_file)
            .succeeds();

        // Check that all files were created
        assert!(Path::new(ts_file).exists());
        assert!(Path::new(timing_file).exists());
        assert!(Path::new(io_file).exists());

        // Check that the timing file has the expected advanced format
        let timing_content = fs::read_to_string(timing_file).unwrap();
        let re = Regex::new(r"^[IO] \d+\.\d+ \d+$").unwrap();
        assert!(re.is_match(timing_content.lines().next().unwrap()));

        // Clean up
        fs::remove_file(ts_file).unwrap();
        fs::remove_file(timing_file).unwrap();
        fs::remove_file(io_file).unwrap();
    }

    #[test]
    fn test_output_limit() {
        let ts_file = "typescript_limit";

        new_ucmd!()
            .arg("-c")
            .arg("yes | head -n 1000") // Generate a lot of output
            .arg("-o")
            .arg("1K") // Limit to 1KB
            .arg(ts_file)
            .succeeds();

        // Check that the typescript file was created and is limited in size
        assert!(Path::new(ts_file).exists());
        let metadata = fs::metadata(ts_file).unwrap();
        assert!(metadata.len() <= 1024 + 100); // Allow some margin for terminal control sequences

        // Clean up
        fs::remove_file(ts_file).unwrap();
    }
}

#[cfg(not(target_family = "unix"))]
mod non_unix {
    use crate::common::util::TestScenario;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .arg("-c")
            .arg("echo test")
            .fails()
            .code_is(1)
            .stderr_is("script: `script` is unavailable on non-UNIX-like platforms.\n");
    }
}

