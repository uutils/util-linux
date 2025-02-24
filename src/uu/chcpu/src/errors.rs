// This file is part of the uutils hostname package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::fmt;

use uucore::error::UError;

#[derive(Debug, PartialEq, Eq)]
pub enum ChCpuError {
    EmptyCpuList,
    CpuSpecNotPositiveInteger,
    CpuSpecFirstAfterLast,
}

impl fmt::Display for ChCpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCpuList => write!(f, "CPU list is empty"),
            Self::CpuSpecNotPositiveInteger => {
                write!(f, "CPU list element is not a positive number")
            }
            Self::CpuSpecFirstAfterLast => {
                write!(
                    f,
                    "first element of CPU list range is greater than its last element"
                )
            }
        }
    }
}

impl UError for ChCpuError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}

impl std::error::Error for ChCpuError {}
