// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::Arg;
use clap::{crate_version, Command};
use std::env;
#[cfg(not(windows))]
use std::fs;
#[cfg(not(windows))]
use std::os::unix::fs::MetadataExt;
use std::process;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("mountpoint.md");
const USAGE: &str = help_usage!("mountpoint.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let path = matches.get_one::<String>("path");

    if let Some(path) = path {
        if is_mountpoint(path) {
            println!("{} is a mountpoint", path);
        } else {
            println!("{} is not a mountpoint", path);
        }
    } else {
        // Handle the case where path is not provided
        eprintln!("Error: Path argument is required");
        process::exit(1);
    }
    Ok(())
}

#[cfg(not(windows))]
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

// TODO: implement for windows
#[cfg(windows)]
fn is_mountpoint(_path: &str) -> bool {
    false
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("path")
                .value_name("PATH")
                .help("Path to check for mountpoint")
                .required(true)
                .index(1),
        )
}
