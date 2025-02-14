use parse_size::parse_size;
use std::{fs, path::PathBuf};

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
    pub _pkg_id: usize,
    pub caches: Vec<CpuCache>,
}

#[derive(Debug)]
pub struct CpuCache {
    pub typ: CacheType,
    pub level: usize,
    pub size: u64,
    pub shared_cpu_map: String,
}

#[derive(Debug)]
pub enum CacheType {
    Data,
    Instruction,
    Unified,
}

// TODO: respect `--hex` option and output the bitmask instead of human-readable range
pub fn read_online_cpus() -> String {
    fs::read_to_string("/sys/devices/system/cpu/online")
        .expect("Could not read sysfs")
        .trim()
        .to_string()
}

// Takes in a human-readable list of CPUs, and returns a list of indices parsed from that list
// These can come in the form of a plain range like `X-Y`, or a comma-separated ranges and indices ie. `1,3-4,7-8,10`
// Kernel docs with examples: https://www.kernel.org/doc/html/latest/admin-guide/cputopology.html
fn parse_cpu_list(list: &str) -> Vec<usize> {
    let mut out: Vec<usize> = vec![];

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

pub fn read_cpu_topology() -> CpuTopology {
    let mut out: Vec<Cpu> = vec![];

    let online_cpus = parse_cpu_list(&read_online_cpus());

    for cpu_index in online_cpus {
        let cpu_dir = PathBuf::from(format!("/sys/devices/system/cpu/cpu{}/", cpu_index));

        let physical_pkg_id = fs::read_to_string(cpu_dir.join("topology/physical_package_id"))
            .unwrap()
            .trim()
            .parse::<usize>()
            .unwrap();

        let caches = read_cpu_caches(cpu_index);

        out.push(Cpu {
            _index: cpu_index,
            _pkg_id: physical_pkg_id,
            caches,
        })
    }

    CpuTopology { cpus: out }
}

fn read_cpu_caches(cpu_index: usize) -> Vec<CpuCache> {
    let cpu_dir = PathBuf::from(format!("/sys/devices/system/cpu/cpu{}/", cpu_index));
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
            _ => panic!("Unrecognized cache type: {}", type_string),
        };

        let c_level = fs::read_to_string(cache_path.join("level"))
            .map(|s| s.trim().parse::<usize>().unwrap())
            .unwrap();

        let size_string = fs::read_to_string(cache_path.join("size")).unwrap();
        let c_size = parse_size(size_string.trim()).unwrap();

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
    match fs::read_to_string("/sys/devices/system/cpu/cpufreq/boost") {
        Ok(content) => Some(content.trim() == "1"),
        Err(_) => None,
    }
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
            _ => eprintln!("Unrecognised Byte Order: {}", byte_order),
        }
    }
    None
}
