// This file is part of the uutils hostname package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ChCpuError {
    #[error("CPU {0} is enabled")]
    CpuIsEnabled(usize),

    #[error("CPU {0} is not configurable")]
    CpuNotConfigurable(usize),

    #[error("CPU {0} is not hot pluggable")]
    CpuNotHotPluggable(usize),

    #[error("this system does not support rescanning of CPUs")]
    CpuRescanUnsupported,

    #[error("first element of CPU list range is greater than its last element")]
    CpuSpecFirstAfterLast,

    #[error("CPU list element is not a positive number")]
    CpuSpecNotPositiveInteger,

    #[error("CPU list is empty")]
    EmptyCpuList,

    #[error("CPU {0} does not exist")]
    InvalidCpuIndex(usize),

    #[error("{0}: {1}")]
    IO0(String, std::io::Error),

    #[error("{0} '{path}': {2}", path = .1.display())]
    IO1(String, PathBuf, std::io::Error),

    #[error("only one CPU is enabled")]
    OneCpuIsEnabled,

    #[error("data is not an integer '{0}'")]
    NotInteger(String),

    #[error("this system does not support setting the dispatching mode of CPUs")]
    SetCpuDispatchUnsupported,
}

impl ChCpuError {
    pub(crate) fn io0(message: impl Into<String>, error: std::io::Error) -> Self {
        Self::IO0(message.into(), error)
    }

    pub(crate) fn io1(
        message: impl Into<String>,
        path: impl Into<PathBuf>,
        error: std::io::Error,
    ) -> Self {
        Self::IO1(message.into(), path.into(), error)
    }

    pub(crate) fn with_io_message(self, message: impl Into<String>) -> Self {
        match self {
            Self::IO0(_, err) => Self::IO0(message.into(), err),

            Self::IO1(_, path, err) => Self::IO1(message.into(), path, err),

            Self::CpuIsEnabled(_)
            | Self::CpuNotConfigurable(_)
            | Self::CpuNotHotPluggable(_)
            | Self::CpuRescanUnsupported
            | Self::CpuSpecFirstAfterLast
            | Self::CpuSpecNotPositiveInteger
            | Self::EmptyCpuList
            | Self::InvalidCpuIndex(_)
            | Self::OneCpuIsEnabled
            | Self::NotInteger(_)
            | Self::SetCpuDispatchUnsupported => self,
        }
    }
}

impl uucore::error::UError for ChCpuError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}
