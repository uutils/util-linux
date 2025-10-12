// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{collections::HashSet, fs, path::PathBuf};
use uucore::parser::parse_size;

pub struct CpuVulnerability {
    pub name: String,
    pub mitigation: String,
}

pub struct CpuTopology {
    pub cpus: Vec<Cpu>,
}

#[derive(Debug)]
pub struct Cpu {
    _index: usize,
    pub pkg_id: usize,
    pub core_id: usize,
    pub caches: Vec<CpuCache>,
}

#[derive(Debug)]
pub struct CpuCache {
    pub typ: CacheType,
    pub level: usize,
    pub size: CacheSize,
    pub shared_cpu_map: String,
}

#[derive(Debug)]
pub struct CacheSize(u64);

#[derive(Debug)]
pub enum CacheType {
    Data,
    Instruction,
    Unified,
}

impl CpuTopology {
    pub fn new() -> Self {
        let mut out: Vec<Cpu> = vec![];

        let online_cpus = parse_cpu_list(&read_online_cpus());

        for cpu_index in online_cpus {
            let cpu_dir = PathBuf::from(format!("/sys/devices/system/cpu/cpu{cpu_index}/"));

            let pkg_id = fs::read_to_string(cpu_dir.join("topology/physical_package_id"))
                .unwrap()
                .trim()
                .parse::<usize>()
                .unwrap();

            let core_id = fs::read_to_string(cpu_dir.join("topology/core_id"))
                .unwrap()
                .trim()
                .parse::<usize>()
                .unwrap();

            let caches = read_cpu_caches(cpu_index);

            out.push(Cpu {
                _index: cpu_index,
                pkg_id,
                core_id,
                caches,
            })
        }
        Self { cpus: out }
    }

    pub fn socket_count(&self) -> usize {
        // Each physical socket is represented as its own package_id, so amount of unique pkg_ids = sockets
        // https://www.kernel.org/doc/html/latest/admin-guide/abi-stable.html#abi-sys-devices-system-cpu-cpux-topology-physical-package-id
        let physical_sockets: HashSet<_> = self.cpus.iter().map(|cpu| cpu.pkg_id).collect();

        physical_sockets.len()
    }

    pub fn core_count(&self) -> usize {
        let core_ids: HashSet<_> = self.cpus.iter().map(|cpu| cpu.core_id).collect();
        core_ids.len()
    }
}

impl CacheSize {
    pub fn new(size: u64) -> Self {
        Self(size)
    }

    fn parse(s: &str) -> Self {
        Self(parse_size::parse_size_u64(s).expect("Could not parse cache size"))
    }

    pub fn size_bytes(&self) -> u64 {
        self.0
    }

    pub fn raw(&self) -> String {
        format!("{}", self.0)
    }

    pub fn human_readable(&self) -> String {
        let (unit, denominator) = match self.0 {
            x if x < 1024_u64.pow(1) => ("B", 1024_u64.pow(0)),
            x if x < 1024_u64.pow(2) => ("KiB", 1024_u64.pow(1)),
            x if x < 1024_u64.pow(3) => ("MiB", 1024_u64.pow(2)),
            x if x < 1024_u64.pow(4) => ("GiB", 1024_u64.pow(3)),
            x if x < 1024_u64.pow(5) => ("TiB", 1024_u64.pow(4)),
            _ => return format!("{} bytes", self.0),
        };
        let scaled_size = self.0 / denominator;
        format!("{scaled_size} {unit}")
    }
}

// TODO: respect `--hex` option and output the bitmask instead of human-readable range
pub fn read_online_cpus() -> String {
    fs::read_to_string("/sys/devices/system/cpu/online")
        .expect("Could not read sysfs")
        .trim()
        .to_string()
}

fn read_cpu_caches(cpu_index: usize) -> Vec<CpuCache> {
    let cpu_dir = PathBuf::from(format!("/sys/devices/system/cpu/cpu{cpu_index}/"));
    let cache_dir = fs::read_dir(cpu_dir.join("cache")).unwrap();
    let cache_paths = cache_dir
        .flatten()
        .filter(|x| x.path().is_dir())
        .map(|x| x.path());

    let mut caches: Vec<CpuCache> = vec![];

    for cache_path in cache_paths {
        let type_string = fs::read_to_string(cache_path.join("type")).unwrap();

        let c_type = match type_string.trim() {
            "Unified" => CacheType::Unified,
            "Data" => CacheType::Data,
            "Instruction" => CacheType::Instruction,
            _ => panic!("Unrecognized cache type: {type_string}"),
        };

        let c_level = fs::read_to_string(cache_path.join("level"))
            .map(|s| s.trim().parse::<usize>().unwrap())
            .unwrap();

        let size_string = fs::read_to_string(cache_path.join("size")).unwrap();
        let c_size = CacheSize::parse(size_string.trim());

        let shared_cpu_map = fs::read_to_string(cache_path.join("shared_cpu_map"))
            .unwrap()
            .trim()
            .to_string();

        caches.push(CpuCache {
            level: c_level,
            size: c_size,
            typ: c_type,
            shared_cpu_map,
        });
    }

    caches
}

