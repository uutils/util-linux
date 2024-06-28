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

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    let files = matches.get_many::<String>("file");

    match files {
        Some(files) => {
            for path in files {
                let file = match std::fs::File::open(path) {
                    Ok(val) => val,
                    Err(err) => {
                        uucore::error::set_exit_code(1);
                        uucore::show_error!("cannot open {}: {}", path, err);
                        continue;
                    }
                };
                if let Err(err) = rev_stream(file) {
                    uucore::error::set_exit_code(1);
                    uucore::show_error!("cannot read {}: {}", path, err);
                }
            }
        }
        None => {
            let stdin = std::io::stdin().lock();
            let _ = rev_stream(stdin);
        }
    }

    Ok(())
}

fn rev_stream(stream: impl Read) -> std::io::Result<()> {
    let mut stdout = std::io::stdout().lock();
    let mut stream = BufReader::new(stream);
    let mut buf = Vec::with_capacity(4096);
    loop {
        buf.clear();
        stream.read_until(b'\n', &mut buf)?;
        if buf.last().copied() != Some(b'\n') {
            buf.reverse();
            stdout.write_all(&buf)?;
            break;
        } else {
            buf.pop();
            buf.reverse();
            buf.push(b'\n');
            stdout.write_all(&buf)?;
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
            Arg::new("file")
                .value_name("FILE")
                .help("Paths of files to reverse")
                .index(1)
                .action(ArgAction::Set)
                .num_args(1..),
        )
}
