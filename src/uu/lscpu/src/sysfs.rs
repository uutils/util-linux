// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use uucore::{
    error::{UResult, USimpleError},
    parser::parse_size,
};

/// `Path::join` discards the root when the joined path is absolute, so strip
/// the leading separator before joining.
pub fn system_path(root: &Path, path: &str) -> PathBuf {
    root.join(path.trim_start_matches('/'))
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|contents| contents.trim().to_string())
}

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
    // i64 rather than usize: the kernel reports -1 when the architecture
    // does not expose physical package information
    pub pkg_id: i64,
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
    pub fn new(root: &Path) -> UResult<Self> {
        let mut out: Vec<Cpu> = vec![];

        let online_cpus = parse_cpu_list(&read_online_cpus(root)?)?;

        for cpu_index in online_cpus {
            let cpu_dir = system_path(root, &format!("/sys/devices/system/cpu/cpu{cpu_index}/"));

            // Keep reporting partial topology when sysfs is incomplete: -1
            // means an unknown package, and a CPU without core_id falls back
            // to its CPU index instead of failing the whole command.
            let pkg_id = read_trimmed(&cpu_dir.join("topology/physical_package_id"))
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(-1);

            let core_id = read_trimmed(&cpu_dir.join("topology/core_id"))
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(cpu_index);

            let caches = read_cpu_caches(root, cpu_index);

            out.push(Cpu {
                _index: cpu_index,
                pkg_id,
                core_id,
                caches,
            })
        }
        Ok(Self { cpus: out })
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

    fn parse(s: &str) -> Option<Self> {
        parse_size::parse_size_u64(s).ok().map(Self)
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
pub fn read_online_cpus(root: &Path) -> UResult<String> {
    let path = system_path(root, "/sys/devices/system/cpu/online");

    fs::read_to_string(&path)
        .map(|contents| contents.trim().to_string())
        .map_err(|err| USimpleError::new(1, format!("Could not read {}: {err}", path.display())))
}

fn read_cpu_caches(root: &Path, cpu_index: usize) -> Vec<CpuCache> {
    let cpu_dir = system_path(root, &format!("/sys/devices/system/cpu/cpu{cpu_index}/"));
    let Ok(cache_dir) = fs::read_dir(cpu_dir.join("cache")) else {
        return Vec::new();
    };
    let cache_paths = cache_dir
        .flatten()
        .filter(|x| x.path().is_dir())
        .map(|x| x.path());

    let mut caches: Vec<CpuCache> = vec![];

    for cache_path in cache_paths {
        // Ignore entries with unknown or missing metadata rather than failing
        // the command; new kernel cache types should not break lscpu.
        let Some(c_type) =
            read_trimmed(&cache_path.join("type")).and_then(|value| match value.as_str() {
                "Unified" => Some(CacheType::Unified),
                "Data" => Some(CacheType::Data),
                "Instruction" => Some(CacheType::Instruction),
                _ => None,
            })
        else {
            continue;
        };

        let Some(c_level) =
            read_trimmed(&cache_path.join("level")).and_then(|value| value.parse::<usize>().ok())
        else {
            continue;
        };

        let Some(c_size) =
            read_trimmed(&cache_path.join("size")).and_then(|value| CacheSize::parse(&value))
        else {
            continue;
        };

        let Some(shared_cpu_map) = read_trimmed(&cache_path.join("shared_cpu_map")) else {
            continue;
        };

        caches.push(CpuCache {
            level: c_level,
            size: c_size,
            typ: c_type,
            shared_cpu_map,
        });
    }

    caches
}

pub fn read_freq_boost_state(root: &Path) -> Option<bool> {
    fs::read_to_string(system_path(root, "/sys/devices/system/cpu/cpufreq/boost"))
        .map(|content| content.trim() == "1")
        .ok()
}

pub fn read_cpu_vulnerabilities(root: &Path) -> Vec<CpuVulnerability> {
    let mut out: Vec<CpuVulnerability> = vec![];

    if let Ok(dir) = fs::read_dir(system_path(root, "/sys/devices/system/cpu/vulnerabilities")) {
        let mut files: Vec<_> = dir
            .flatten()
            .map(|x| x.path())
            .filter(|x| !x.is_dir())
            .collect();

        files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        for file in files {
            if let Ok(content) = fs::read_to_string(&file) {
                let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };

                out.push(CpuVulnerability {
                    name: vulnerability_name(name),
                    mitigation: content.trim().to_string(),
                });
            }
        }
    };

    out
}

pub fn read_cpu_byte_order(root: &Path) -> Option<&'static str> {
    if let Ok(byte_order) = fs::read_to_string(system_path(root, "/sys/kernel/cpu_byteorder")) {
        match byte_order.trim() {
            "big" => return Some("Big Endian"),
            "little" => return Some("Little Endian"),
            _ => eprintln!("Unrecognised Byte Order: {byte_order}"),
        }
    }
    None
}

