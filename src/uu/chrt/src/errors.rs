// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) ERANGE

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io;

use uucore::error::{strip_errno, UError};

use crate::parse::NumericDetail;
use crate::policy::Policy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumericArg {
    Priority,
    Pid,
}

impl fmt::Display for NumericArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Priority => "priority",
            Self::Pid => "PID",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskKind {
    Pid,
    Tid,
}

impl fmt::Display for TaskKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pid => "pid",
            Self::Tid => "tid",
        })
    }
}

impl fmt::Display for NumericDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => Ok(()),
            Self::Overflow => {
                let range = io::Error::from_raw_os_error(libc::ERANGE);
                write!(f, ": {}", strip_errno(&range))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ChrtError {
    #[error("invalid {arg} argument: '{}'{detail}", value.to_string_lossy())]
    InvalidNumber {
        arg: NumericArg,
        value: OsString,
        detail: NumericDetail,
    },

    #[error("too few arguments")]
    TooFewArguments,

    #[error("policy {} requires a priority argument", .0.name())]
    PriorityRequired(Policy),

    #[error("no command or priority specified")]
    NoCommandOrPriority,

    #[error("no command specified")]
    NoCommand,

    #[error("unsupported priority value for the policy: {0}: see --max for valid range")]
    UnsupportedPriority(i32),

    #[error("--sched-runtime option is supported for SCHED_OTHER, SCHED_BATCH and SCHED_DEADLINE")]
    RuntimeNotSupported,

    #[error("--sched-{{deadline,period}} options are supported for SCHED_DEADLINE only")]
    DeadlineParametersNotSupported,

    #[error("cannot obtain the list of tasks: {}", strip_errno(.0))]
    TaskListUnavailable(io::Error),

    #[error("failed to get pid {pid}'s policy: {}", strip_errno(error))]
    GetFailed { pid: i32, error: io::Error },

    #[error("failed to set {kind} {id}'s policy: {}", strip_errno(error))]
    SetFailed {
        kind: TaskKind,
        id: i32,
        error: io::Error,
    },

    #[error("write error: {}", strip_errno(.0))]
    WriteError(io::Error),

    #[error("failed to execute {}: {}", program.to_string_lossy(), strip_errno(error))]
    ExecFailed { program: OsString, error: io::Error },
}

impl ChrtError {
    pub(crate) fn invalid_number(arg: NumericArg, value: &OsStr, detail: NumericDetail) -> Self {
        Self::InvalidNumber {
            arg,
            value: value.to_owned(),
            detail,
        }
    }
}

impl UError for ChrtError {
    fn code(&self) -> i32 {
        match self {
            Self::ExecFailed { error, .. } if error.kind() == io::ErrorKind::NotFound => 127,
            Self::ExecFailed { .. } => 126,
            _ => 1,
        }
    }

    fn usage(&self) -> bool {
        false
    }
}
