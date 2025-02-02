use crate::common::util::TestScenario;

#[test]
fn test_findmnt() {
    new_ucmd!().succeeds().stdout_contains("/proc");
}
