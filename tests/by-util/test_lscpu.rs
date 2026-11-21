// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

#[cfg(target_os = "linux")]
use std::path::Path;
use uutests::new_ucmd;

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

#[cfg(target_os = "linux")]
fn write_file(dir: &Path, name: &str, content: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(name), content).unwrap();
}

#[cfg(target_os = "linux")]
/// Builds a minimal fake sysfs/procfs tree for lscpu testing.
///
/// /proc/cpuinfo
/// /sys/devices/system/cpu/online
/// /sys/devices/system/cpu/cpu{N}/topology/{physical_package_id,core_id}
/// /sys/devices/system/cpu/cpu{N}/cache/index0/{type,level,size,shared_cpu_map}
/// /sys/kernel/cpu_byteorder
struct TestSysCpu {
    sysroot: tempfile::TempDir,
}

#[cfg(target_os = "linux")]
impl TestSysCpu {
    fn new() -> Self {
        let sysroot = tempfile::TempDir::new().unwrap();
        let root = sysroot.path();

        // /proc/cpuinfo
        let proc_dir = root.join("proc");
        write_file(
            &proc_dir,
            "cpuinfo",
            "processor\t: 0\n\
             vendor_id\t: GenuineIntel\n\
             cpu family\t: 6\n\
             model\t\t: 142\n\
             model name\t: Test CPU @ 1.00GHz\n\
             address sizes\t: 39 bits physical, 48 bits virtual\n\
             \n\
             processor\t: 1\n\
             vendor_id\t: GenuineIntel\n\
             cpu family\t: 6\n\
             model\t\t: 142\n\
             model name\t: Test CPU @ 1.00GHz\n\
             address sizes\t: 39 bits physical, 48 bits virtual\n",
        );

        // /sys/devices/system/cpu/
        let cpu_base = root.join("sys").join("devices").join("system").join("cpu");
        write_file(&cpu_base, "online", "0-1");

        // Two CPUs, one socket, two cores
        for (cpu, core_id, cpu_map) in [("cpu0", "0", "00000001"), ("cpu1", "1", "00000002")] {
            let topo = cpu_base.join(cpu).join("topology");
            write_file(&topo, "physical_package_id", "0");
            write_file(&topo, "core_id", core_id);

            let cache = cpu_base.join(cpu).join("cache").join("index0");
            write_file(&cache, "type", "Unified");
            write_file(&cache, "level", "1");
            write_file(&cache, "size", "512K");
            write_file(&cache, "shared_cpu_map", cpu_map);
        }

        // /sys/kernel/cpu_byteorder
        let kernel_dir = root.join("sys").join("kernel");
        write_file(&kernel_dir, "cpu_byteorder", "little");

        TestSysCpu { sysroot }
    }

    fn path(&self) -> &Path {
        self.sysroot.path()
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_basic() {
    let sys = TestSysCpu::new();
    new_ucmd!()
        .args(&["-s", sys.path().to_str().unwrap()])
        .succeeds()
        .no_stderr()
        .stdout_contains("CPU(s):")
        .stdout_contains("2")
        .stdout_contains("0-1")
        .stdout_contains("Little Endian")
        .stdout_contains("GenuineIntel")
        .stdout_contains("Test CPU @ 1.00GHz")
        .stdout_contains("Socket(s):");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_json() {
    let sys = TestSysCpu::new();
    let res = new_ucmd!()
        .args(&["-s", sys.path().to_str().unwrap(), "--json"])
        .succeeds();
    res.no_stderr();

    let stdout = res.stdout_str();
    assert!(stdout.starts_with("{"));
    assert!(stdout.ends_with("}\n"));

    res.stdout_contains("\"field\": \"CPU(s)\"")
        .stdout_contains("\"data\": \"2\"")
        .stdout_contains("GenuineIntel")
        .stdout_contains("Little Endian");
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_long_flag() {
    // --sysroot and -s should behave identically
    let sys = TestSysCpu::new();
    let out_short = new_ucmd!()
        .args(&["-s", sys.path().to_str().unwrap()])
        .succeeds()
        .stdout_str()
        .to_string();
    let out_long = new_ucmd!()
        .args(&["--sysroot", sys.path().to_str().unwrap()])
        .succeeds()
        .stdout_str()
        .to_string();
    assert_eq!(out_short, out_long);
}

#[test]
#[cfg(target_os = "linux")]
fn test_sysroot_cache() {
    let sys = TestSysCpu::new();
    // Two CPUs each with a 512K L1 cache and unique shared_cpu_map, so 2 instances total
    new_ucmd!()
        .args(&["-s", sys.path().to_str().unwrap()])
        .succeeds()
        .no_stderr()
        .stdout_contains("Caches (sum of all):")
        .stdout_contains("(2 instances)");
}
