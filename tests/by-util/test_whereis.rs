use crate::common::util::TestScenario;

#[test]
#[cfg(target_os = "linux")]
fn test_basic_lookup() {
    new_ucmd!().arg("gcc").succeeds().stdout_contains("/usr/bin/gcc");
}

#[test]
#[cfg(target_os = "linux")]
fn test_bin_only() {
    new_ucmd!().arg("-b").arg("ping").succeeds().stdout_contains("/usr/bin/ping");
}

#[test]
#[cfg(target_os = "linux")]
fn test_man_only() {
    new_ucmd!().arg("-m").arg("nmap").succeeds().stdout_contains("/usr/share/man/man1/nmap.1.gz");
}

#[test]
#[cfg(target_os = "linux")]
// dig doesn't seem to have any output when passing in the -s flag.
fn test_src_only() {
    new_ucmd!().arg("-s").arg("dig").succeeds().stdout_contains("");
}

#[test]
#[cfg(target_os = "linux")]
fn test_output() {

    let res = new_ucmd!().arg("ping").arg("gcc").succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Non-exhaustive list of fields we expect
    // Check that 'ping' and 'gcc' have their paths listed
    assert!(stdout.contains("ping:"));
    assert!(stdout.contains("gcc:"));

    // Check that paths are printed next to the command name, as expected
    assert!(stdout.contains("/usr/bin/ping"));
    assert!(stdout.contains("/usr/bin/gcc"));
    
    assert!(stdout.contains("/usr/lib/gcc"));
    assert!(stdout.contains("/usr/share/gcc"));
}

