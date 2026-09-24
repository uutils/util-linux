// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The -i prompt, and how much of stdin it takes.
//!
//! C util-linux decides this once, from the terminal settings on fd 0, before
//! the first operand is looked at, and the answer has two shapes. Off a
//! terminal, and on one whose line discipline assembles lines, a whole line is
//! consumed per prompt and a read holding no newline is read again. On a
//! terminal in non-canonical mode nothing will ever supply that newline, so one
//! read is the answer whatever it holds, the rest of it is discarded, and the
//! newline the terminal never sent is written to stdout instead.
//!
//! Reading a line rather than the whole answer is not an optimization. It is
//! what keeps a run bounded: a stdin that never ends would otherwise be
//! accumulated until the allocator gives up.

use std::ffi::OsStr;
use std::io::{self, BufRead, Write};

use crate::output::Output;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Answering {
    Line,
    Keystroke,
}

impl Answering {
    pub(crate) fn from_stdin() -> Self {
        if imp::stdin_assembles_lines() {
            Self::Line
        } else {
            Self::Keystroke
        }
    }

    /// Ask, then take one answer. Accepted when it begins with `y` or `Y`,
    /// which is what rpmatch accepts under LC_ALL=C; a locale whose YESEXPR
    /// differs is a known divergence.
    pub(crate) fn accepts<W: Write>(self, out: &mut Output<W>, new: &OsStr) -> bool {
        out.write_bytes(uucore::util_name().as_bytes());
        out.write_bytes(b": overwrite ");
        out.write_quoted(new);
        out.write_bytes(b"? ");
        out.flush();

        let answer = self.read(&mut io::stdin().lock(), out);
        match answer {
            Some(answer) => matches!(answer, b'y' | b'Y'),
            None => {
                // A prompt that reaches end of input declines, and says so:
                // C util-linux echoes a literal `n` and terminates the line
                // the prompt left open. The echo is not the locale's nostr,
                // and a typed decline never produces it.
                out.write_bytes(b"n\n");
                false
            }
        }
    }

    /// The first byte of the answer, or `None` at end of input - which includes
    /// a stdin that cannot be read at all, because C util-linux cannot tell
    /// those two apart either and assumes the same answer for both.
    ///
    /// Only the first byte is ever kept, so the buffer never grows with the
    /// input.
    fn read<R: BufRead, W: Write>(self, input: &mut R, out: &mut Output<W>) -> Option<u8> {
        let mut first = None;

        loop {
            let (leading, available, newline) = match input.fill_buf() {
                Ok([]) | Err(_) => break,
                Ok(chunk) => (
                    chunk.first().copied(),
                    chunk.len(),
                    chunk.iter().position(|byte| *byte == b'\n'),
                ),
            };

            first = first.or(leading);

            match self {
                Self::Keystroke => {
                    input.consume(available);
                    if first != Some(b'\n') {
                        out.write_bytes(b"\n");
                    }
                    return first;
                }
                Self::Line => match newline {
                    Some(end) => {
                        input.consume(end + 1);
                        return first;
                    }
                    None => input.consume(available),
                },
            }
        }

        // An answer the input ended without terminating is still an answer.
        first
    }
}

#[cfg(unix)]
mod imp {
    /// Anything that is not a terminal is read through a buffer that stops at a
    /// newline, and a terminal assembles lines exactly when ICANON is set.
    pub(super) fn stdin_assembles_lines() -> bool {
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();

        // SAFETY: tcgetattr writes the struct only on success, and this is the
        // only reference to it.
        let described = unsafe { libc::tcgetattr(libc::STDIN_FILENO, termios.as_mut_ptr()) } == 0;
        if !described {
            return true;
        }

        // SAFETY: tcgetattr returned success, so it initialized c_lflag.
        let c_lflag = unsafe { (*termios.as_ptr()).c_lflag };
        c_lflag & libc::ICANON != 0
    }
}

#[cfg(windows)]
mod imp {
    /// Windows has no termios. A console reads a line at a time unless a
    /// program clears ENABLE_LINE_INPUT, which nothing in this utility does.
    /// C util-linux does not run here, so there is nothing to conform to.
    pub(super) fn stdin_assembles_lines() -> bool {
        true
    }
}
