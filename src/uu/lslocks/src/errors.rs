// This file is part of the uutils hostname package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::c_int;
use std::fmt;
use std::path::PathBuf;

use uucore::error::UError;

#[derive(Debug)]
pub enum LsLocksError {
    InvalidColumnName(String),
    InvalidColumnSequence(String),
    IO0(String, std::io::Error),
    IO1(String, PathBuf, std::io::Error),
}

impl LsLocksError {
    pub(crate) fn io0(message: impl Into<String>, error: impl Into<std::io::Error>) -> Self {
        Self::IO0(message.into(), error.into())
    }

    pub(crate) fn io1(
        message: impl Into<String>,
        path: impl Into<PathBuf>,
        error: impl Into<std::io::Error>,
    ) -> Self {
        Self::IO1(message.into(), path.into(), error.into())
    }

    pub(crate) fn io_from_neg_errno(
        message: impl Into<String>,
        result: c_int,
    ) -> Result<usize, LsLocksError> {
        if let Ok(result) = usize::try_from(result) {
            Ok(result)
        } else {
            let err = std::io::Error::from_raw_os_error(-result);
            Err(Self::IO0(message.into(), err))
        }
    }
}

impl fmt::Display for LsLocksError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO0(message, err) => write!(f, "{message}: {err}"),
            Self::IO1(message, path, err) => write!(f, "{message} '{}': {err}", path.display()),
            Self::InvalidColumnName(name) => write!(f, "invalid column name: {name}"),
            Self::InvalidColumnSequence(seq) => write!(f, "invalid column sequence: {seq}"),
        }
    }
}

impl UError for LsLocksError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}

impl std::error::Error for LsLocksError {}
