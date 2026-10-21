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
use uucore::error::{set_exit_code, strip_errno, UResult, USimpleError};
use uucore::{format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("mountpoint.md");
const USAGE: &str = help_usage!("mountpoint.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let path = matches.get_one::<String>("path");

    if let Some(path) = path {
        match is_mountpoint(path) {
            Ok(true) => println!("{path} is a mountpoint"),
            Ok(false) => {
                println!("{path} is not a mountpoint");
                set_exit_code(32);
            }
            Err(e) => return Err(USimpleError::new(1, format!("{path}: {}", strip_errno(&e)))),
        }
    } else {
        // Handle the case where path is not provided
        eprintln!("Error: Path argument is required");
        process::exit(1);
    }
    Ok(())
}

/// A path is a mount point when the kernel lists it in mountinfo, which also
/// covers bind mounts on the same device and mounts on files. Without /proc
/// (a chroot, early boot) it falls back to comparing with the parent.
#[cfg(target_os = "linux")]
fn is_mountpoint(path: &str) -> std::io::Result<bool> {
    use std::os::unix::ffi::OsStrExt;

    let target = fs::canonicalize(path)?;
    let Ok(mountinfo) = fs::read("/proc/self/mountinfo") else {
        return is_mountpoint_by_device(path);
    };
    Ok(mountinfo
        .split(|&b| b == b'\n')
        .filter_map(|line| line.split(|&b| b == b' ').nth(4))
        .any(|mount_point| unescape_mountinfo(mount_point) == target.as_os_str().as_bytes()))
}

/// mountinfo writes space, tab, newline and backslash as `\ooo` octal escapes.
#[cfg(target_os = "linux")]
fn unescape_mountinfo(field: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.len());
    let mut i = 0;
    while i < field.len() {
        let octal = field
            .get(i + 1..i + 4)
            .filter(|d| d.iter().all(|c| (b'0'..=b'7').contains(c)));
        match (field[i], octal) {
            (b'\\', Some(d)) => {
                out.push((d[0] - b'0') * 64 + (d[1] - b'0') * 8 + (d[2] - b'0'));
                i += 4;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn is_mountpoint(path: &str) -> std::io::Result<bool> {
    is_mountpoint_by_device(path)
}

#[cfg(not(windows))]
fn is_mountpoint_by_device(path: &str) -> std::io::Result<bool> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_dir() {
        return Ok(false);
    }

    // A mount point sits on another device than its parent; `/` is its own parent
    let parent = fs::metadata(std::path::Path::new(path).join(".."))?;
    Ok(parent.dev() != metadata.dev() || parent.ino() == metadata.ino())
}

// TODO: implement for windows
#[cfg(windows)]
fn is_mountpoint(_path: &str) -> std::io::Result<bool> {
    Ok(false)
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

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::unescape_mountinfo;

    #[test]
    fn test_unescape_mountinfo() {
        assert_eq!(unescape_mountinfo(br"/mnt/a\040b"), b"/mnt/a b");
        assert_eq!(unescape_mountinfo(br"/mnt/a\134b"), br"/mnt/a\b");
        assert_eq!(unescape_mountinfo(br"/mnt/a\"), br"/mnt/a\");
        assert_eq!(unescape_mountinfo(br"/mnt/a\04"), br"/mnt/a\04");
        assert_eq!(unescape_mountinfo(br"/mnt/a\9b"), br"/mnt/a\9b");
    }
}
