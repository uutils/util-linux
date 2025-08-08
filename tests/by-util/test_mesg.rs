// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;
use uutests::util::TestScenario;
use uutests::util_name;

#[test]
fn test_invalid_verb() {
    new_ucmd!().arg("foo").fails().code_is(1);
}

#[test]
#[cfg(target_family = "unix")]
fn test_no_terminal() {
    for args in &[vec![], vec!["y"], vec!["n"]] {
        new_ucmd!()
            .args(args)
            .fails()
            .code_is(2)
            .stderr_contains("stdin/stdout/stderr is not a terminal");
    }
}

#[cfg(not(target_family = "unix"))]
mod non_unix {
    use uutests::new_ucmd;
    use uutests::util::TestScenario;
    use uutests::util_name;

    #[test]
    fn test_fails_on_unsupported_platforms() {
        new_ucmd!()
            .fails()
            .code_is(1)
            .stderr_is("mesg: `mesg` is available only on Unix platforms.\n");
    }
}
