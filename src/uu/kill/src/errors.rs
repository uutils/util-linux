// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::error::Error;

use uucore::error::UError;

#[derive(Debug)]
pub enum KillError {
    /// Unsupported platform
    #[cfg(not(target_os = "linux"))]
    UnsupportedPlatform,
    OperationNotPermitted(i32),
    NoSuchProcess(i32),
}

impl std::fmt::Display for KillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(target_os = "linux"))]
            Self::UnsupportedPlatform => write!(f, "kill is only supported on Linux for now"),
            Self::OperationNotPermitted(pid) => {
                write!(f, "bash: kill: ({pid}) - Operation not permitted")
            }
            Self::NoSuchProcess(pid) => write!(f, "bash: kill: ({pid}) - No such process"),
        }
    }
}

impl UError for KillError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}

impl Error for KillError {}
