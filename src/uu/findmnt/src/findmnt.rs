// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::fs;

use clap::{crate_version, Command};
use uucore::error::UResult;

#[derive(Debug)]
struct Mount {
    target: String,
    source: String,
    fs_type: String,
    options: String,
}

impl Mount {
    // Parses a line from `/proc/mounts`, which follows the format described under fstab(5)
    // Each line contains six space-separated fields, last two being unused in `/proc/mounts`
    fn parse(input: &str) -> Self {
        let parts: Vec<_> = input.trim().split(" ").collect();
        assert_eq!(parts.len(), 6);

        let source = parts[0].to_string();
        let target = parts[1].to_string();
        let fs_type = parts[2].to_string();
        let options = parts[3].to_string();
        //Ignore fields 5 and 6 as they are not used in /proc/mounts and are only populated for compatibility purposes

        Self {
            source,
            target,
            fs_type,
            options,
        }
    }
}

fn read_mounts() -> Vec<Mount> {
    let content = fs::read_to_string("/proc/mounts").expect("Could not read /proc/mounts");
    let mounts: Vec<_> = content.lines().map(Mount::parse).collect();
    mounts
}

fn print_output(mounts: Vec<Mount>) {
    for mount in mounts {
        println!(
            "{}\t{}\t{}\t{}",
            mount.target, mount.source, mount.fs_type, mount.options
        )
    }
}

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    let mounts = read_mounts();

    print_output(mounts);

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name()).version(crate_version!())
}
