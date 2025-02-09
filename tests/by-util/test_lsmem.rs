// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::common::util::TestScenario;

#[must_use]
fn sysroot() -> String {
    format!("{}/tests/fixtures/lsmem/input", env!("CARGO_MANIFEST_DIR"))
}

fn sysroot_test_with_args(expected_output: &str, args: &[&str]) {
    let mut cmd = new_ucmd!();
    cmd.arg("-s").arg(sysroot());
    for arg in args {
        cmd.arg(arg);
    }
    cmd.succeeds()
        .no_stderr()
        .stdout_is_templated_fixture(expected_output, &[("\r\n", "\n")]);
}

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_columns_json() {
    sysroot_test_with_args(
        "test_lsmem_columns_json.expected",
        &["-o", "block,size", "-J"],
    );
}

#[test]
fn test_columns_pairs() {
    sysroot_test_with_args(
        "test_lsmem_columns_pairs.expected",
        &["-o", "block,size", "-P"],
    );
}

#[test]
fn test_columns_raw() {
    sysroot_test_with_args(
        "test_lsmem_columns_raw.expected",
        &["-o", "block,size", "-r"],
    );
}

#[test]
fn test_columns_table() {
    sysroot_test_with_args("test_lsmem_columns_table.expected", &["-o", "block,size"]);
}

#[test]
fn test_json() {
    sysroot_test_with_args("test_lsmem_json.expected", &["-J"]);
}

#[test]
fn test_json_all() {
    sysroot_test_with_args("test_lsmem_json_all.expected", &["-J", "-a"]);
}

#[test]
fn test_json_bytes() {
    sysroot_test_with_args("test_lsmem_json_bytes.expected", &["-J", "-b"]);
}

#[test]
fn test_json_noheadings() {
    sysroot_test_with_args("test_lsmem_json_noheadings.expected", &["-J", "-n"]);
}

#[test]
fn test_pairs() {
    sysroot_test_with_args("test_lsmem_pairs.expected", &["-P"]);
}

#[test]
fn test_pairs_all() {
    sysroot_test_with_args("test_lsmem_pairs_all.expected", &["-P", "-a"]);
}

#[test]
fn test_pairs_bytes() {
    sysroot_test_with_args("test_lsmem_pairs_bytes.expected", &["-P", "-b"]);
}

#[test]
fn test_pairs_noheadings() {
    sysroot_test_with_args("test_lsmem_pairs_noheadings.expected", &["-P", "-n"]);
}

#[test]
fn test_raw() {
    sysroot_test_with_args("test_lsmem_raw.expected", &["-r"]);
}

#[test]
fn test_raw_all() {
    sysroot_test_with_args("test_lsmem_raw_all.expected", &["-r", "-a"]);
}

#[test]
fn test_raw_bytes() {
    sysroot_test_with_args("test_lsmem_raw_bytes.expected", &["-r", "-b"]);
}

#[test]
fn test_raw_noheadings() {
    sysroot_test_with_args("test_lsmem_raw_noheadings.expected", &["-r", "-n"]);
}

#[test]
fn test_split_node() {
    sysroot_test_with_args("test_lsmem_split_node.expected", &["-S", "node"]);
}

#[test]
fn test_split_removable() {
    sysroot_test_with_args("test_lsmem_split_removable.expected", &["-S", "removable"]);
}

#[test]
fn test_split_state() {
    sysroot_test_with_args("test_lsmem_split_state.expected", &["-S", "state"]);
}

#[test]
fn test_split_zones() {
    sysroot_test_with_args("test_lsmem_split_zones.expected", &["-S", "zones"]);
}

#[test]
fn test_table() {
    sysroot_test_with_args("test_lsmem_table.expected", &[]);
}

#[test]
fn test_table_all() {
    sysroot_test_with_args("test_lsmem_table_all.expected", &["-a"]);
}

#[test]
fn test_table_bytes() {
    sysroot_test_with_args("test_lsmem_table_bytes.expected", &["-b"]);
}

#[test]
fn test_table_noheadings() {
    sysroot_test_with_args("test_lsmem_table_noheadings.expected", &["-n"]);
}
