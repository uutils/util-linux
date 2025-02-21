// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{fmt, fs, str::FromStr};

use clap::{crate_version, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

// See https://www.man7.org/linux/man-pages/man5/proc_locks.5.html for details on each field's meaning
#[derive(Debug)]
struct Lock {
    _ord: usize,
    lock_type: LockType,
    mandatory: bool,
    mode: LockMode,
    pid: Option<usize>, // This value is -1 for OFD locks, hence the Option
    major_minor: String,
    inode: usize,
    start_offset: usize,       // Byte offset to start of lock
    end_offset: Option<usize>, // None = lock does not have an explicit end offset and applies until the end of the file
}

impl Lock {
    fn get_value(&self, col: &Column) -> String {
        match col {
            Column::Command => resolve_command(self).unwrap_or("<unknown>".to_string()),
            Column::Pid => self
                .pid
                .map(|pid| pid.to_string())
                .unwrap_or("-".to_string()),
            Column::Type => self.lock_type.to_string(),
            Column::Size => todo!(),
            Column::Inode => self.inode.to_string(),
            Column::MajorMinor => self.major_minor.clone(),
            Column::Mode => self.mode.to_string(),
            Column::Mandatory => {
                if self.mandatory {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            Column::Start => self.start_offset.to_string(),
            // TODO: In the case of EOF end_offset, we should actually resolve the actual file size and display that as the end offset
            Column::End => self
                .end_offset
                .map(|offset| offset.to_string())
                .unwrap_or("EOF".to_string()),
            Column::Path => todo!(), // TODO: Resolve filepath of the lock target
            Column::Blocker => todo!(), // TODO: Check if lock is blocker (and by what)
            Column::Holders => todo!(), // TODO: Resolve all holders of the lock (this would also let us display a process/PID for OFD locks)
        }
    }
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
        let mandatory = parts
            .next()
            .map(|part| match part {
                "MANDATORY" => true,
                "ADVISORY" => false,
                _ => panic!("Unrecognized value in lock line: {}", part),
            })
            .unwrap();
        let mode = parts
            .next()
            .and_then(|part| LockMode::from_str(part).ok())
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
        let major_minor_inode: Vec<_> = parts.next().unwrap().split(":").collect();
        assert_eq!(major_minor_inode.len(), 3);
        let major_minor = [major_minor_inode[0], major_minor_inode[1]].join(":");
        let inode = major_minor_inode[2].parse::<usize>().unwrap();

        let start_offset = parts.next().unwrap().parse::<usize>().unwrap();
        let end_offset: Option<usize> = parts.next().and_then(|offset_str| match offset_str {
            "EOF" => None,
            other => other.parse::<usize>().ok(),
        });

        Ok(Self {
            _ord: ord,
            lock_type,
            mandatory,
            mode,
            pid,
            major_minor,
            inode,
            start_offset,
            end_offset,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
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

impl fmt::Display for LockType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockType::FLOCK => write!(f, "FLOCK"),
            LockType::OFDLCK => write!(f, "OFDLCK"),
            LockType::POSIX => write!(f, "POSIX"),
        }
    }
}

#[derive(Debug)]
enum LockMode {
    Read,
    Write,
}

impl FromStr for LockMode {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "WRITE" => Ok(Self::Write),
            "READ" => Ok(Self::Read),
            _ => Err(()),
        }
    }
}

impl fmt::Display for LockMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockMode::Write => write!(f, "WRITE"),
            LockMode::Read => write!(f, "READ"),
        }
    }
}

// All of the columns that need to be supported in the final version
#[derive(Clone)]
#[allow(dead_code)]
enum Column {
    Command,
    Pid,
    Type,
    Size,
    Inode,
    MajorMinor,
    Mode,
    Mandatory,
    Start,
    End,
    Path,
    Blocker,
    Holders,
}

impl Column {
    fn header_text(&self) -> &'static str {
        match self {
            Self::Command => "COMMAND",
            Self::Pid => "PID",
            Self::Type => "TYPE",
            Self::Size => "SIZE",
            Self::Inode => "INODE",
            Self::MajorMinor => "MAJ:MIN",
            Self::Mode => "MODE",
            Self::Mandatory => "M",
            Self::Start => "START",
            Self::End => "END",
            Self::Path => "PATH",
            Self::Blocker => "BLOCKER",
            Self::Holders => "HOLDERS",
        }
    }
}

fn resolve_command(lock: &Lock) -> Option<String> {
    if let Some(pid) = lock.pid {
        return fs::read_to_string(format!("/proc/{}/comm", pid))
            .map(|content| content.trim().to_string())
            .ok();
    }

    // File descriptor locks don't have a real notion of an "owner process", since it can be shared by multiple processes.
    // The original `lslocks` goes through `/proc/<pid>/fdinfo/*` to find *any* of the processes that reference the same device/inode combo from the lock
    // We don't implement this behaviour yet, and just show "unknown" for the locks which don't have a clear owner
    None
}

const DEFAULT_COLS: &[Column] = &[
    Column::Command,
    Column::Pid,
    Column::Type,
    //TODO: Implement Column::Size here
    Column::Mode,
    Column::Mandatory,
    Column::Start,
    Column::End,
    //TODO: Implements Column::Path here
];

struct OutputOptions {
    cols: Vec<Column>,
}

fn print_output(locks: Vec<Lock>, output_opts: OutputOptions) {
    let mut column_widths: Vec<_> = output_opts
        .cols
        .iter()
        .map(|col| col.header_text().len())
        .collect();

    for lock in &locks {
        for (i, col) in output_opts.cols.iter().enumerate() {
            column_widths[i] = column_widths[i].max(lock.get_value(col).len());
        }
    }

    let headers: Vec<_> = output_opts
        .cols
        .iter()
        .enumerate()
        .map(|(i, col)| format!("{:<width$}", col.header_text(), width = column_widths[i]))
        .collect();

    println!("{}", headers.join(" "));

    for lock in &locks {
        let values: Vec<_> = output_opts
            .cols
            .iter()
            .enumerate()
            .map(|(i, col)| format!("{:<width$}", lock.get_value(col), width = column_widths[i]))
            .collect();
        println!("{}", values.join(" "));
    }
}

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    let output_opts = OutputOptions {
        cols: Vec::from(DEFAULT_COLS),
    };

    let locks: Vec<_> = match fs::read_to_string("/proc/locks") {
        Ok(content) => content
            .lines()
            .map(|line| Lock::from_str(line).unwrap())
            .collect(),
        Err(e) => panic!("Could not read /proc/locks: {}", e),
    };

    print_output(locks, output_opts);
    Ok(())
}

const ABOUT: &str = help_about!("lslocks.md");
const USAGE: &str = help_usage!("lslocks.md");

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
