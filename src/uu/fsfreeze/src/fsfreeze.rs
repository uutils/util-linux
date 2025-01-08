// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};
#[cfg(target_os = "linux")]
use std::{fs::File, io, os::fd::AsRawFd};
#[cfg(target_os = "linux")]
use uucore::{error::UIoError, libc};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("fsfreeze.md");
const USAGE: &str = help_usage!("fsfreeze.md");

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let mountpoint = matches.get_one::<String>("mountpoint").unwrap();
    let file = File::open(mountpoint)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(UIoError::new(io::ErrorKind::InvalidData, "not a directory"));
    }

    let (op_name, op_code) = if matches.get_flag("freeze") {
        ("freeze", linux_raw_sys::ioctl::FIFREEZE)
    } else {
        ("unfreeze", linux_raw_sys::ioctl::FITHAW)
    };

    if unsafe { libc::ioctl(file.as_raw_fd(), op_code.into(), 0) } < 0 {
        uucore::show_error!(
            "failed to {} the filesystem: {}",
            op_name,
            UIoError::from(io::Error::last_os_error())
        );
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("freeze")
                .short('f')
                .long("freeze")
                .help("freeze the filesystem")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("unfreeze")
                .short('u')
                .long("unfreeze")
                .help("unfreeze the filesystem")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("mountpoint")
                .help("mountpoint of the filesystem")
                .required(true)
                .action(ArgAction::Set),
        )
        .group(
            ArgGroup::new("action")
                .required(true)
                .args(["freeze", "unfreeze"]),
        )
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`fsfreeze` is available only on Linux.",
    ))
}
