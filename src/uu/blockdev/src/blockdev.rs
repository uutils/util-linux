// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, value_parser, Arg, ArgAction, Command};
use linux_raw_sys::ioctl::*;
#[cfg(target_os = "linux")]
use std::collections::BTreeMap;
#[cfg(target_os = "linux")]
use uucore::error::USimpleError;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("blockdev.md");
const USAGE: &str = help_usage!("blockdev.md");

#[derive(Copy, Clone, Debug)]
enum IoctlArgType {
    Short,
    Int,
    Long,
    U64Sectors,
    U64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum IoctlCommand {
    GetAttribute(IoctlArgType),
    SetAttribute,
    Operation(u32),
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
enum BlockdevCommand {
    SetVerbosity(bool),
    Ioctl(&'static str, u32, IoctlCommand),
}

const BLOCKDEV_ACTIONS: &[(&str, BlockdevCommand)] = &[
    ("verbose", BlockdevCommand::SetVerbosity(true)),
    ("quiet", BlockdevCommand::SetVerbosity(false)),
    (
        "flushbufs",
        BlockdevCommand::Ioctl("flush buffers", BLKFLSBUF, IoctlCommand::Operation(0)),
    ),
    (
        "getalignoff",
        BlockdevCommand::Ioctl(
            "get alignment offset in bytes",
            BLKALIGNOFF,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getbsz",
        BlockdevCommand::Ioctl(
            "get blocksize",
            BLKBSZGET,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getdiscardzeroes",
        BlockdevCommand::Ioctl(
            "get discard zeroes support status",
            BLKDISCARDZEROES,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getfra",
        BlockdevCommand::Ioctl(
            "get filesystem readahead",
            BLKFRAGET,
            IoctlCommand::GetAttribute(IoctlArgType::Long),
        ),
    ),
    (
        "getiomin",
        BlockdevCommand::Ioctl(
            "get minimum I/O size",
            BLKIOMIN,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getioopt",
        BlockdevCommand::Ioctl(
            "get optimal I/O size",
            BLKIOOPT,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getmaxsect",
        BlockdevCommand::Ioctl(
            "get max sectors per request",
            BLKSECTGET,
            IoctlCommand::GetAttribute(IoctlArgType::Short),
        ),
    ),
    (
        "getpbsz",
        BlockdevCommand::Ioctl(
            "get physical block (sector) size",
            BLKPBSZGET,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getra",
        BlockdevCommand::Ioctl(
            "get readahead",
            BLKRAGET,
            IoctlCommand::GetAttribute(IoctlArgType::Long),
        ),
    ),
    (
        "getro",
        BlockdevCommand::Ioctl(
            "get read-only",
            BLKROGET,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getsize64",
        BlockdevCommand::Ioctl(
            "get size in bytes",
            BLKGETSIZE64,
            IoctlCommand::GetAttribute(IoctlArgType::U64),
        ),
    ),
    (
        "getsize",
        BlockdevCommand::Ioctl(
            "get 32-bit sector count (deprecated, use --getsz)",
            BLKGETSIZE,
            IoctlCommand::GetAttribute(IoctlArgType::Long),
        ),
    ),
    (
        "getss",
        BlockdevCommand::Ioctl(
            "get logical block (sector) size",
            BLKSSZGET,
            IoctlCommand::GetAttribute(IoctlArgType::Int),
        ),
    ),
    (
        "getsz",
        BlockdevCommand::Ioctl(
            "get size in 512-byte sectors",
            BLKGETSIZE64,
            IoctlCommand::GetAttribute(IoctlArgType::U64Sectors),
        ),
    ),
    (
        "rereadpt",
        BlockdevCommand::Ioctl(
            "reread partition table",
            BLKRRPART,
            IoctlCommand::Operation(0),
        ),
    ),
    (
        "setbsz",
        BlockdevCommand::Ioctl("set blocksize", BLKBSZSET, IoctlCommand::SetAttribute),
    ),
    (
        "setfra",
        BlockdevCommand::Ioctl(
            "set filesystem readahead",
            BLKFRASET,
            IoctlCommand::SetAttribute,
        ),
    ),
    (
        "setra",
        BlockdevCommand::Ioctl("set readahead", BLKRASET, IoctlCommand::SetAttribute),
    ),
    (
        "setro",
        BlockdevCommand::Ioctl("set read-only", BLKROSET, IoctlCommand::Operation(1)),
    ),
    (
        "setrw",
        BlockdevCommand::Ioctl("set read-write", BLKROSET, IoctlCommand::Operation(0)),
    ),
];

#[cfg(target_os = "linux")]
mod linux {
    use crate::*;
    use std::{fs::File, io, os::fd::AsRawFd};
    use std::{io::Read, os::unix::fs::MetadataExt, path::Path};
    use uucore::{error::UIoError, libc};

    unsafe fn uu_ioctl<T>(device_file: &File, ioctl_code: u32, input: T) -> UResult<()> {
        if libc::ioctl(device_file.as_raw_fd(), ioctl_code.into(), input) < 0 {
            Err(Box::new(UIoError::from(io::Error::last_os_error())))
        } else {
            Ok(())
        }
    }

    unsafe fn get_ioctl_attribute(
        device_file: &File,
        ioctl_code: u32,
        ioctl_type: IoctlArgType,
    ) -> UResult<u64> {
        unsafe fn ioctl_get<T: Default + Into<u64>>(
            device: &File,
            ioctl_code: u32,
        ) -> UResult<u64> {
            let mut retval: T = Default::default();
            uu_ioctl(device, ioctl_code, &mut retval as *mut T as usize).map(|_| retval.into())
        }

        match ioctl_type {
            IoctlArgType::Int => ioctl_get::<libc::c_uint>(device_file, ioctl_code),
            IoctlArgType::Long => ioctl_get::<libc::c_ulong>(device_file, ioctl_code),
            IoctlArgType::Short => ioctl_get::<libc::c_ushort>(device_file, ioctl_code),
            IoctlArgType::U64 => ioctl_get::<u64>(device_file, ioctl_code),
            IoctlArgType::U64Sectors => Ok(ioctl_get::<u64>(device_file, ioctl_code)? / 512),
        }
    }

    fn get_partition_offset(device_file: &File) -> UResult<usize> {
        let rdev = device_file.metadata()?.rdev();
        let major = libc::major(rdev);
        let minor = libc::minor(rdev);
        if Path::new(&format!("/sys/dev/block/{major}:{minor}/partition")).exists() {
            let mut start_fd = File::open(format!("/sys/dev/block/{major}:{minor}/start"))?;
            let mut str = String::new();
            start_fd.read_to_string(&mut str)?;
            return str
                .trim()
                .parse()
                .map_err(|_| USimpleError::new(1, "Unable to parse partition start offset"));
        }
        Ok(0)
    }

    pub fn do_report(device_path: &str) -> UResult<()> {
        let device_file = File::open(device_path)?;
        let partition_offset = get_partition_offset(&device_file)?;
        let report_ioctls = &["getro", "getra", "getss", "getbsz", "getsize64"];
        let ioctl_values = report_ioctls
            .iter()
            .map(|flag| {
                let Some((
                    _,
                    BlockdevCommand::Ioctl(_, ioctl_code, IoctlCommand::GetAttribute(ioctl_type)),
                )) = BLOCKDEV_ACTIONS.iter().find(|(n, _)| flag == n)
                else {
                    unreachable!()
                };
                unsafe { get_ioctl_attribute(&device_file, *ioctl_code, *ioctl_type) }
            })
            .collect::<Result<Vec<u64>, _>>()?;
        println!(
            "{} {:5} {:5} {:5} {:15} {:15}   {}",
            if ioctl_values[0] == 1 { "ro" } else { "rw" },
            ioctl_values[1],
            ioctl_values[2],
            ioctl_values[3],
            partition_offset,
            ioctl_values[4],
            device_path
        );
        Ok(())
    }

    pub fn do_ioctl_command(
        device: &File,
        name: &str,
        ioctl_code: u32,
        ioctl_action: &IoctlCommand,
        verbose: bool,
        arg: usize,
    ) -> UResult<()> {
        match ioctl_action {
            IoctlCommand::GetAttribute(ioctl_type) => {
                let ret = unsafe { get_ioctl_attribute(device, ioctl_code, *ioctl_type)? };
                if verbose {
                    println!("{name}: {ret}");
                } else {
                    println!("{ret}");
                }
            }
            IoctlCommand::SetAttribute => {
                unsafe { uu_ioctl(device, ioctl_code, arg)? };
                if verbose {
                    println!("{name} succeeded.");
                }
            }
            IoctlCommand::Operation(param) => {
                unsafe { uu_ioctl(device, ioctl_code, param)? };
                if verbose {
                    println!("{name} succeeded.");
                }
            }
        };
        Ok(())
    }
}

#[cfg(target_os = "linux")]
use linux::*;

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    use std::fs::File;

    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let devices = matches
        .get_many::<String>("devices")
        .expect("Required command-line argument");

    if matches.get_flag("report") {
        println!("RO    RA   SSZ   BSZ        StartSec            Size   Device");
        for device_path in devices {
            uucore::show_if_err!(do_report(device_path));
        }
        Ok(())
    } else {
        // Recover arguments from clap in the same order they were passed
        // Based on https://docs.rs/clap/latest/clap/_cookbook/find/index.html
        let mut operations = BTreeMap::new();
        for (id, op) in BLOCKDEV_ACTIONS {
            if matches.value_source(id) != Some(clap::parser::ValueSource::CommandLine) {
                continue;
            }
            let indices = matches.indices_of(id).unwrap();
            let values = matches.get_many::<usize>(id).unwrap();
            for (index, value) in indices.zip(values) {
                operations.insert(index, (op.clone(), *value));
            }
        }

        for device_path in devices {
            let mut verbose = false;
            let device_file = File::open(device_path)?;
            for (operation, value) in operations.values() {
                match operation {
                    BlockdevCommand::SetVerbosity(true) => verbose = true,
                    BlockdevCommand::SetVerbosity(false) => verbose = false,
                    BlockdevCommand::Ioctl(description, ioctl_code, ioctl_action) => {
                        if let Err(e) = do_ioctl_command(
                            &device_file,
                            description,
                            *ioctl_code,
                            ioctl_action,
                            verbose,
                            *value,
                        ) {
                            if verbose {
                                println!("{description} failed.");
                            }
                            return Err(e);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn uu_app() -> Command {
    let mut cmd = Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("report")
                .long("report")
                .help("print report for specified devices")
                .action(ArgAction::SetTrue),
        )
        .arg(Arg::new("devices").required(true).action(ArgAction::Append));

    for (flag, action) in BLOCKDEV_ACTIONS {
        let mut arg = Arg::new(flag)
            .long(flag)
            .conflicts_with("report")
            .action(ArgAction::Append)
            .value_parser(value_parser!(usize));

        match action {
            BlockdevCommand::SetVerbosity(true) => {
                arg = arg.short('v').help("verbose mode");
            }
            BlockdevCommand::SetVerbosity(false) => {
                arg = arg.short('q').help("quiet mode");
            }
            BlockdevCommand::Ioctl(name, _, _) => {
                arg = arg.help(name);
            }
        }

        match action {
            BlockdevCommand::Ioctl(_, _, IoctlCommand::SetAttribute) => {
                arg = arg.num_args(1);
            }
            _ => {
                arg = arg
                    .num_args(0)
                    .default_value("0")
                    .default_missing_value("0");
            }
        }
        cmd = cmd.arg(arg);
    }
    cmd
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`blockdev` is available only on Linux.",
    ))
}
