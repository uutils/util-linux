// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) ioprio

use std::io;

use uucore::error::{strip_errno, UError};

use crate::ioprio::Who;
use crate::parse::NumericDetail;

#[derive(Debug, thiserror::Error)]
pub(crate) enum IoniceError {
    #[error("invalid {who} argument: '{value}': {detail}")]
    InvalidNumber {
        who: Who,
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
    pub(crate) fn invalid_number(who: Who, value: &str, detail: NumericDetail) -> Self {
        Self::InvalidNumber {
            who,
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
