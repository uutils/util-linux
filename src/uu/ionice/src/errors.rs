// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) ERANGE ioprio pgid strerror

use std::fmt;
use std::io;

use uucore::error::{strip_errno, UError};

/// Which option a numeric argument came from. Supplies the middle of the
/// "invalid ... argument" message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericArg {
    Class,
    ClassData,
    Pid,
    Pgid,
    Uid,
}

impl fmt::Display for NumericArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Class => "class",
            Self::ClassData => "class data",
            Self::Pid => "PID",
            Self::Pgid => "PGID",
            Self::Uid => "UID",
        })
    }
}

/// Trailing detail on a numeric argument error: nothing when the text was
/// merely malformed, strerror(ERANGE) when it did not fit in an i32, and the
/// bounds themselves when it fell outside a range ionice(1) documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericDetail {
    Malformed,
    Overflow,
    OutOfRange { low: i32, high: i32 },
}

impl fmt::Display for NumericDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => Ok(()),
            Self::Overflow => {
                let range = io::Error::from_raw_os_error(libc::ERANGE);
                write!(f, ": {}", strip_errno(&range))
            }
            Self::OutOfRange { low, high } => write!(f, ": must be {low}-{high}"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum IoniceError {
    #[error("invalid {arg} argument: '{value}'{detail}")]
    InvalidNumber {
        arg: NumericArg,
        value: String,
        detail: NumericDetail,
    },

    #[error("unknown scheduling class: '{0}'")]
    UnknownClass(String),

    #[error("can handle only one of pid, pgid or uid at once")]
    ConflictingIdKinds,

    // The hint is built here rather than left to UError::usage(), which spells
    // out execution_phrase() - the whole multicall binary path - instead of the
    // utility name.
    #[error(
        "bad usage\nTry '{} --help' for more information.",
        uucore::util_name()
    )]
    BadUsage,

    #[error("ioprio_get failed: {}", strip_errno(.0))]
    GetFailed(io::Error),

    #[error("ioprio_set failed: {}", strip_errno(.0))]
    SetFailed(io::Error),
}

impl IoniceError {
    pub(crate) fn invalid_number(arg: NumericArg, value: &str, detail: NumericDetail) -> Self {
        Self::InvalidNumber {
            arg,
            value: value.to_owned(),
            detail,
        }
    }
}

impl UError for IoniceError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}
