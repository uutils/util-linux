// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::common::util::TestScenario;
use std::path::Path;

fn write_file_content(dir: &Path, name: &str, content: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), content).unwrap();
}

const MEMORY_BLOCK_IDS: [usize; 125] = [
    0, 1, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117,
    118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136,
    137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 2, 3, 32, 33, 34, 35, 36, 37,
    38, 39, 4, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 5, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59,
    6, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82,
    83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99,
];

struct TestSysMemory {
    sysroot: String,
}

/// Builds up a fake /sys/devices/system/memory filesystem.
///
/// /sys/devices/system/memory/block_size_bytes
/// /sys/devices/system/memory/memoryXX/removable
/// /sys/devices/system/memory/memoryXX/state
/// /sys/devices/system/memory/memoryXX/valid_zones
/// /sys/devices/system/memory/memoryXX/node0/ (folder)
///
/// And removes it automatically after the reference is dropped.
impl TestSysMemory {
    fn new() -> Self {
        let random = rand::random::<u32>();
        let sysroot = Path::new(&env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("testsysmem-{random}"));
        let sysmem = sysroot
            .join("sys")
            .join("devices")
            .join("system")
            .join("memory");
        write_file_content(&sysmem, "block_size_bytes", "8000000\n");

        for i in MEMORY_BLOCK_IDS {
            let block_dir = sysmem.join(format!("memory{}", i));
            write_file_content(&block_dir, "removable", "1\n");
            write_file_content(&block_dir, "state", "online\n");
            let valid_zone = match i {
                0 => "none\n",
                1..=6 => "DMA32\n",
                _ => "Normal\n",
            };
            write_file_content(&block_dir, "valid_zones", valid_zone);
            let node_dir = block_dir.join("node0");
            write_file_content(&node_dir, ".gitkeep", "");
        }

        TestSysMemory {
            sysroot: sysroot.display().to_string(),
        }
    }
}

impl Drop for TestSysMemory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.sysroot).unwrap();
    }
}

fn sysroot_test_with_args(test_root: &TestSysMemory, expected_output: &str, args: &[&str]) {
    let mut cmd = new_ucmd!();
    cmd.arg("-s").arg(&test_root.sysroot);
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
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_columns_json.expected",
        &["-o", "block,size", "-J"],
    );
}

#[test]
fn test_columns_pairs() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_columns_pairs.expected",
        &["-o", "block,size", "-P"],
    );
}

#[test]
fn test_columns_raw() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_columns_raw.expected",
        &["-o", "block,size", "-r"],
    );
}

#[test]
fn test_columns_table() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_columns_table.expected",
        &["-o", "block,size"],
    );
}

#[test]
fn test_json() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_json.expected", &["-J"]);
}

#[test]
fn test_json_all() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_json_all.expected", &["-J", "-a"]);
}

#[test]
fn test_json_bytes() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_json_bytes.expected", &["-J", "-b"]);
}

#[test]
fn test_json_noheadings() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_json_noheadings.expected",
        &["-J", "-n"],
    );
}

#[test]
fn test_pairs() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_pairs.expected", &["-P"]);
}

#[test]
fn test_pairs_all() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_pairs_all.expected", &["-P", "-a"]);
}

#[test]
fn test_pairs_bytes() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_pairs_bytes.expected", &["-P", "-b"]);
}

#[test]
fn test_pairs_noheadings() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_pairs_noheadings.expected",
        &["-P", "-n"],
    );
}

#[test]
fn test_raw() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_raw.expected", &["-r"]);
}

#[test]
fn test_raw_all() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_raw_all.expected", &["-r", "-a"]);
}

#[test]
fn test_raw_bytes() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_raw_bytes.expected", &["-r", "-b"]);
}

#[test]
fn test_raw_noheadings() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_raw_noheadings.expected",
        &["-r", "-n"],
    );
}

#[test]
fn test_split_node() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_split_node.expected",
        &["-S", "node"],
    );
}

#[test]
fn test_split_output_default() {
    // If split is not provided, then it defaults to splitting on the provided(or default) columns
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_split_output_default.expected",
        &["-o", "block,size,zones,node"],
    );
}

#[test]
fn test_split_removable() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_split_removable.expected",
        &["-S", "removable"],
    );
}

#[test]
fn test_split_state() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_split_state.expected",
        &["-S", "state"],
    );
}

#[test]
fn test_split_zones() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_split_zones.expected",
        &["-S", "zones"],
    );
}

#[test]
fn test_summary_always() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_summary_always.expected",
        &["--summary=always"],
    );
}

#[test]
fn test_summary_empty() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_summary_empty.expected",
        &["--summary"],
    );
}

#[test]
fn test_summary_never() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_summary_never.expected",
        &["--summary=never"],
    );
}

#[test]
fn test_summary_only() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(
        &test_root,
        "test_lsmem_summary_only.expected",
        &["--summary=only"],
    );
}

#[test]
fn test_summary_conflict_json() {
    new_ucmd!().arg("--summary").arg("-J").fails().code_is(1);
}

#[test]
fn test_summary_conflict_pairs() {
    new_ucmd!().arg("--summary").arg("-P").fails().code_is(1);
}

#[test]
fn test_summary_conflict_raw() {
    new_ucmd!().arg("--summary").arg("-r").fails().code_is(1);
}

#[test]
fn test_table() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_table.expected", &[]);
}

#[test]
fn test_table_all() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_table_all.expected", &["-a"]);
}

#[test]
fn test_table_bytes() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_table_bytes.expected", &["-b"]);
}

#[test]
fn test_table_noheadings() {
    let test_root = TestSysMemory::new();
    sysroot_test_with_args(&test_root, "test_lsmem_table_noheadings.expected", &["-n"]);
}
