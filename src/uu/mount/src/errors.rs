use thiserror::Error;
use uucore::error::UError;

pub const EXIT_MOUNT_FAILED: i32 = 32;

#[derive(Error, Debug)]
pub enum MountError {
    #[error("cannot open /proc/mounts: {0}")]
    ProcMounts(std::io::Error),

    #[error("cannot read {0}: {1}")]
    FstabRead(String, std::io::Error),

    #[error("I/O error: {0}")]
    Fstab(#[from] std::io::Error),

    #[error("cannot find device with label {0:?}")]
    LabelNotFound(String),

    #[error("cannot find device with UUID {0:?}")]
    UuidNotFound(String),

    #[error("cannot find mount entry for {0:?} in fstab")]
    FstabEntryNotFound(String),

    #[error("no mount point specified and none found in fstab for {0}")]
    NoMountPoint(String),

    #[error("cannot create mount point {0}: {1}")]
    CreateMountPoint(String, std::io::Error),

    #[error("cannot fork mount worker: {0}")]
    Fork(std::io::Error),

    #[error("cannot wait for mount worker: {0}")]
    Wait(std::io::Error),

    #[error("invalid source path: {0}")]
    InvalidSource(std::ffi::NulError),

    #[error("invalid target path: {0}")]
    InvalidTarget(std::ffi::NulError),

    #[error("invalid filesystem type: {0}")]
    InvalidFSType(std::ffi::NulError),

    #[error("invalid mount options: {0}")]
    InvalidOptions(std::ffi::NulError),

    #[error("mount: {1} on {2}: {0}")]
    MountFailed(std::io::Error, String, String),
}

impl UError for MountError {
    fn code(&self) -> i32 {
        match self {
            MountError::MountFailed(_, _, _) => EXIT_MOUNT_FAILED,
            _ => 1,
        }
    }
}
