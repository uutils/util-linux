// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

use std::fs;
use std::io;
use std::path::Path;

const ABOUT: &str = help_about!("lsmem.md");
const USAGE: &str = help_usage!("lsmem.md");

const PATH_SYS_MEMORY: &str = "/sys/devices/system/memory";

#[repr(u8)]
enum ZoneId {
    ZoneDma = 0,
    ZoneDma32,
    ZoneNormal,
    ZoneHighmem,
    ZoneMovable,
    ZoneDevice,
    ZoneNone,
    ZoneUnknown,
    MaxNrZones,
}

struct MemoryBlock {
    index: u64,
    count: u64,
    state: i32,
    node: i32,
    nr_zones: i32,
    zones: [i32; ZoneId::MaxNrZones as usize],
    removable: u8,
}

fn get_blocks() -> Vec<MemoryBlock> {
    let mut blocks = Vec::<MemoryBlock>::new();

    list_files_and_folders(PATH_SYS_MEMORY).unwrap();

    return blocks;
}

fn list_files_and_folders<P: AsRef<Path>>(path: P) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            println!("Directory: {:?}", path);
        } else {
            println!("File: {:?}", path);
        }
    }
    Ok(())
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    get_blocks();
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
