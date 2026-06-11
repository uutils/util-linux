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
#[cfg(target_os = "linux")]
fn test_non_numerical_pid() {
    let res = new_ucmd!().arg("xyz").run();

    let stdout = res.stdout_str();
    let stderr = res.stderr_str();

    assert!(stdout.trim().len() == 0);
    assert!(stderr.contains("invalid value 'xyz'"));
}

#[test]
#[cfg(target_os = "linux")]
fn test_pid_doesnt_exist() {
    let non_existent_pid = "1234567890";
    let res = new_ucmd!().arg(non_existent_pid).run();

    let stdout = res.stdout_str();
    let stderr = res.stderr_str();
    let error_msg = format!("bash: kill: ({non_existent_pid}) - No such process");

    assert!(stdout.trim().len() == 0);
    assert!(stderr.contains(error_msg.as_str()));
}
