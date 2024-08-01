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

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let files = matches.get_many::<String>(options::FILE);
    let zero = matches.get_flag(options::ZERO);

    let sep = if zero { b'\0' } else { b'\n' };

    if let Some(files) = files {
        for path in files {
            let Ok(file) = std::fs::File::open(path) else {
                uucore::error::set_exit_code(1);
                uucore::show_error!("cannot open {path}: No such file or directory");
                continue;
            };
            if let Err(err) = rev_stream(file, sep) {
                uucore::error::set_exit_code(1);
                uucore::show_error!("cannot read {path}: {err}");
            }
        }
    } else {
        let stdin = std::io::stdin().lock();
        let _ = rev_stream(stdin, sep);
    }

    Ok(())
}

fn rev_stream(stream: impl Read, sep: u8) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    let mut stream = BufReader::new(stream);
    let mut buf = Vec::with_capacity(4096);
    loop {
        buf.clear();
        stream.read_until(sep, &mut buf)?;
        if buf.last().copied() == Some(sep) {
            buf.pop();
            buf.reverse();
            buf.push(sep);
            stdout.write_all(&buf)?;
        } else {
            buf.reverse();
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
