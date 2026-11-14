// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) ioprio pgid

use std::fmt;
use std::io;

use uucore::error::{strip_errno, UError};

use crate::parse::NumericDetail;

/// Which option a numeric argument came from. Supplies the middle of the
/// "invalid ... argument" message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericArg {
    Pid,
    Pgid,
    Uid,
}

impl fmt::Display for NumericArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pid => "PID",
            Self::Pgid => "PGID",
            Self::Uid => "UID",
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum IoniceError {
    #[error("invalid {arg} argument: '{value}': {detail}")]
    InvalidNumber {
        arg: NumericArg,
        value: String,
        detail: NumericDetail,
    },

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
