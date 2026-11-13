// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;
use uutests::util::TestScenario;
use uutests::util_name;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_missing_args() {
    new_ucmd!()
        .fails()
        .code_is(1)
        .stderr_contains("required arguments were not provided");
}

#[test]
fn test_lock_mode_override() {
    let ts = TestScenario::new(util_name!());
    let lockfile = ts.fixtures.plus("lockfile");
    let lockfile = lockfile.to_str().unwrap();
    ts.ucmd().args(&["-s", "-x", lockfile, "true"]).succeeds();
    ts.ucmd().args(&["-u", "-s", lockfile, "true"]).succeeds();
    ts.ucmd().args(&["-s", "-s", lockfile, "true"]).succeeds();
}

#[test]
fn test_short_v_is_invalid() {
    new_ucmd!()
        .args(&["-v", "file", "true"])
        .fails()
        .code_is(1)
        .stderr_contains("unexpected argument '-v'");
}

#[test]
fn test_command_flag_and_positional_conflict() {
    new_ucmd!()
        .args(&["file", "-c", "echo hi", "extra_arg"])
        .fails()
        .stderr_contains("cannot be used with");
}

#[test]
fn test_conflict_exit_code_out_of_range() {
    new_ucmd!()
        .args(&["-E", "300", "lockfile", "true"])
        .fails()
        .code_is(1)
        .stderr_contains("300 is not in 0..=255");
}
