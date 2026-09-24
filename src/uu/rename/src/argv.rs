// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Where getopt would have stopped reading options.
//!
//! rename(1) permutes options with operands, so a flag is recognized anywhere
//! before a `--` - after all three operands, or between the substring and the
//! replacement. `POSIXLY_CORRECT` turns that off, and its mere presence does
//! it whatever the value: scanning stops at the first argument that is not an
//! option, and everything from there on is a filename however much it looks
//! like a flag.
//!
//! clap has no such concept, so rather than teach it one the argv is
//! terminated where getopt would have stopped and clap is handed the result.

use std::env;
use std::ffi::{OsStr, OsString};

const TERMINATOR: &str = "--";

pub(crate) fn collect_getopt_argv(args: impl uucore::Args) -> Vec<OsString> {
    terminate_at_first_operand(args.collect(), env::var_os("POSIXLY_CORRECT").is_some())
}

/// A bare `-` is an operand and not an option, so it stops the scan like any
/// other name. `--` is longer than one unit and so answers true here, which is
/// why the caller tests for it first.
fn is_option(arg: &OsStr) -> bool {
    // ASCII survives this on every platform: the encoded form keeps ASCII bytes
    // as themselves, which is all a leading `-` needs.
    matches!(arg.as_encoded_bytes(), [b'-', _, ..])
}

fn terminate_at_first_operand(mut argv: Vec<OsString>, posixly_correct: bool) -> Vec<OsString> {
    if !posixly_correct {
        return argv;
    }

    let mut operand_at = None;
    // argv[0] is the utility's own name and is never scanned.
    for (index, arg) in argv.iter().enumerate().skip(1) {
        if arg == TERMINATOR {
            // getopt consumes this one itself, so there is nothing to insert
            // and nothing after it to protect.
            return argv;
        }
        if !is_option(arg) {
            operand_at = Some(index);
            break;
        }
    }

    if let Some(index) = operand_at {
        argv.insert(index, OsString::from(TERMINATOR));
    }

    argv
}
