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
fn test_nologin_args() {
    let args_to_try: &[&[&str]] = &[
        &[],
        &["-c", "command"],
        &["--init-file", "file"],
        &["-i"],
        &["--interactive"],
        &["-l"],
        &["--login"],
        &["--noprofile"],
        &["--norc"],
        &["--posix"],
        &["--rcfile", "file"],
        &["--restricted"],
        &["-r"],
    ];
    for args in args_to_try {
        new_ucmd!()
            .args(args)
            .fails()
            .code_is(1)
            .stdout_contains("This account is currently not available.");
    }
}
