// This file is part of the uutils hostname package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::fmt;
use std::path::PathBuf;

use uucore::error::UError;

#[derive(Debug)]
pub enum ChCpuError {
    CpuIsEnabled(usize),
    CpuNotConfigurable(usize),
    CpuNotHotPluggable(usize),
    CpuRescanUnsupported,
    CpuSpecFirstAfterLast,
    CpuSpecNotPositiveInteger,
    EmptyCpuList,
    InvalidCpuIndex(usize),
    IO0(String, std::io::Error),
    IO1(String, PathBuf, std::io::Error),
    OneCpuIsEnabled,
    NotInteger(String),
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

impl fmt::Display for ChCpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CpuIsEnabled(index) => write!(f, "CPU {index} is enabled"),
            Self::CpuNotConfigurable(index) => write!(f, "CPU {index} is not configurable"),
            Self::CpuNotHotPluggable(index) => write!(f, "CPU {index} is not hot pluggable"),
            Self::CpuRescanUnsupported => {
                write!(f, "this system does not support rescanning of CPUs")
            }
            Self::CpuSpecFirstAfterLast => {
                write!(
                    f,
                    "first element of CPU list range is greater than its last element"
                )
            }
            Self::CpuSpecNotPositiveInteger => {
                write!(f, "CPU list element is not a positive number")
            }
            Self::EmptyCpuList => write!(f, "CPU list is empty"),
            Self::InvalidCpuIndex(index) => write!(f, "CPU {index} does not exist"),
            Self::IO0(message, err) => write!(f, "{message}: {err}"),
            Self::IO1(message, path, err) => write!(f, "{message} '{}': {err}", path.display()),
            Self::OneCpuIsEnabled => write!(f, "only one CPU is enabled"),
            Self::NotInteger(data) => write!(f, "data is not an integer '{data}'"),
            Self::SetCpuDispatchUnsupported => {
                write!(
                    f,
                    "this system does not support setting the dispatching mode of CPUs"
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
