// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::io::{self, Write};
#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;
use std::path::Path;

use crate::escape::escape_octal;

/// Append a mount entry to the file at `path`.
///
/// If `path` is a symbolic link the function returns `Ok(())` without writing
/// anything. On modern Linux systems `/etc/mtab` is typically a symlink to
/// `/proc/mounts`, so the kernel already maintains the mount list and there is
/// nothing to update.
///
/// When the file is a real file an exclusive `flock(2)` lock is acquired
/// before appending so that concurrent `mount` invocations cannot corrupt it.
///
/// The entry is written in the same whitespace-separated format used by
/// `/proc/mounts`:
/// ```text
/// <source> <target> <fstype> <options> 0 0
/// ```
#[cfg(target_os = "linux")]
pub fn write_mtab_to(
    path: &Path,
    source: &str,
    target: &str,
    fs_type: &str,
    opts: &str,
) -> io::Result<()> {
    // Use symlink_metadata so that we inspect the link itself, not its target.
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        // If the file doesn't exist there is nothing to update.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };

    if meta.file_type().is_symlink() {
        return Ok(());
    }

    let file = std::fs::OpenOptions::new().append(true).open(path)?;

    // Hold an exclusive lock for the duration of the write to prevent races
    // with other concurrent mount processes.
    let lock_result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) };
    if lock_result != 0 {
        return Err(io::Error::last_os_error());
    }

    let entry = format!(
        "{} {} {} {} 0 0\n",
        escape_octal(source),
        escape_octal(target),
        escape_octal(fs_type),
        escape_octal(opts)
    );
    (&file).write_all(entry.as_bytes())?;

    // The exclusive lock is released automatically when `file` drops here.
    Ok(())
}

/// Append a mount entry to `/etc/mtab`.
///
/// This is a convenience wrapper around [`write_mtab_to`] that uses the
/// standard path `/etc/mtab`.
#[cfg(target_os = "linux")]
pub fn write_mtab(source: &str, target: &str, fs_type: &str, opts: &str) -> io::Result<()> {
    write_mtab_to(Path::new("/etc/mtab"), source, target, fs_type, opts)
}
