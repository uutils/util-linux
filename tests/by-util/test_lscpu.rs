// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use serde_json::Value;
use std::path::Path;
use uutests::new_ucmd;

const CPUINFO: &str = "\
processor\t: 0
vendor_id\t: TestVendor
cpu family\t: 6
model\t\t: 42
model name\t: Test CPU
address sizes\t: 39 bits physical, 48 bits virtual
";

fn write_file(dir: &Path, name: &str, contents: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), contents).unwrap();
}

fn assert_field(stdout: &str, field: &str, value: &str) {
    let prefix = format!("{field}:");
    let line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .unwrap_or_else(|| panic!("missing {field}"));

    let actual = line.trim_start()[prefix.len()..].trim();
    assert_eq!(actual, value, "unexpected value for {field}");
}

/// Builds a minimal fake `/sys/devices/system/cpu` and `/proc/cpuinfo`.
///
/// Values are deliberately different from typical host values so tests fail if
/// any read accidentally bypasses `--sysroot` and uses the host filesystem.
struct TestSysCpu {
    sysroot: tempfile::TempDir,
}

impl TestSysCpu {
    fn new() -> Self {
        let sysroot = tempfile::tempdir().unwrap();
        let root = sysroot.path();
        let cpu = root.join("sys/devices/system/cpu");

        write_file(&cpu, "online", "0-1\n");
        write_file(&cpu, "present", "0-1\n");
        write_file(&cpu, "offline", "\n");
        write_file(&root.join("sys/kernel"), "cpu_byteorder", "big\n");
        write_file(&cpu.join("cpufreq"), "boost", "0\n");
        write_file(
            &cpu.join("vulnerabilities"),
            "fake_vuln",
            "Mitigation: fake\n",
        );

        for cpu_index in 0..2 {
            let cpu_dir = cpu.join(format!("cpu{cpu_index}"));
            let topology = cpu_dir.join("topology");

            write_file(&topology, "physical_package_id", "0\n");
            write_file(&topology, "core_id", &format!("{cpu_index}\n"));

            let caches = [
                ("index0", "Data", "1", "32K", format!("{cpu_index:x}")),
                (
                    "index1",
                    "Instruction",
                    "1",
                    "64K",
                    format!("{cpu_index:x}"),
                ),
                ("index2", "Unified", "2", "256K", format!("{cpu_index:x}")),
                ("index3", "Unified", "3", "8M", "3".to_string()),
            ];

            for (name, cache_type, level, size, shared_cpu_map) in caches {
                let cache_dir = cpu_dir.join("cache").join(name);
                write_file(&cache_dir, "type", cache_type);
                write_file(&cache_dir, "level", level);
                write_file(&cache_dir, "size", size);
                write_file(&cache_dir, "shared_cpu_map", &shared_cpu_map);
            }
        }

        write_file(&root.join("proc"), "cpuinfo", CPUINFO);

        Self { sysroot }
    }

    fn path(&self) -> &Path {
        self.sysroot.path()
    }

