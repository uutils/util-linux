use std::fs;

pub struct CpuVulnerability {
    pub name: String,
    pub mitigation: String,
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
