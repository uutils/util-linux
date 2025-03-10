use crate::common::util::TestScenario;

#[cfg(target_os = "linux")]
#[test]
fn test_findmnt() {
    new_ucmd!().succeeds().stdout_contains("/proc");
}
