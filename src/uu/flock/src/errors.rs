// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{ffi::OsString, io};
use uucore::{
    display::Quotable,
    error::{strip_errno, UError},
};

// Exit codes
pub const EX_USAGE: i32 = 64;
pub const EX_DATAERR: i32 = 65;
pub const EX_NOINPUT: i32 = 66;
pub const EX_UNAVAILABLE: i32 = 69;
pub const EX_OSERR: i32 = 71;

#[derive(Debug, thiserror::Error)]
pub enum FlockError {
    #[error("cannot open lock file {}: {}", .path.to_string_lossy(), strip_errno(.source))]
    OpenLockFile { path: OsString, source: io::Error },

    #[error("bad file descriptor: {}", .file_or_fd.quote())]
    BadFileDescriptor { file_or_fd: OsString },

    #[error("cannot set up timer: Invalid argument")]
    TimerSetup,

    #[error("{}: {}", .file_or_fd.to_string_lossy(), strip_errno(.source))]
    CannotAcquireLock {
        file_or_fd: OsString,
        source: io::Error,
    },

    #[error("failed to execute {}: {}", .program.to_string_lossy(), strip_errno(.source))]
    FailedToExecute {
        program: OsString,
        source: io::Error,
    },
}

impl UError for FlockError {
    fn code(&self) -> i32 {
        match self {
            Self::BadFileDescriptor { .. } => EX_USAGE,
            Self::CannotAcquireLock { .. } => EX_DATAERR,
            Self::OpenLockFile { .. } => EX_NOINPUT,
            Self::FailedToExecute { .. } => EX_UNAVAILABLE,
            Self::TimerSetup => EX_OSERR,
        }
    }
}
