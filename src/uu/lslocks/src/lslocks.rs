// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{fs, str::FromStr};

use clap::{crate_version, Command};
use uucore::error::UResult;

// See https://www.man7.org/linux/man-pages/man5/proc_locks.5.html for details on each field's meaning
#[derive(Debug)]
struct Lock {
    ord: usize,
    lock_type: LockType,
    strictness: Strictness,
    variant: Variant,
    pid: Option<usize>, // This value is -1 for OFD locks, hence the Option
    major_minor: String,
    inode: usize,
    start_offset: usize,       // Byte offset to start of lock
    end_offset: Option<usize>, // None = lock does not have an explicit end offset and applies until the end of the file
}

impl FromStr for Lock {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut parts = input.split_whitespace();

        // Ordinal position comes in the form of `<value>:`, so we need to strip away the `:`
        let ord = parts
            .next()
            .and_then(|s| s.strip_suffix(":"))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let lock_type = parts
            .next()
            .and_then(|part| LockType::from_str(part).ok())
            .unwrap();
        let strictness = parts
            .next()
            .and_then(|part| Strictness::from_str(part).ok())
            .unwrap();
        let variant = parts
            .next()
            .and_then(|part| Variant::from_str(part).ok())
            .unwrap();
        let pid: Option<usize> = parts.next().and_then(|pid_str| match pid_str {
            "-1" => None,
            other => other.parse::<usize>().ok(),
        });

        if lock_type == LockType::OFDLCK && pid.is_some() {
            println!("Unexpected PID value on OFD lock: '{}'", input);
            return Err(());
        };

        // This field has a format of MAJOR:MINOR:INODE
        let maj_min_inode: Vec<_> = parts.next().unwrap().split(":").collect();
        assert_eq!(maj_min_inode.len(), 3);
        let major_minor = [maj_min_inode[0], maj_min_inode[1]].join(":");
        let inode = maj_min_inode[2].parse::<usize>().unwrap();

        let start_offset = parts.next().unwrap().parse::<usize>().unwrap();
        let end_offset: Option<usize> = parts.next().and_then(|offset_str| match offset_str {
            "EOF" => None,
            other => other.parse::<usize>().ok(),
        });

        Ok(Self {
            ord,
            lock_type,
            strictness,
            variant,
            pid,
            major_minor,
            inode,
            start_offset,
            end_offset,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LockType {
    FLOCK,  // BSD file lock
    OFDLCK, // Open file descriptor
    POSIX,  // POSIX byte-range lock
}

impl FromStr for LockType {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "FLOCK" => Ok(Self::FLOCK),
            "OFDLCK" => Ok(Self::OFDLCK),
            "POSIX" => Ok(Self::POSIX),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
enum Strictness {
    Advisory,
    Mandatory,
}

impl FromStr for Strictness {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "ADVISORY" => Ok(Self::Advisory),
            "MANDATORY" => Ok(Self::Mandatory),
            _ => Err(()),
        }
    }
}

#[derive(Debug)]
enum Variant {
    Read,
    Write,
}

impl FromStr for Variant {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "WRITE" => Ok(Self::Write),
            "READ" => Ok(Self::Read),
            _ => Err(()),
        }
    }
}

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    let locks: Vec<_> = match fs::read_to_string("/proc/locks") {
        Ok(content) => content
            .lines()
            .map(|line| Lock::from_str(line).unwrap())
            .collect(),
        Err(e) => panic!("Could not read /proc/locks: {}", e),
    };

    for lock in locks {
        println!("{:?}", lock);
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        //.about(ABOUT)
        //.override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
