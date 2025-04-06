// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use regex::Regex;

use crate::common::util::TestScenario;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_fails_on_non_existing_path() {
    new_ucmd!()
        .arg("/non/existing")
        .fails()
        .code_is(1)
        .stderr_contains("No such file or directory");
}

#[cfg(unix)]
#[test]
fn test_fails_on_no_permission() {
    let (at, mut ucmd) = at_and_ucmd!();
    at.mkdir("noperms");
    at.make_file("noperms/testfile");
    at.set_mode("noperms", 0);
    let argmnt = at.plus_as_string("noperms/testfile");
    ucmd.arg(argmnt)
        .fails()
        .code_is(1)
        .stderr_contains("Permission denied");
}

#[test]
fn test_long_arg() {
    let scene = TestScenario::new(util_name!());
    let at = &scene.fixtures;
    at.touch(at.plus_as_string("test-long"));

    #[cfg(unix)]
    let regex = r" *[-bcCdDlMnpPsStTx?]([r-][w-][xt-]){3} [a-z0-9_\.][a-z0-9_\-\.]*[$]? [a-z0-9_\.][a-z0-9_\-\.]*[$]? .*";
    #[cfg(target_os="windows")]
    let regex = r"[-dl](r[w-]x){3}.*";

    let re = &Regex::new(regex).unwrap();

    let args = vec!["-l", "--long"];
    for arg in args {
        let result = scene.ucmd().arg(arg).arg(at.as_string()).succeeds();
        result.stdout_matches(re);
    }
}

#[test]
fn test_modes_arg() {
    let scene = TestScenario::new(util_name!());
    let at = &scene.fixtures;
    at.touch(at.plus_as_string("test-modes"));

    let regex = r" +[-bcCdDlMnpPsStTx?]([r-][w-][xt-]){3} .*";

    let re = &Regex::new(regex).unwrap();

    let args = vec!["-m", "--modes"];
    for arg in args {
        let result = scene.ucmd().arg(arg).arg(at.as_string()).succeeds();
        result.stdout_matches(re);
    }
}

#[cfg(unix)]
#[test]
fn test_owners_arg() {
    let scene = TestScenario::new(util_name!());
    let at = &scene.fixtures;
    at.touch(at.plus_as_string("test-owners"));

    #[cfg(not(windows))]
    let regex =
        r" +[-bcCdDlMnpPsStTx?] [a-z0-9_\.][a-z0-9_\-\.]*[$]? [a-z0-9_\.][a-z0-9_\-\.]*[$]? .*";

    let re = &Regex::new(regex).unwrap();

    let args = vec!["-o", "--owners"];
    for arg in args {
        let result = scene.ucmd().arg(arg).arg(at.as_string()).succeeds();
        result.stdout_matches(re);
    }
}

#[test]
fn test_vertical_arg() {
    let scene = TestScenario::new(util_name!());
    let at = &scene.fixtures;
    at.touch(at.plus_as_string("test-vertical"));

    let regex = r"[-bcCdDlMnpPsStTx?] +.*";

    let re = &Regex::new(regex).unwrap();

    let args = vec!["-v", "--vertical"];
    for arg in args {
        let result = scene.ucmd().arg(arg).arg(at.as_string()).succeeds();
        result.stdout_matches(re);
    }
}