pub fn read_freq_boost_state() -> Option<bool> {
    fs::read_to_string("/sys/devices/system/cpu/cpufreq/boost")
        .map(|content| content.trim() == "1")
        .ok()
}

pub fn read_cpu_vulnerabilities() -> Vec<CpuVulnerability> {
    let mut out: Vec<CpuVulnerability> = vec![];

    if let Ok(dir) = fs::read_dir("/sys/devices/system/cpu/vulnerabilities") {
        let mut files: Vec<_> = dir
            .flatten()
            .map(|x| x.path())
            .filter(|x| !x.is_dir())
            .collect();

        files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        for file in files {
            if let Ok(content) = fs::read_to_string(&file) {
                let name = file.file_name().unwrap().to_str().unwrap();

                out.push(CpuVulnerability {
                    name: (name[..1].to_uppercase() + &name[1..]).replace("_", " "),
                    mitigation: content.trim().to_string(),
                });
            }
        }
    };

    out
}

pub fn read_cpu_byte_order() -> Option<&'static str> {
    if let Ok(byte_order) = fs::read_to_string("/sys/kernel/cpu_byteorder") {
        match byte_order.trim() {
            "big" => return Some("Big Endian"),
            "little" => return Some("Little Endian"),
            _ => eprintln!("Unrecognised Byte Order: {byte_order}"),
        }
    }
    None
}

// Takes in a human-readable list of CPUs, and returns a list of indices parsed from that list
// These can come in the form of a plain range like `X-Y`, or a comma-separated ranges and indices ie. `1,3-4,7-8,10`
// Kernel docs with examples: https://www.kernel.org/doc/html/latest/admin-guide/cputopology.html
fn parse_cpu_list(list: &str) -> Vec<usize> {
    let mut out: Vec<usize> = vec![];

    if list.is_empty() {
        return out;
    }

    for part in list.trim().split(",") {
        if part.contains("-") {
            let bounds: Vec<_> = part.split("-").flat_map(|x| x.parse::<usize>()).collect();
            assert_eq!(bounds.len(), 2);
            for idx in bounds[0]..bounds[1] + 1 {
                out.push(idx)
            }
        } else {
            let idx = part.parse::<usize>().expect("Invalid CPU index value");
            out.push(idx);
        }
    }

    out
}

#[test]
fn test_parse_cache_size() {
    assert_eq!(CacheSize::parse("512").size_bytes(), 512);
    assert_eq!(CacheSize::parse("1K").size_bytes(), 1024);
    assert_eq!(CacheSize::parse("1M").size_bytes(), 1024 * 1024);
    assert_eq!(CacheSize::parse("1G").size_bytes(), 1024 * 1024 * 1024);
    assert_eq!(
        CacheSize::parse("1T").size_bytes(),
        1024 * 1024 * 1024 * 1024
    );
    assert_eq!(CacheSize::parse("123K").size_bytes(), 123 * 1024);
    assert_eq!(CacheSize::parse("32M").size_bytes(), 32 * 1024 * 1024);
    assert_eq!(
        CacheSize::parse("345G").size_bytes(),
        345 * 1024 * 1024 * 1024
    );
}

#[test]
fn test_print_cache_size() {
    assert_eq!(CacheSize::new(1023).human_readable(), "1023 B");
    assert_eq!(CacheSize::new(1024).human_readable(), "1 KiB");
    assert_eq!(CacheSize::new(1024 * 1024).human_readable(), "1 MiB");
    assert_eq!(CacheSize::new(1024 * 1024 * 1024).human_readable(), "1 GiB");

    assert_eq!(CacheSize::new(1023).raw(), "1023");
    assert_eq!(CacheSize::new(1024 * 1024).raw(), "1048576");
    assert_eq!(CacheSize::new(1024 * 1024 * 1024).raw(), "1073741824");

    assert_eq!(CacheSize::new(3 * 1024).human_readable(), "3 KiB");
    assert_eq!(
        CacheSize::new((7.6 * 1024.0 * 1024.0) as u64).human_readable(),
        "7 MiB"
    );
}

#[test]
fn test_parse_cpu_list() {
    assert_eq!(parse_cpu_list(""), Vec::<usize>::new());
    assert_eq!(parse_cpu_list("1-3"), Vec::<usize>::from([1, 2, 3]));
    assert_eq!(parse_cpu_list("1,2,3"), Vec::<usize>::from([1, 2, 3]));
    assert_eq!(
        parse_cpu_list("1,3-6,8"),
        Vec::<usize>::from([1, 3, 4, 5, 6, 8])
    );
    assert_eq!(
        parse_cpu_list("1-2,3-5,7"),
        Vec::<usize>::from([1, 2, 3, 4, 5, 7])
    );
}
