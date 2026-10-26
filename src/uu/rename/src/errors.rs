// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Per-operand diagnostics.
//!
//! Each variant keeps the path it complains about as an `OsString` and writes
//! it out as the bytes it is made of. A name that is not valid UTF-8 is exactly
//! the kind of name people reach for rename to fix, and C util-linux prints it
//! raw on stderr just as it does on stdout; rendering it through a `String`
//! would replace it with U+FFFD.
//!
//! That is also why these do not implement `Display`. `Formatter` writes
//! `&str`, so the diagnostic and a lossless name are mutually exclusive.
//!
//! The trailing " (os error NN)" that Rust appends to an io::Error is stripped
//! back off: C util-linux prints the bare strerror text and nothing else.

use std::ffi::OsString;
use std::io::{self, Write};

use uucore::error::strip_errno;

use crate::output::Output;

#[derive(Debug)]
pub(crate) enum RenameError {
    NotAccessible {
        path: OsString,
        source: io::Error,
    },

    NotASymlink {
        path: OsString,
    },

    RenameFailed {
        old: OsString,
        new: OsString,
        source: io::Error,
    },

    /// The two halves of a symlink rewrite report separately, as they do for
    /// C util-linux: a link in a directory that denies writes fails at the
    /// unlink, and says so.
    UnlinkFailed {
        path: OsString,
        source: io::Error,
    },

    /// This one names the target it tried to create, so an empty target prints
    /// two spaces between "to" and "failed".
    SymlinkFailed {
        path: OsString,
        new: OsString,
        source: io::Error,
    },

    WriteFailed {
        source: io::Error,
    },
}

impl RenameError {
    /// The `rename: ` prefix is written here rather than by `show_error!`,
    /// which renders through `format!` and cannot carry a name losslessly.
    pub(crate) fn report<W: Write>(&self, out: &mut Output<W>) {
        out.write_bytes(uucore::util_name().as_bytes());
        out.write_bytes(b": ");

        match self {
            Self::NotAccessible { path, source } => {
                out.write_os(path);
                out.write_bytes(b": not accessible: ");
                out.write_bytes(strip_errno(source).as_bytes());
            }
            Self::NotASymlink { path } => {
                out.write_os(path);
                out.write_bytes(b": not a symbolic link");
            }
            Self::RenameFailed { old, new, source } => {
                out.write_os(old);
                out.write_bytes(b": rename to ");
                out.write_os(new);
                out.write_bytes(b" failed: ");
                out.write_bytes(strip_errno(source).as_bytes());
            }
            Self::UnlinkFailed { path, source } => {
                out.write_os(path);
                out.write_bytes(b": unlink failed: ");
                out.write_bytes(strip_errno(source).as_bytes());
            }
            Self::SymlinkFailed { path, new, source } => {
                out.write_os(path);
                out.write_bytes(b": symlinking to ");
                out.write_os(new);
                out.write_bytes(b" failed: ");
                out.write_bytes(strip_errno(source).as_bytes());
            }
            Self::WriteFailed { source } => {
                out.write_bytes(b"write error: ");
                out.write_bytes(strip_errno(source).as_bytes());
            }
        }

        out.write_bytes(b"\n");
    }
}