    fn write_relative(&self, relative: &str, contents: &str) {
        let path = self.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn remove_file(&self, relative: &str) {
        std::fs::remove_file(self.path().join(relative)).unwrap();
    }

    fn remove_dir(&self, relative: &str) {
        std::fs::remove_dir_all(self.path().join(relative)).unwrap();
    }
}

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
#[ignore = "not yet implemented"]
fn test_hex() {
    new_ucmd!().arg("--hex").succeeds().stdout_contains("0x");
}

#[test]
#[cfg(target_os = "linux")]
fn test_json() {
    let res = new_ucmd!().arg("--json").succeeds();

    let stdout = res.no_stderr().stdout_str();
    assert!(stdout.starts_with("{"));
    assert!(stdout.ends_with("}\n"));

    res.stdout_contains("\"lscpu\": [")
        .stdout_contains("\"field\": \"Architecture\"")
        .stdout_contains("\"field\": \"CPU(s)\"")
        .stdout_contains("\"children\": [");
}

#[test]
#[cfg(target_os = "linux")]
fn test_output() {
    let res = new_ucmd!().succeeds();
    let stdout = res.no_stderr().stdout_str();

    // Non-exhaustive list of fields we expect
    // This also checks that fields which should be indented, are indeed indented as excepted
    assert!(stdout.contains("Architecture:"));
    assert!(stdout.contains("\n  Address sizes:"));
    assert!(stdout.contains("\n  Byte Order:"));
    assert!(stdout.contains("\nCPU(s):"));
    assert!(stdout.contains("\nVendor ID:"));
    assert!(stdout.contains("\n  Model name:"));
    assert!(stdout.contains("\n    CPU Family:"));
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_output() {
    let test_root = TestSysCpu::new();
    let res = new_ucmd!()
        .arg("--sysroot")
        .arg(test_root.path())
        .succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert_field(stdout, "Address sizes", "39 bits physical, 48 bits virtual");
    assert_field(stdout, "Byte Order", "Big Endian");
    assert_field(stdout, "On-line CPU(s) list", "0-1");
    assert_field(stdout, "Vendor ID", "TestVendor");
    assert_field(stdout, "Model name", "Test CPU");
    assert_field(stdout, "CPU Family", "6");
    assert_field(stdout, "Model", "42");
    assert_field(stdout, "Thread(s) per core", "1");
    assert_field(stdout, "Core(s) per socket", "2");
    assert_field(stdout, "Socket(s)", "1");
    assert_field(stdout, "Frequency boost", "disabled");

    // L1/L2 are private to each CPU, L3 is shared by both CPUs.
    assert_field(stdout, "L1d", "64 KiB (2 instances)");
    assert_field(stdout, "L1i", "128 KiB (2 instances)");
    assert_field(stdout, "L2", "512 KiB (2 instances)");
    assert_field(stdout, "L3", "8 MiB (1 instances)");

    assert!(stdout.contains("Vulnerabilities:"));
    assert_field(stdout, "Fake vuln", "Mitigation: fake");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_json() {
    let test_root = TestSysCpu::new();
    let res = new_ucmd!()
        .arg("-s")
        .arg(test_root.path())
        .arg("--json")
        .succeeds();

    fn find_entry<'a>(entries: &'a [Value], field: &str) -> Option<&'a Value> {
        for entry in entries {
            if entry["field"] == field {
                return Some(entry);
            }

            if let Some(children) = entry["children"].as_array() {
                if let Some(found) = find_entry(children, field) {
                    return Some(found);
                }
            }
        }

        None
    }

    let json: Value = serde_json::from_str(res.no_stderr().stdout_str()).unwrap();
    let entries = json["lscpu"].as_array().unwrap();

    for (field, value) in [
        ("CPU(s)", "2"),
        ("Vendor ID", "TestVendor"),
        ("L1d", "64 KiB (2 instances)"),
    ] {
        let entry = find_entry(entries, field).unwrap_or_else(|| panic!("missing field {field}"));
        assert_eq!(entry["data"], value, "unexpected value for {field}");
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_invalid() {
    let sysroot = tempfile::tempdir().unwrap();
    let missing = sysroot.path().join("missing");

    new_ucmd!()
        .arg("--sysroot")
        .arg(missing)
        .fails()
        .code_is(1)
        .stderr_contains("Could not read");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_missing_cache_dir() {
    let test_root = TestSysCpu::new();
    test_root.remove_dir("sys/devices/system/cpu/cpu0/cache");
    test_root.remove_dir("sys/devices/system/cpu/cpu1/cache");

    let res = new_ucmd!().arg("-s").arg(test_root.path()).succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert_field(stdout, "CPU(s)", "2");
    assert!(!stdout.contains("Caches (sum of all)"));
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_missing_cache_size() {
    let test_root = TestSysCpu::new();
    for cpu in 0..2 {
        test_root.remove_file(&format!(
            "sys/devices/system/cpu/cpu{cpu}/cache/index0/size"
        ));
    }

    let res = new_ucmd!().arg("-s").arg(test_root.path()).succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert!(!stdout.contains("L1d:"));
    assert_field(stdout, "L1i", "128 KiB (2 instances)");
    assert_field(stdout, "L2", "512 KiB (2 instances)");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_missing_cache_shared_cpu_map() {
    let test_root = TestSysCpu::new();
    for cpu in 0..2 {
        test_root.remove_file(&format!(
            "sys/devices/system/cpu/cpu{cpu}/cache/index1/shared_cpu_map"
        ));
    }

    let res = new_ucmd!().arg("-s").arg(test_root.path()).succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert!(!stdout.contains("L1i:"));
    assert_field(stdout, "L1d", "64 KiB (2 instances)");
    assert_field(stdout, "L2", "512 KiB (2 instances)");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_missing_cache_level() {
    let test_root = TestSysCpu::new();
    for cpu in 0..2 {
        test_root.remove_file(&format!(
            "sys/devices/system/cpu/cpu{cpu}/cache/index2/level"
        ));
    }

    let res = new_ucmd!().arg("-s").arg(test_root.path()).succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert!(!stdout.contains("L2:"));
    assert_field(stdout, "L1d", "64 KiB (2 instances)");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_unknown_cache_type() {
    let test_root = TestSysCpu::new();
    for cpu in 0..2 {
        test_root.write_relative(
            &format!("sys/devices/system/cpu/cpu{cpu}/cache/index2/type"),
            "Mystery\n",
        );
    }

    let res = new_ucmd!().arg("-s").arg(test_root.path()).succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert!(!stdout.contains("L2:"));
    assert_field(stdout, "L3", "8 MiB (1 instances)");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_missing_topology() {
    let test_root = TestSysCpu::new();
    for cpu in 0..2 {
        test_root.remove_file(&format!(
            "sys/devices/system/cpu/cpu{cpu}/topology/physical_package_id"
        ));
        test_root.remove_file(&format!("sys/devices/system/cpu/cpu{cpu}/topology/core_id"));
    }

    let res = new_ucmd!().arg("-s").arg(test_root.path()).succeeds();
    let stdout = res.no_stderr().stdout_str();

    assert_field(stdout, "CPU(s)", "2");
    assert_field(stdout, "Thread(s) per core", "1");
    assert_field(stdout, "Core(s) per socket", "2");
    assert_field(stdout, "Socket(s)", "1");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_invalid_cpu_list() {
    let test_root = TestSysCpu::new();
    test_root.write_relative("sys/devices/system/cpu/online", "0-abc\n");

    new_ucmd!()
        .arg("-s")
        .arg(test_root.path())
        .fails()
        .code_is(1)
        .stderr_contains("Invalid CPU index value: abc");
}
