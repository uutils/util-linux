// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) ioprio isspace

use std::num::IntErrorKind;
use std::ops::RangeInclusive;

use crate::errors::NumericDetail;
use crate::ioprio::{CLASSES, IOPRIO_CLASS_IDLE, IOPRIO_CLASS_NONE};

/// The characters C's isspace() accepts, and which therefore may precede a
/// number. Note the vertical tab, which char::is_ascii_whitespace omits.
const BLANKS: [char; 6] = [' ', '\t', '\n', '\x0b', '\x0c', '\r'];

/// The ranges ionice(1) documents for -c and -n. The reference enforces
/// neither, and the kernel only reinterprets: it takes the class from three
/// bits and reads the level modulo eight. So a wider value selects a class or
/// level the caller never named, unless the wrap lands on a class the kernel
/// refuses outright. Refusing it here is a deliberate divergence from the
/// reference; see uutils/util-linux#624.
const DOCUMENTED_CLASSES: RangeInclusive<i32> = IOPRIO_CLASS_NONE..=IOPRIO_CLASS_IDLE;
const DOCUMENTED_LEVELS: RangeInclusive<i32> = 0..=7;

pub(crate) enum ClassError {
    Number(NumericDetail),
    UnknownName,
}

/// A decimal i32 with no leading blanks: an optional sign, then digits, then
/// nothing else. Overflow is kept apart from the other parse failures so that
/// a well-formed but oversized value is reported as such rather than as
/// malformed.
pub(crate) fn strict_i32(text: &str) -> Result<i32, NumericDetail> {
    text.parse::<i32>().map_err(|error| match error.kind() {
        IntErrorKind::PosOverflow | IntErrorKind::NegOverflow => NumericDetail::Overflow,
        _ => NumericDetail::Malformed,
    })
}

/// A decimal i32 that tolerates leading blanks, as -n, -p, -P and -u do.
pub(crate) fn blank_tolerant_i32(text: &str) -> Result<i32, NumericDetail> {
    strict_i32(text.trim_start_matches(|c: char| BLANKS.contains(&c)))
}

/// Hold a value to a documented range, naming the bounds so that the message
/// carries its own remedy. Applied after the parse, so a value too wide for
/// an i32 is still reported as the overflow it is.
fn within(value: i32, range: &RangeInclusive<i32>) -> Result<i32, NumericDetail> {
    if range.contains(&value) {
        Ok(value)
    } else {
        Err(NumericDetail::OutOfRange {
            low: *range.start(),
            high: *range.end(),
        })
    }
}

/// A -n value: a priority level, blanks tolerated, held to its range.
pub(crate) fn level(text: &str) -> Result<i32, NumericDetail> {
    within(blank_tolerant_i32(text)?, &DOCUMENTED_LEVELS)
}

/// A -c value. A leading digit selects the numeric form, so " 3" and "-3" take
/// the name branch and are reported as unknown class names rather than as
/// malformed numbers.
pub(crate) fn class(text: &str) -> Result<i32, ClassError> {
    if text.starts_with(|c: char| c.is_ascii_digit()) {
        strict_i32(text)
            .and_then(|value| within(value, &DOCUMENTED_CLASSES))
            .map_err(ClassError::Number)
    } else {
        CLASSES
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(text))
            .map(|&(_, value)| value)
            .ok_or(ClassError::UnknownName)
    }
}
