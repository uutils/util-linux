use std::fs;

pub struct CpuVulnerability {
    pub name: String,
    pub mitigation: String,
}

pub struct CpuTopology {
    pub cpus: Vec<Cpu>,
}

pub struct Cpu {
    _index: usize,
    _caches: Vec<CpuCache>,
}

pub struct CpuCache {
    _index: usize,
    _typ: String,
    _level: String,
    _size: String,
}

// TODO: This should go through each CPU in sysfs and calculate things such as cache sizes and physical topology
// For now it just returns a list of CPUs which are enabled
pub fn read_cpu_topology() -> CpuTopology {
    let mut out: Vec<Cpu> = vec![];

    // NOTE: All examples I could find was where this file contains a CPU index range in the form of `<start>-<end>`
    // Theoretically, there might be a situation where some cores are disabled, so that `enabled` cannot be represented
    // as a continuous range. For now we just assume it's always `X-Y` and use those as our bounds to read CPU information
    let enabled_cpus = match fs::read_to_string("/sys/devices/system/cpu/enabled") {
        Ok(content) => {
            let parts: Vec<_> = content
                .trim()
                .split("-")
                .flat_map(|part| part.parse::<usize>())
                .collect();
            assert_eq!(parts.len(), 2);
            (parts[0], parts[1])
        }
        Err(e) => panic!("Could not read sysfs: {}", e),
    };

    for cpu_index in enabled_cpus.0..(enabled_cpus.1 + 1) {
        out.push(Cpu {
            _index: cpu_index,
            _caches: vec![],
        })
    }

    CpuTopology { cpus: out }
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
