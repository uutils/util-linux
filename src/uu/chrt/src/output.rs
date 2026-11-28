// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Standard output, line-buffered on a terminal and in pieces of up to 4096
//! bytes otherwise. A failed write stops nothing; the first is reported at exit.

use std::fmt;
use std::io::{self, BufWriter, IsTerminal, StdoutLock, Write};

use crate::errors::ChrtError;

/// What chrt(1) writes at a time to a pipe or a file.
const BUFFER_SIZE: usize = 4096;

pub(crate) struct Output {
    inner: BufWriter<StdoutLock<'static>>,
    line_buffered: bool,
    error: Option<io::Error>,
}

impl Output {
    pub(crate) fn stdout() -> Self {
        let stdout = io::stdout();
        Self {
            line_buffered: stdout.is_terminal(),
            inner: BufWriter::with_capacity(BUFFER_SIZE, stdout.lock()),
            error: None,
        }
    }

    fn record(&mut self, result: io::Result<()>) {
        if let Err(error) = result {
            self.error.get_or_insert(error);
        }
    }

    pub(crate) fn write(&mut self, text: &str) {
        let result = self.inner.write_all(text.as_bytes());
        self.record(result);
        if self.line_buffered && text.contains('\n') {
            self.flush();
        }
    }

    fn flush(&mut self) {
        let result = self.inner.flush();
        self.record(result);
    }

    /// Reports the first failed write: nothing else tells a full disk from success.
    pub(crate) fn finish(mut self) -> Result<(), ChrtError> {
        self.flush();
        // A failed flush leaves bytes behind; dropping the BufWriter would try
        // to write them again.
        drop(self.inner.into_parts());
        self.error
            .map_or(Ok(()), |error| Err(ChrtError::WriteError(error)))
    }
}

impl fmt::Write for Output {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.write(text);
        Ok(())
    }
}

pub(crate) fn print(text: &str) -> Result<(), ChrtError> {
    let mut out = Output::stdout();
    out.write(text);
    out.finish()
}
