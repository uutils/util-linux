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
fn test_operations_mutually_exclusive() {
    new_ucmd!()
        .arg("--freeze")
        .arg("--unfreeze")
        .arg("/foo")
        .fails()
        .code_is(1)
        .stderr_contains("the argument '--freeze' cannot be used with '--unfreeze'");
}

#[cfg(target_os = "linux")]
mod linux {

    use uutests::new_ucmd;

    #[test]
    fn test_fails_on_non_existing_path() {
        new_ucmd!()
            .arg("--unfreeze")
            .arg("/non/existing")
            .fails()
            .code_is(1)
            .stderr_contains("No such file or directory");
    }

    #[test]
    fn test_fails_on_non_directory() {
        new_ucmd!()
            .arg("--unfreeze")
            .arg("/dev/null")
            .fails()
            .code_is(1)
            .stderr_contains("not a directory");
    }
}

#[cfg(not(target_os = "linux"))]
mod non_linux {
    use uutests::new_ucmd;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .arg("--unfreeze")
            .arg("/non/existing")
            .fails()
            .code_is(1)
            .stderr_is("fsfreeze: `fsfreeze` is available only on Linux.\n");
    }
}
