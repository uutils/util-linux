// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{
    fs::File,
    io::{stdin, Read},
};

use clap::{crate_version, Arg, ArgAction, Command};
use md5::{Digest, Md5};
use rand::RngCore;
use uucore::{
    error::{UResult, USimpleError},
    format_usage, help_about, help_usage,
    parser::parse_size,
};

mod options {
    pub const FILE: &str = "file";
    pub const MAX_SIZE: &str = "max-size";
    pub const VERBOSE: &str = "verbose";
}

const ABOUT: &str = help_about!("mcookie.md");
const USAGE: &str = help_usage!("mcookie.md");

const RANDOM_BYTES: usize = 128;
const MAX_DEFAULT: u64 = 4096;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let verbose = matches.get_flag(options::VERBOSE);

    let seed_files: Vec<&str> = matches
        .get_many::<String>(options::FILE)
        .unwrap_or_default()
        .map(|v| v.as_str())
        .collect();

    let max_size = if let Some(size_str) = matches.get_one::<String>(options::MAX_SIZE) {
        match parse_size::parse_size_u64(size_str) {
            Ok(0) => MAX_DEFAULT,
            Ok(size) => size,
            Err(_) => {
                return Err(USimpleError::new(1, "Failed to parse max-size value"));
            }
        }
    } else {
        MAX_DEFAULT
    };

    let mut hasher = Md5::new();

    for file_path in seed_files {
        let mut buffer: Vec<u8> = Vec::new();
        let input_name: &str;

        if file_path == "-" {
            input_name = "stdin";
            let stdin_handle = stdin();

            let mut limited_reader = stdin_handle.take(max_size);
            limited_reader.read_to_end(&mut buffer)?;
        } else {
            input_name = file_path;
            let open_result = File::open(file_path);
            if let Err(err) = open_result {
                eprintln!("mcookie: cannot open {file_path}: {err}");
                continue;
            }

            let f = open_result.unwrap();
            let mut limited_reader = f.take(max_size);
            limited_reader.read_to_end(&mut buffer)?;
        }

        if verbose {
            eprintln!("Got {} bytes from {}", buffer.len(), input_name);
        }

        hasher.update(&buffer);
    }

    let mut rng = rand::rng();
    let mut rand_bytes = [0u8; RANDOM_BYTES];
    rng.fill_bytes(&mut rand_bytes);

    hasher.update(rand_bytes);

    if verbose {
        eprintln!("Got {RANDOM_BYTES} bytes from randomness source");
    }

    let result = hasher.finalize();
    let output = result
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("");

    println!("{output}");

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
                .short('f')
                .long("file")
                .value_name("file")
                .action(ArgAction::Append)
                .help("use file as a cookie seed"),
        )
        .arg(
            Arg::new(options::MAX_SIZE)
                .short('m')
                .long("max-size")
                .value_name("num")
                .action(ArgAction::Set)
                .help("limit how much is read from seed files (supports B suffix or binary units: KiB, MiB, GiB, TiB)"),
        )
        .arg(
            Arg::new(options::VERBOSE)
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("explain what is being done"),
        )
}
