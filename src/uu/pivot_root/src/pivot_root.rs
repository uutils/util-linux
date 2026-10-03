// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use std::ffi::{OsStr, OsString};
use uucore::{error::UResult, format_usage, help_about, help_usage};

mod errors;
use crate::errors::PivotRootError;

const ABOUT: &str = help_about!("pivot_root.md");
const USAGE: &str = help_usage!("pivot_root.md");

mod options {
    pub const NEW_ROOT: &str = "new_root";
    pub const PUT_OLD: &str = "put_old";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let new_root = matches
        .get_one::<OsString>(options::NEW_ROOT)
        .expect("required argument");
    let put_old = matches
        .get_one::<OsString>(options::PUT_OLD)
        .expect("required argument");

    pivot_root_syscall(new_root, put_old)?;

    Ok(())
}

/// Thin wrapper around the `pivot_root(2)` system call.
///
/// This function performs the `pivot_root` syscall directly via `libc::syscall`.
/// It does **not** perform any validation of the paths beyond checking for
/// embedded null bytes (which would be invalid for C strings passed to the kernel).
///
/// The kernel itself performs all semantic validation. See `pivot_root(2)` and
/// `stat(2)` man pages for full details on these errors.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn pivot_root_syscall(new_root: &OsStr, put_old: &OsStr) -> Result<(), PivotRootError> {
    use crate::errors::PathWhich;
    use std::ffi::CString;
    use std::io;
    use std::os::unix::ffi::OsStrExt;

    let new_root_cstr =
        CString::new(new_root.as_bytes()).map_err(|e| PivotRootError::NulError {
            which: PathWhich::NewRoot,
            pos: e.nul_position(),
            source: e,
            path: new_root.to_os_string(),
        })?;
    let put_old_cstr = CString::new(put_old.as_bytes()).map_err(|e| PivotRootError::NulError {
        which: PathWhich::PutOld,
        pos: e.nul_position(),
        source: e,
        path: put_old.to_os_string(),
    })?;

    let result = unsafe {
        libc::syscall(
            libc::SYS_pivot_root,
            new_root_cstr.as_ptr(),
            put_old_cstr.as_ptr(),
        )
    };

    match result {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error().into()),
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn pivot_root_syscall(_new_root: &OsStr, _put_old: &OsStr) -> Result<(), PivotRootError> {
    Err(PivotRootError::UnsupportedPlatform)
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::NEW_ROOT)
                .value_name("NEW_ROOT")
                .help("New root file system")
                .required(true)
                .index(1)
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::DirPath)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::PUT_OLD)
                .value_name("PUT_OLD")
                .help("Directory to move the current root to")
                .required(true)
                .index(2)
                .value_parser(clap::value_parser!(OsString))
                .value_hint(clap::ValueHint::DirPath)
                .action(ArgAction::Set),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uucore::error::UError;

    #[test]
    #[cfg(unix)]
    fn test_null_byte_in_new_root() {
        // Create a path with an embedded null byte using raw bytes
        // It's not expected that a null byte could be passed in via
        // the command line, but perhaps if pivot_root_syscall becomes
        // used outside of this crate.
        use std::os::unix::ffi::OsStrExt;
        let bytes = b"/tmp\0/test";
        let new_root = OsStr::from_bytes(bytes);
        let put_old = OsStr::new("/old");

        let result = pivot_root_syscall(new_root, put_old);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), 1);
        assert!(err.to_string().contains("null byte"));
    }

    #[test]
    #[cfg(unix)]
    fn test_null_byte_in_put_old() {
        use std::os::unix::ffi::OsStrExt;
        let new_root = OsStr::new("/tmp");
        let bytes = b"/old\0/test";
        let put_old = OsStr::from_bytes(bytes);

        let result = pivot_root_syscall(new_root, put_old);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), 1);
        assert!(err.to_string().contains("null byte"));
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn test_non_existent_paths() {
        // This test verifies that non-existent paths produce a proper syscall error, not a panic
        let new_root = OsStr::new("/non_existent_new_root_12345");
        let put_old = OsStr::new("/non_existent_put_old_12345");

        let result = pivot_root_syscall(new_root, put_old);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), 1);
        let s = err.to_string();
        // On most systems without sufficient privileges, EPERM is expected.
        // We only assert that we got a proper error message from the syscall path.
        assert!(s.contains("failed to change root"), "{s}");
    }

    #[test]
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    fn test_unsupported_platform() {
        let new_root = OsStr::new("/tmp");
        let put_old = OsStr::new("/old");

        let result = pivot_root_syscall(new_root, put_old);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), 1);
        assert!(err.to_string().contains("only supported on Linux"));
    }

    #[test]
    fn test_uu_app_has_correct_args() {
        let app = uu_app();
        let matches = app.try_get_matches_from(vec!["pivot_root", "/new", "/old"]);
        assert!(matches.is_ok());

        let matches = matches.unwrap();
        assert!(matches.contains_id(options::NEW_ROOT));
        assert!(matches.contains_id(options::PUT_OLD));

        let new_root = matches.get_one::<OsString>(options::NEW_ROOT);
        let put_old = matches.get_one::<OsString>(options::PUT_OLD);

        assert!(new_root.is_some());
        assert!(put_old.is_some());
        assert_eq!(new_root.unwrap(), "/new");
        assert_eq!(put_old.unwrap(), "/old");
    }

    #[test]
    fn test_uu_app_missing_args() {
        let app = uu_app();
        let result = app.try_get_matches_from(vec!["pivot_root"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let s = err.to_string();
        assert!(s.to_lowercase().contains("required"), "{s}");
    }

    #[test]
    fn test_uu_app_one_arg() {
        let app = uu_app();
        let result = app.try_get_matches_from(vec!["pivot_root", "/new"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let s = err.to_string();
        assert!(s.to_lowercase().contains("required"), "{s}");
    }

    #[test]
    fn test_uu_app_help() {
        let app = uu_app();
        let result = app.try_get_matches_from(vec!["pivot_root", "--help"]);
        // --help causes an exit with DisplayHelp error
        assert!(result.is_err());
        let s = result.unwrap_err().to_string();
        assert!(s.contains("pivot_root"), "{s}");
        assert!(s.contains("Usage") || s.contains("USAGE"), "{s}");
    }

    #[test]
    fn test_uu_app_version() {
        let app = uu_app();
        let result = app.try_get_matches_from(vec!["pivot_root", "--version"]);
        // --version causes an exit with DisplayVersion error
        assert!(result.is_err());
        let s = result.unwrap_err().to_string();
        assert!(s.contains("pivot_root"), "{s}");
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn test_valid_path_construction() {
        // Test that we can create paths from valid inputs without panicking
        let test_cases = vec![
            ("/tmp", "/old"),
            ("/new/root/path", "/put/old/path"),
            ("/", "/old"),
        ];

        for (new_root, put_old) in test_cases {
            let new_root_os = OsString::from(new_root);
            let put_old_os = OsString::from(put_old);

            // This shouldn't panic, even though it will fail with permission/ENOENT errors
            let result = pivot_root_syscall(&new_root_os, &put_old_os);

            // We expect an error (no permission or path doesn't exist),
            // but it should be a proper error, not a panic
            assert!(result.is_err());
            let s = result.unwrap_err().to_string();
            assert!(s.contains("failed to change root"), "{s}");
        }
    }

    #[test]
    fn test_uu_app_accepts_paths_with_special_chars() {
        // Test that clap accepts paths with special characters and non-UTF-8 (on Unix)
        let test_cases = vec![
            ("pivot_root", "/new-root", "/put_old"),
            ("pivot_root", "/new root", "/put old"),
            ("pivot_root", "/new@root#123", "/put$old%456"),
        ];

        for args in test_cases {
            let app = uu_app();
            let result = app.try_get_matches_from(vec![args.0, args.1, args.2]);
            assert!(result.is_ok(), "Failed to parse args: {:?}", args);
        }

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let non_utf8_cases: Vec<(&[u8], &[u8])> = vec![
                (b"/new-\xFFroot", b"/put-old"),
                (b"/new-root", b"/put-\x80old"),
            ];

            for (new_root_bytes, put_old_bytes) in non_utf8_cases {
                let app = uu_app();

                let mut args: Vec<OsString> = Vec::new();
                args.push(OsString::from("pivot_root"));
                args.push(OsStr::from_bytes(new_root_bytes).to_os_string());
                args.push(OsStr::from_bytes(put_old_bytes).to_os_string());

                let result = app.try_get_matches_from(args);
                assert!(result.is_ok(), "Failed to parse non-UTF-8 args");
            }
        }
    }

    #[test]
    fn test_uu_app_too_many_args() {
        let app = uu_app();
        let result = app.try_get_matches_from(vec!["pivot_root", "/new", "/old", "/extra"]);
        assert!(result.is_err(), "Should reject extra arguments");
        let s = result.unwrap_err().to_string();
        assert!(
            s.contains("unexpected") || s.contains("Found argument"),
            "{s}"
        );
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn test_empty_paths() {
        // Test empty path handling
        let new_root = OsStr::new("");
        let put_old = OsStr::new("");

        let result = pivot_root_syscall(new_root, put_old);
        assert!(result.is_err());
        // Should get a syscall error, not a panic
    }
}
