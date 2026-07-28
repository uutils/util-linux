// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

#[test]
#[cfg(target_os = "linux")]
fn test_invalid_arg() {
    new_ucmd!()
    	.arg("--definitely-invalid")
    	.fails()
    	.code_is(1);
}

#[test]
#[cfg(target_os = "linux")]
fn invalid_user() {
	new_ucmd!()
		.arg("--user=fools_have_this_username")
		.arg("--command=\"echo hello_world\"")
		.fails()
		.code_is(1)
		.stderr_contains("User doesn't exist");
}

#[test]
#[cfg(target_os = "linux")]
fn invalid_group() {
	new_ucmd!()
		.arg("--user=root")
		.arg("--group=hopefully_nonexistant_group")
		.arg("--command=\"echo hello_world\"")
		.fails()
		.code_is(1)
		.stderr_contains("Group doesn't exist");
}


#[test]
#[cfg(target_os = "linux")]
fn invalid_supp_group() {
	new_ucmd!()
		.arg("--user=root")
		.arg("--supp-group=does_anyone_read_this")
		.arg("--command=\"echo hello_world\"")
		.fails()
		.code_is(1)
		.stderr_contains("Supp group doesn't exist");
}

#[test]
#[cfg(target_os = "linux")]
fn missing_command() {
	new_ucmd!()
		.arg("--user=root")
		.fails()
		.code_is(1)
		.stderr_contains("Incorrect usage");
}



#[cfg(not(target_os = "linux"))]
fn unsupported_platform () {
	new_ucmd!()
		.fails()
		.code_is(1)
		.stderr_contains("`runuser` is only available on linux")
}
