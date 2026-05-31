// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::c_int;
use std::fmt;

use uucore::error::UError;

#[derive(Debug)]
pub enum LsnsError {
    /// Generic I/O error with context message
    IOError(String, std::io::Error),
    /// CString conversion error (null byte in string)
    NulError(String, std::ffi::NulError),
    /// Invalid namespace type index
    InvalidNamespaceType(usize),
}

impl LsnsError {
    /// Create an I/O error with a context message
    pub(crate) fn io0(message: impl Into<String>, error: impl Into<std::io::Error>) -> Self {
        Self::IOError(message.into(), error.into())
    }

    /// Helper to convert negative errno to Result
    pub(crate) fn io_from_neg_errno(
        message: impl Into<String>,
        result: c_int,
    ) -> Result<usize, LsnsError> {
        if let Ok(result) = usize::try_from(result) {
            Ok(result)
        } else {
            let err = std::io::Error::from_raw_os_error(-result);
            Err(Self::IOError(message.into(), err))
        }
    }
}

impl fmt::Display for LsnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IOError(message, err) => write!(f, "{message}: {err}"),
            Self::NulError(message, err) => write!(f, "{message}: {err}"),
            Self::InvalidNamespaceType(idx) => write!(f, "Invalid namespace type index: {}", idx),
        }
    }
}

impl UError for LsnsError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}

impl std::error::Error for LsnsError {}

// Implement From trait for automatic conversion from std::io::Error
impl From<std::io::Error> for LsnsError {
    fn from(err: std::io::Error) -> Self {
        Self::IOError(String::new(), err)
    }
}

// Implement From trait for automatic conversion from std::ffi::NulError
impl From<std::ffi::NulError> for LsnsError {
    fn from(err: std::ffi::NulError) -> Self {
        Self::NulError(String::new(), err)
    }
}
