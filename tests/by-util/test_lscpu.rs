// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

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

    fn assert_field(stdout: &str, field: &str, value: &str) {
        assert!(stdout.contains(&format!("{field}:")), "missing {field}");
        assert!(stdout.contains(value), "missing value {value:?}");
    }

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

    res.stdout_contains("\"field\": \"CPU(s)\"")
        .stdout_contains("\"data\": \"2\"")
        .stdout_contains("\"field\": \"Vendor ID\"")
        .stdout_contains("\"data\": \"TestVendor\"")
        .stdout_contains("\"field\": \"L1d\"")
        .stdout_contains("\"data\": \"64 KiB (2 instances)\"");
}
