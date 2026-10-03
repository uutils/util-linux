// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{NulError, OsString};

#[derive(Debug)]
#[allow(dead_code)] // Never constructed on non-Linux platforms
pub(crate) enum PathWhich {
    NewRoot,
    PutOld,
}

impl std::fmt::Display for PathWhich {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathWhich::NewRoot => write!(f, "new_root"),
            PathWhich::PutOld => write!(f, "put_old"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PivotRootError {
    #[error("{which} path contains null byte at position {pos} (in '{path:?}')")]
    NulError {
        which: PathWhich,
        pos: usize,
        source: NulError,
        path: OsString,
    },

    #[error("{message}")]
    SyscallFailed {
        message: String,
        source: std::io::Error,
    },

    #[allow(dead_code)] // Only used on non-Linux platforms
    #[error("pivot_root is only supported on Linux")]
    UnsupportedPlatform,
}

impl uucore::error::UError for PivotRootError {
    fn code(&self) -> i32 {
        1
    }

    fn usage(&self) -> bool {
        false
    }
}

/// Convert a `std::io::Error` into a `PivotRootError` immediately after a
/// failed `pivot_root(2)` syscall.
///
/// Important: this conversion is intended to be used right at the call site of
/// `pivot_root`, with the error value obtained from `std::io::Error::last_os_error()`.
/// Doing so preserves the correct `errno` from the kernel and lets us attach
/// helpful hints to well-known error codes (e.g., `EPERM`, `EINVAL`). Using an
/// arbitrary `std::io::Error` captured earlier or created in another context
/// may carry a stale or unrelated `raw_os_error`, which would yield misleading
/// diagnostics. The error codes can be obtained from the `pivot_root(2)` man page,
/// which acknowledges that errors from the `stat(2)` system call may also occur.
impl From<std::io::Error> for PivotRootError {
    fn from(err: std::io::Error) -> Self {
        let mut msg = format!("failed to change root: {}", err);
        if let Some(code) = err.raw_os_error() {
            msg.push_str(&format!(" (errno {code})"));
            msg.push_str(match code {
                libc::EPERM => "; the calling process does not have the CAP_SYS_ADMIN capability",
                libc::EBUSY => "; new_root or put_old is on the current root mount",
                libc::EINVAL => {
                    "; new_root is not a mount point, put_old is not at or underneath new_root, \
                     the current root is not a mount point, the current root is on the rootfs, \
                     or a mount point has propagation type MS_SHARED"
                }
                libc::ENOTDIR => "; new_root or put_old is not a directory",
                libc::EACCES => "; search permission denied for a directory in the path prefix",
                libc::EBADF => "; bad file descriptor",
                libc::EFAULT => "; new_root or put_old points outside the accessible address space",
                libc::ELOOP => "; too many symbolic links encountered while resolving the path",
                libc::ENAMETOOLONG => "; new_root or put_old path is too long",
                libc::ENOENT => {
                    "; a component of new_root or put_old does not exist, \
                     or is a dangling symbolic link"
                }
                libc::ENOMEM => "; out of kernel memory",
                libc::EOVERFLOW => {
                    "; path refers to a file whose size, inode number, or number of blocks \
                     cannot be represented"
                }
                _ => "",
            });
        }

        PivotRootError::SyscallFailed {
            message: msg,
            source: err,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nul_error_display() {
        // Create a NulError via CString::new
        let bytes = b"/tmp\0/dir";
        let err = std::ffi::CString::new(&bytes[..]).unwrap_err();
        let e = PivotRootError::NulError {
            which: PathWhich::NewRoot,
            pos: err.nul_position(),
            source: err,
            path: OsString::from("/tmp\u{0}/dir"),
        };
        let s = e.to_string();
        assert!(s.contains("new_root"), "{s}");
        assert!(s.contains("null byte"), "{s}");
    }

    fn msg_for(code: i32) -> String {
        let err = std::io::Error::from_raw_os_error(code);
        let e = PivotRootError::from(err);
        e.to_string()
    }

    #[test]
    fn test_syscall_failed_eperm_hint() {
        let s = msg_for(libc::EPERM);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("errno"), "{s}");
        assert!(s.contains("CAP_SYS_ADMIN"), "{s}");
    }

    #[test]
    fn test_syscall_failed_ebusy_hint() {
        let s = msg_for(libc::EBUSY);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("on the current root mount"), "{s}");
    }

    #[test]
    fn test_syscall_failed_einval_hint() {
        let s = msg_for(libc::EINVAL);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("not a mount point"), "{s}");
        assert!(s.contains("MS_SHARED"), "{s}");
    }

    #[test]
    fn test_syscall_failed_enotdir_hint() {
        let s = msg_for(libc::ENOTDIR);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("not a directory"), "{s}");
    }

    #[test]
    fn test_syscall_failed_eacces_hint() {
        let s = msg_for(libc::EACCES);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("permission denied"), "{s}");
    }

    #[test]
    fn test_syscall_failed_ebadf_hint() {
        let s = msg_for(libc::EBADF);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("bad file descriptor"), "{s}");
    }

    #[test]
    fn test_syscall_failed_efault_hint() {
        let s = msg_for(libc::EFAULT);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("accessible address space"), "{s}");
    }

    #[test]
    fn test_syscall_failed_eloop_hint() {
        let s = msg_for(libc::ELOOP);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("symbolic links"), "{s}");
    }

    #[test]
    fn test_syscall_failed_enametoolong_hint() {
        let s = msg_for(libc::ENAMETOOLONG);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("path is too long"), "{s}");
    }

    #[test]
    fn test_syscall_failed_enoent_hint() {
        let s = msg_for(libc::ENOENT);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("does not exist"), "{s}");
        assert!(s.contains("dangling symbolic link"), "{s}");
    }

    #[test]
    fn test_syscall_failed_enomem_hint() {
        let s = msg_for(libc::ENOMEM);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("out of kernel memory"), "{s}");
    }

    #[test]
    fn test_syscall_failed_eoverflow_hint() {
        let s = msg_for(libc::EOVERFLOW);
        assert!(s.contains("failed to change root"), "{s}");
        assert!(s.contains("cannot be represented"), "{s}");
    }

    #[test]
    fn test_unsupported_platform_display() {
        let s = PivotRootError::UnsupportedPlatform.to_string();
        assert!(s.contains("only supported on Linux"), "{s}");
    }
}
