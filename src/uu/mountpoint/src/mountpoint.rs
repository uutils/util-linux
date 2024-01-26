// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Command};
use std::env;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::process;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("mountpoint.md");
const USAGE: &str = help_usage!("mountpoint.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: mountpoint <path>");
        process::exit(1);
    }

    let path = &args[1];
    if is_mountpoint(path) {
        println!("{} is a mountpoint", path);
    } else {
        println!("{} is not a mountpoint", path);
    }
    Ok(())
}

fn is_mountpoint(path: &str) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    let dev = metadata.dev();
    let inode = metadata.ino();

    // Root inode (typically 2 in most Unix filesystems) indicates a mount point
    inode == 2
        || match fs::metadata("..") {
            Ok(parent_metadata) => parent_metadata.dev() != dev,
            Err(_) => false,
        }
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
