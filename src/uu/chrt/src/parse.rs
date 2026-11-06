// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) isspace strtol strtoull

//! chrt(1) reads its numbers three different ways, and the differences decide
//! what a token means, not just whether it is valid.

use std::ffi::OsStr;

/// The bytes C's isspace() accepts in the C locale, which strtol skips before
/// the number, the vertical tab included: u8::is_ascii_whitespace omits it.
const BLANKS: [u8; 6] = [b' ', b'\t', b'\n', 0x0b, 0x0c, b'\r'];

#[derive(Debug, Clone, Copy)]
pub(crate) enum NumericDetail {
    Malformed,
    Overflow,
}

/// An exec-mode priority, ASCII digits only: `None` when the token is not a
/// priority, `Some(Err)` when it is all digits but does not fit an i32.
#[cfg(target_os = "linux")]
pub(crate) fn digits_i32(text: &OsStr) -> Option<Result<i32, NumericDetail>> {
    let bytes = text.as_encoded_bytes();
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(decimal(bytes))
}

/// A pid, or a priority under --pid, as strtol base 10 reads it with the whole
/// token consumed: leading blanks, an optional sign, digits, then the end.
#[cfg(target_os = "linux")]
pub(crate) fn strtol_i32(text: &OsStr) -> Result<i32, NumericDetail> {
    let (negative, digits) = split_number(text)?;
    let magnitude: i64 = decimal(digits)?;
    let value = if negative { -magnitude } else { magnitude };
    i32::try_from(value).map_err(|_| NumericDetail::Overflow)
}

/// A -T, -P or -D value, as strtoull base 10 reads it except that a negative
/// number is out of range rather than wrapped, and -0 is 0.
fn strtoull_u64(text: &OsStr) -> Result<u64, NumericDetail> {
    let (negative, digits) = split_number(text)?;
    match decimal(digits)? {
        0 => Ok(0),
        _ if negative => Err(NumericDetail::Overflow),
        value => Ok(value),
    }
}

pub(crate) fn nanoseconds(text: &str) -> Result<u64, &'static str> {
    strtoull_u64(OsStr::new(text)).map_err(|_| "expected an unsigned 64-bit number")
}

fn split_number(text: &OsStr) -> Result<(bool, &[u8]), NumericDetail> {
    let bytes = text.as_encoded_bytes();
    let start = bytes
        .iter()
        .position(|byte| !BLANKS.contains(byte))
        .unwrap_or(bytes.len());
    let (negative, digits) = match bytes[start..] {
        [b'-', ref rest @ ..] => (true, rest),
        [b'+', ref rest @ ..] => (false, rest),
        ref rest => (false, rest),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return Err(NumericDetail::Malformed);
    }
    Ok((negative, digits))
}

/// Digits only, already checked; the one failure left is overflow.
fn decimal<T: std::str::FromStr>(digits: &[u8]) -> Result<T, NumericDetail> {
    std::str::from_utf8(digits)
        .map_err(|_| NumericDetail::Malformed)?
        .parse()
        .map_err(|_| NumericDetail::Overflow)
}
