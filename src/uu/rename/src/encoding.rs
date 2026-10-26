// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The bridge between OsStr and the code units the engine works on.
//!
//! Both directions are lossless: bytes on unix, UTF-16 units on Windows. The
//! path separator is 0x2F in either encoding, so the scope rule needs no
//! special casing. `\` is deliberately not a separator here: rename(1)
//! documents `/`, and Rust accepts `/` on Windows too.
//!
//! Only unix and windows are covered; a third target does not build.
//!
//! Writing a name is done here rather than through `uucore::display::OsWrite`
//! because that trait refuses a Windows name that is not valid Unicode, with
//! `io::ErrorKind::InvalidData`, where a report has to print whatever the name
//! actually is - and because it has no impl for stderr.

#[cfg(unix)]
mod imp {
    use std::ffi::{OsStr, OsString};
    use std::io::{self, Write};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;

    pub(crate) type Unit = u8;
    pub(crate) const SEP: Unit = b'/';

    pub(crate) fn units(s: &OsStr) -> Vec<Unit> {
        s.as_bytes().to_vec()
    }

    pub(crate) fn os_string(units: Vec<Unit>) -> OsString {
        OsString::from_vec(units)
    }

    /// Filenames are written raw. A name holding an invalid byte is emitted as
    /// that byte, which is what C util-linux does and what makes its -v output
    /// round-trip through a shell.
    pub(crate) fn write_os(w: &mut impl Write, s: &OsStr) -> io::Result<()> {
        w.write_all(s.as_bytes())
    }

    pub(crate) fn symlink(target: &OsStr, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }
}

#[cfg(windows)]
mod imp {
    use std::ffi::{OsStr, OsString};
    use std::io::{self, Write};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;

    pub(crate) type Unit = u16;
    pub(crate) const SEP: Unit = b'/' as u16;

    pub(crate) fn units(s: &OsStr) -> Vec<Unit> {
        s.encode_wide().collect()
    }

    pub(crate) fn os_string(units: Vec<Unit>) -> OsString {
        OsString::from_wide(&units)
    }

    /// There is no raw byte form of a Windows filename, so a lone surrogate is
    /// replaced here. Unlike the unix arm this is lossy, and only for output.
    pub(crate) fn write_os(w: &mut impl Write, s: &OsStr) -> io::Result<()> {
        w.write_all(s.to_string_lossy().as_bytes())
    }

    /// Windows splits the call in two and needs the privilege or Developer
    /// Mode to make either. Which one is right depends on what the target
    /// names, and a relative target is stored relative to the LINK, so it is
    /// resolved from the link's own directory - not from the process working
    /// directory, which would answer about a different object and leave a link
    /// Windows reports as broken. The -o guard resolves relative to the
    /// process instead, because that side has C util-linux to conform to and
    /// this side has none.
    ///
    /// A target that does not resolve is treated as a file, and whatever the
    /// OS then says is passed back to the caller rather than pre-empted here.
    pub(crate) fn symlink(target: &OsStr, link: &Path) -> io::Result<()> {
        let base = link.parent().unwrap_or_else(|| Path::new(""));
        if base.join(target).is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
}

pub(crate) use imp::{os_string, symlink, units, write_os, Unit, SEP};
