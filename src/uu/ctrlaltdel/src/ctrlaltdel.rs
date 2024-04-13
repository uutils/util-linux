// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use std::fmt::Display;
use uucore::{
    error::{UError, UResult},
    format_usage, help_about, help_usage,
};

const ABOUT: &str = help_about!("ctrlaltdel.md");
const USAGE: &str = help_usage!("ctrlaltdel.md");
const CTRL_ALT_DEL_PATH: &str = "/proc/sys/kernel/ctrl-alt-del";

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let pattern = matches.get_one::<String>("pattern");
    match pattern {
        Some(x) if x == "hard" => {
            set_ctrlaltdel(CtrlAltDel::Hard)?;
        }
        Some(x) if x == "soft" => {
            set_ctrlaltdel(CtrlAltDel::Soft)?;
        }
        Some(x) => {
            Err(Error::UnknownArgument(x.clone()))?;
        }
        None => {
            println!("{}", get_ctrlaltdel()?);
        }
    }

    Ok(())
}

fn get_ctrlaltdel() -> UResult<CtrlAltDel> {
    let value: i32 = std::fs::read_to_string(CTRL_ALT_DEL_PATH)?
        .trim()
        .parse()
        .map_err(|_| Error::UnknownData)?;

    Ok(CtrlAltDel::from_sysctl(value))
}

fn set_ctrlaltdel(ctrlaltdel: CtrlAltDel) -> UResult<()> {
    std::fs::write(CTRL_ALT_DEL_PATH, format!("{}\n", ctrlaltdel.to_sysctl()))
        .map_err(|_| Error::NotRoot)?;

    Ok(())
}

#[derive(Clone, Copy)]
enum CtrlAltDel {
    Soft,
    Hard,
}
impl CtrlAltDel {
    /// # Panics
    /// Panics if value of the parameter `value` is neither `0` nor `1`.
    fn from_sysctl(value: i32) -> Self {
        match value {
            0 => Self::Soft,
            1 => Self::Hard,
            _ => unreachable!(),
        }
    }

    fn to_sysctl(self) -> i32 {
        match self {
            Self::Soft => 0,
            Self::Hard => 1,
        }
    }
}
impl Display for CtrlAltDel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Soft => write!(f, "soft"),
            Self::Hard => write!(f, "hard"),
        }
    }
}

#[derive(Debug)]
enum Error {
    NotRoot,
    UnknownArgument(String),
    UnknownData,
}
impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRoot => write!(f, "You must be root to set the Ctrl-Alt-Del behavior"),
            Self::UnknownArgument(x) => write!(f, "unknown argument: {x}"),
            Self::UnknownData => write!(f, "unknown data"),
        }
    }
}
impl std::error::Error for Error {}
impl UError for Error {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(Arg::new("pattern").action(ArgAction::Set))
}
