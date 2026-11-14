// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Command};
use clap::{Arg, ArgAction};
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("rev.md");
const USAGE: &str = help_usage!("rev.md");

mod options {
    pub const FILE: &str = "file";
    pub const ZERO: &str = "zero";
}

// uucore's get_locale_encoding uses LC_COLLATE and accepts empty values.
fn is_utf8_locale() -> bool {
    let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
        .into_iter()
        .filter_map(env::var_os)
        .find(|value| !value.is_empty());
    locale
        .as_deref()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let codeset = match name.split_once('.') {
                Some((_, codeset)) => codeset,
                None if cfg!(target_vendor = "apple") => name,
                None => return false,
            };
            let codeset = codeset.split_once('@').map_or(codeset, |(code, _)| code);
            codeset.eq_ignore_ascii_case("UTF-8") || codeset.eq_ignore_ascii_case("UTF8")
        })
}

// Checking while swapping avoids a separate scan in ASCII-only mode.
// The caller handles the middle so the loop can use complete words.
#[clippy::msrv = "1.70.0"]
fn reverse_blocks(buf: &mut [u8], ascii_only: bool) -> &mut [u8] {
    let len = buf.len();
    let (front, back) = buf.split_at_mut(len / 2);
    let mut done = 0;
    for (a, b) in front.chunks_exact_mut(8).zip(back.rchunks_exact_mut(8)) {
        let x = u64::from_ne_bytes(a.try_into().unwrap());
        let y = u64::from_ne_bytes(b.try_into().unwrap());
        if ascii_only && (x | y) & 0x8080_8080_8080_8080 != 0 {
            break;
        }
        a.copy_from_slice(&y.swap_bytes().to_ne_bytes());
        b.copy_from_slice(&x.swap_bytes().to_ne_bytes());
        done += 8;
    }
    &mut buf[done..len - done]
}

#[clippy::msrv = "1.70.0"]
fn utf8_ascii_prefix(buf: &[u8]) -> Option<usize> {
    // Reject invalid trailing sequences before scanning a long ASCII prefix.
    if buf.len() >= 128 {
        let tail = buf.len() - 8;
        if let Some(i) = buf[tail..].iter().rposition(|b| !b.is_ascii()) {
            // A valid UTF-8 character ending here occupies at most four bytes.
            let end = tail + i + 1;
            let lead = buf[end - 4..end].iter().rposition(|&b| b & 0xc0 != 0x80);
            if std::str::from_utf8(&buf[end - 4 + lead.unwrap_or(0)..end]).is_err() {
                return None;
            }
        }
    }
    // Skip validated ASCII blocks in the remaining ASCII and UTF-8 checks.
    let ascii_prefix = buf
        .chunks_exact(64)
        .take_while(|block| block.is_ascii())
        .count()
        * 64;
    let buf = &buf[ascii_prefix..];
    let remaining_prefix = buf.iter().position(|b| !b.is_ascii())?;
    std::str::from_utf8(&buf[remaining_prefix..])
        .is_ok()
        .then_some(ascii_prefix + remaining_prefix)
}

fn reverse_record(buf: &mut [u8], utf8: bool) {
    // A non-ASCII byte at either end would stop the block path immediately.
    let buf = if utf8 && buf.len() >= 128 && buf[0].is_ascii() && buf[buf.len() - 1].is_ascii() {
        reverse_blocks(buf, true)
    } else {
        buf
    };
    if utf8 {
        if let Some(ascii_prefix) = utf8_ascii_prefix(buf) {
            reverse_blocks(buf, false).reverse();
            // The ASCII prefix needs no character repair after byte reversal.
            let repair_end = buf.len() - ascii_prefix;
            let mut start = 0;
            for end in 0..repair_end {
                if buf[end] & 0xc0 != 0x80 {
                    buf[start..=end].reverse();
                    start = end + 1;
                }
            }
            return;
        }
    }
    buf.reverse();
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let files = matches.get_many::<String>(options::FILE);
    let zero = matches.get_flag(options::ZERO);

    let sep = if zero { b'\0' } else { b'\n' };
    let utf8 = is_utf8_locale();

    if let Some(files) = files {
        for path in files {
            let Ok(file) = std::fs::File::open(path) else {
                uucore::error::set_exit_code(1);
                uucore::show_error!("cannot open {path}: No such file or directory");
                continue;
            };
            if let Err(err) = rev_stream(file, sep, utf8) {
                uucore::error::set_exit_code(1);
                uucore::show_error!("cannot read {path}: {err}");
            }
        }
    } else {
        let stdin = std::io::stdin().lock();
        let _ = rev_stream(stdin, sep, utf8);
    }

    Ok(())
}

fn rev_stream(stream: impl Read, sep: u8, utf8: bool) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    let mut stream = BufReader::new(stream);
    let mut buf = Vec::with_capacity(4096);
    loop {
        buf.clear();
        stream.read_until(sep, &mut buf)?;
        if buf.last().copied() == Some(sep) {
            buf.pop();
            reverse_record(&mut buf, utf8);
            buf.push(sep);
            stdout.write_all(&buf)?;
        } else {
            reverse_record(&mut buf, utf8);
            stdout.write_all(&buf)?;
            break;
        }
    }
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE)
                .value_name("FILE")
                .help("Paths of files to reverse")
                .index(1)
                .action(ArgAction::Set)
                .num_args(1..),
        )
        .arg(
            Arg::new(options::ZERO)
                .short('0')
                .long("zero")
                .help("Zero termination. Use the byte '\\0' as line separator.")
                .action(ArgAction::SetTrue),
        )
}
