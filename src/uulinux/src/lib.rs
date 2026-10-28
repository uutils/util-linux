// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::path::{Path, PathBuf};

/// Join `path` under `root`, ignoring any leading `/` in `path`.
///
/// Unlike [`Path::join`], this never discards `root` when `path` is absolute.
/// Useful for prepending a sysroot to a system path like `/sys/devices/...`.
pub fn join_under_root(root: &Path, path: &Path) -> PathBuf {
    let relative = path.strip_prefix("/").unwrap_or(path);
    root.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_path_is_joined_under_root() {
        assert_eq!(
            join_under_root(
                Path::new("/sysroot"),
                Path::new("/sys/devices/system/memory")
            ),
            PathBuf::from("/sysroot/sys/devices/system/memory"),
        );
    }

    #[test]
    fn relative_path_is_joined_normally() {
        assert_eq!(
            join_under_root(Path::new("/sysroot"), Path::new("sys/devices")),
            PathBuf::from("/sysroot/sys/devices"),
        );
    }

    #[test]
    fn root_slash_alone_gives_root() {
        assert_eq!(
            join_under_root(Path::new("/sysroot"), Path::new("/")),
            PathBuf::from("/sysroot"),
        );
    }

    #[test]
    fn trailing_slash_on_root_is_handled() {
        assert_eq!(
            join_under_root(Path::new("/sysroot/"), Path::new("/sys/devices")),
            PathBuf::from("/sysroot/sys/devices"),
        );
    }
}