fn vulnerability_name(file_name: &str) -> String {
    let mut chars = file_name.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    let mut name = first.to_uppercase().collect::<String>();
    name.push_str(chars.as_str());
    name.replace('_', " ")
}

fn parse_cpu_index(value: &str) -> UResult<usize> {
    value
        .trim()
        .parse::<usize>()
        .map_err(|_| USimpleError::new(1, format!("Invalid CPU index value: {value}")))
}

// Takes in a human-readable list of CPUs, and returns a list of indices parsed from that list
// These can come in the form of a plain range like `X-Y`, or a comma-separated ranges and indices ie. `1,3-4,7-8,10`
// Kernel docs with examples: https://www.kernel.org/doc/html/latest/admin-guide/cputopology.html
fn parse_cpu_list(list: &str) -> UResult<Vec<usize>> {
    let mut out: Vec<usize> = vec![];
    let list = list.trim();

    if list.is_empty() {
        return Ok(out);
    }

    for part in list.split(',') {
        let part = part.trim();

        if part.is_empty() {
            return Err(USimpleError::new(1, "Invalid empty CPU list entry"));
        }

        if let Some((start, end)) = part.split_once('-') {
            let start = parse_cpu_index(start)?;
            let end = parse_cpu_index(end)?;

            if start > end {
                return Err(USimpleError::new(1, format!("Invalid CPU range: {part}")));
            }

            out.extend(start..=end);
        } else {
            out.push(parse_cpu_index(part)?);
        }
    }

    Ok(out)
}

#[test]
fn test_parse_cache_size() {
    assert_eq!(CacheSize::parse("512").unwrap().size_bytes(), 512);
    assert_eq!(CacheSize::parse("1K").unwrap().size_bytes(), 1024);
    assert_eq!(CacheSize::parse("1M").unwrap().size_bytes(), 1024 * 1024);
    assert_eq!(
        CacheSize::parse("1G").unwrap().size_bytes(),
        1024 * 1024 * 1024
    );
    assert_eq!(
        CacheSize::parse("1T").unwrap().size_bytes(),
        1024 * 1024 * 1024 * 1024
    );
    assert_eq!(CacheSize::parse("123K").unwrap().size_bytes(), 123 * 1024);
    assert_eq!(
        CacheSize::parse("32M").unwrap().size_bytes(),
        32 * 1024 * 1024
    );
    assert_eq!(
        CacheSize::parse("345G").unwrap().size_bytes(),
        345 * 1024 * 1024 * 1024
    );

    assert!(CacheSize::parse("not-a-size").is_none());
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
fn test_socket_count_with_unknown_package_id() {
    // The kernel reports physical_package_id as -1 when the architecture does
    // not expose package information (e.g. some ppc64 machines, see #495).
    // All-unknown ids collapse into a single socket bucket.
    let topology = CpuTopology {
        cpus: vec![
            Cpu {
                _index: 0,
                pkg_id: -1,
                core_id: 0,
                caches: vec![],
            },
            Cpu {
                _index: 1,
                pkg_id: -1,
                core_id: 1,
                caches: vec![],
            },
        ],
    };
    assert_eq!(topology.socket_count(), 1);
    assert_eq!(topology.core_count(), 2);
}

#[test]
fn test_vulnerability_name() {
    assert_eq!(vulnerability_name("spectre_v1"), "Spectre v1");
    assert_eq!(vulnerability_name("meltdown"), "Meltdown");
    assert_eq!(vulnerability_name(""), "");
}

#[test]
fn test_parse_cpu_list() {
    assert_eq!(parse_cpu_list("").unwrap(), Vec::<usize>::new());
    assert_eq!(
        parse_cpu_list("1-3").unwrap(),
        Vec::<usize>::from([1, 2, 3])
    );
    assert_eq!(
        parse_cpu_list("1,2,3").unwrap(),
        Vec::<usize>::from([1, 2, 3])
    );
    assert_eq!(
        parse_cpu_list("1,3-6,8").unwrap(),
        Vec::<usize>::from([1, 3, 4, 5, 6, 8])
    );
    assert_eq!(
        parse_cpu_list("1-2,3-5,7").unwrap(),
        Vec::<usize>::from([1, 2, 3, 4, 5, 7])
    );

    assert!(parse_cpu_list("1-").is_err());
    assert!(parse_cpu_list("a").is_err());
    assert!(parse_cpu_list("3-1").is_err());
    assert!(parse_cpu_list("1,,2").is_err());
}
