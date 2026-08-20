// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! A stream that remembers its first write failure instead of propagating it.
//!
//! Both of rename's streams need this, and for the same reason: C util-linux
//! finishes the whole run and then reports a failed write once, exiting 1
//! whatever the tally says. The renames still happen; only the report is lost.

use std::ffi::OsStr;
use std::io::{self, Write};

use crate::encoding;

pub(crate) struct Output<W: Write> {
    inner: W,
    error: Option<io::Error>,
}

impl<W: Write> Output<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self { inner, error: None }
    }

    fn record(&mut self, result: io::Result<()>) {
        if let Err(error) = result {
            self.error.get_or_insert(error);
        }
    }

    pub(crate) fn write_bytes(&mut self, bytes: &[u8]) {
        let result = self.inner.write_all(bytes);
        self.record(result);
    }

    /// Names go out raw, so one holding a newline really does split the line in
    /// two - as it does for C util-linux, on either stream.
    pub(crate) fn write_os(&mut self, value: &OsStr) {
        let result = encoding::write_os(&mut self.inner, value);
        self.record(result);
    }

    pub(crate) fn write_quoted(&mut self, value: &OsStr) {
        self.write_bytes(b"`");
        self.write_os(value);
        self.write_bytes(b"'");
    }

    pub(crate) fn flush(&mut self) {
        let result = self.inner.flush();
        self.record(result);
    }

    pub(crate) fn into_error(self) -> Option<io::Error> {
        self.error
    }

    /// On the REPORT stream C util-linux is selective about which failed write
    /// it complains of at exit: a full disk yes, a reader that has gone away
    /// no, so a run piped into something short-lived still reports what it did.
    /// It is not selective on the diagnostic stream, which is why this is a
    /// method here and not a rule in `into_error`.
    pub(crate) fn into_reported_error(self) -> Option<io::Error> {
        self.into_error()
            .filter(|error| error.kind() != io::ErrorKind::BrokenPipe)
    }
}
