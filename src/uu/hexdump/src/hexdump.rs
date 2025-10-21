// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};

use clap::{crate_version, Arg, ArgAction, Command};
use uucore::{
    error::{set_exit_code, UResult, USimpleError},
    format_usage, help_about, help_usage,
    parser::parse_size,
};

struct ChainedFileReader {
    file_paths: Vec<String>,
    current_file: Option<BufReader<File>>,
    current_file_index: usize,
    remaining_bytes: Option<u64>,
    open_error_count: usize,
}

impl ChainedFileReader {
    fn new(file_paths: Vec<String>, length_limit: Option<u64>) -> Self {
        Self {
            file_paths,
            current_file: None,
            current_file_index: 0,
            remaining_bytes: length_limit,
            open_error_count: 0,
        }
    }

    fn ensure_current_file(&mut self) -> bool {
        if self.current_file.is_some() {
            return true;
        }

        while self.current_file_index < self.file_paths.len() {
            let file_path = &self.file_paths[self.current_file_index];
            match File::open(file_path) {
                Ok(file) => {
                    self.current_file = Some(BufReader::new(file));
                    return true;
                }
                Err(e) => {
                    uucore::show_error!("cannot open '{}': {}", file_path, e);
                    set_exit_code(1);
                    self.open_error_count += 1;
                    self.current_file_index += 1;
                }
            }
        }
        false
    }

    fn current_file(&mut self) -> &mut BufReader<File> {
        self.current_file.as_mut().unwrap()
    }

    // Note: Callers assume partial reads won't happen (except on EOF)
    fn read(&mut self, buf: &mut [u8]) -> usize {
        if self.remaining_bytes == Some(0) {
            return 0;
        }

        let mut offset = 0;
        while self.ensure_current_file() {
            let remaining_in_buffer = (buf.len() - offset) as u64;
            let nbytes = remaining_in_buffer.min(self.remaining_bytes.unwrap_or(u64::MAX)) as usize;
            if nbytes == 0 {
                break;
            }

            match self.current_file().read(&mut buf[offset..offset + nbytes]) {
                Ok(0) => {
                    self.current_file = None;
                    self.current_file_index += 1;
                }
                Ok(n) => {
                    offset += n;
                    self.remaining_bytes = self.remaining_bytes.map(|x| x - n as u64);
                }
                Err(e) => {
                    uucore::show_error!(
                        "cannot read '{}': {}",
                        self.file_paths[self.current_file_index],
                        e
                    );
                    set_exit_code(1);
                    self.current_file = None;
                    self.current_file_index += 1;
                }
            }
        }

        offset
    }

    fn skip_bytes(&mut self, bytes_to_skip: u64) {
        let mut remaining = bytes_to_skip;

        while remaining > 0 && self.ensure_current_file() {
            match self.current_file().seek(SeekFrom::End(0)) {
                Ok(file_size) => {
                    if remaining >= file_size {
                        remaining -= file_size;
                        self.current_file = None;
                        self.current_file_index += 1;
                    } else {
                        match self.current_file().seek(SeekFrom::Start(remaining)) {
                            Ok(_) => return,
                            Err(e) => {
                                uucore::show_error!(
                                    "cannot seek '{}': {}",
                                    self.file_paths[self.current_file_index],
                                    e
                                );
                                set_exit_code(1);
                                self.current_file = None;
                                self.current_file_index += 1;
                            }
                        }
                    }
                }
                Err(_) => {
                    // This file doesn't support seeking, fall back to dummy reads
                    while remaining > 0 {
                        let mut dummy_buf = vec![0u8; remaining.min(65536) as usize];
                        match self.current_file().read(&mut dummy_buf) {
                            Ok(0) => {
                                self.current_file = None;
                                self.current_file_index += 1;
                                break;
                            }
                            Ok(n) => remaining -= n as u64,
                            Err(e) => {
                                uucore::show_error!(
                                    "cannot read '{}': {}",
                                    self.file_paths[self.current_file_index],
                                    e
                                );
                                set_exit_code(1);
                                self.current_file = None;
                                self.current_file_index += 1;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
}

const ABOUT: &str = help_about!("hexdump.md");
const USAGE: &str = help_usage!("hexdump.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisplayFormat {
    Canonical,          // -C: hex + ASCII display
    OneByteOctal,       // -b: one-byte octal display
    OneByteHex,         // -X: one-byte hex display
    OneByteChar,        // -c: one-byte character display
    TwoBytesDecimal,    // -d: two-byte decimal display
    TwoBytesOctal,      // -o: two-byte octal display
    TwoBytesHex,        // -x: two-byte hex display
    TwoBytesHexDefault, // default: two-byte hex display with compact spacing
}

#[derive(Debug)]
struct HexdumpOptions {
    formats: Vec<DisplayFormat>,
    length: Option<u64>,
    skip: u64,
    no_squeezing: bool,
    files: Vec<String>,
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    // Collect formats in same order they were in command line
    let format_mappings = [
        ("canonical", DisplayFormat::Canonical),
        ("one-byte-octal", DisplayFormat::OneByteOctal),
        ("one-byte-hex", DisplayFormat::OneByteHex),
        ("one-byte-char", DisplayFormat::OneByteChar),
        ("two-bytes-decimal", DisplayFormat::TwoBytesDecimal),
        ("two-bytes-octal", DisplayFormat::TwoBytesOctal),
        ("two-bytes-hex", DisplayFormat::TwoBytesHex),
    ];

    let mut formats = BTreeMap::new();
    for (id, format) in format_mappings {
        if matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine) {
            for index in matches.indices_of(id).unwrap() {
                formats.insert(index, format);
            }
        }
    }
    if formats.is_empty() {
        formats.insert(0, DisplayFormat::TwoBytesHexDefault);
    }

    let options = HexdumpOptions {
        formats: formats.into_values().collect(),
        length: matches.get_one::<u64>("length").copied(),
        skip: matches.get_one::<u64>("skip").copied().unwrap_or(0),
        no_squeezing: matches.get_flag("no-squeezing"),
        // TODO: /dev/stdin doesn't work on Windows
        files: matches
            .get_many::<String>("files")
            .map(|files| files.cloned().collect())
            .unwrap_or_else(|| vec!["/dev/stdin".to_string()]),
    };
    run_hexdump(options)
}

fn run_hexdump(options: HexdumpOptions) -> UResult<()> {
    let mut reader = ChainedFileReader::new(options.files.clone(), options.length);
    reader.skip_bytes(options.skip);

    let mut offset = options.skip;
    let mut last_line: Vec<u8> = Vec::new();
    let mut squeezing = false;

    loop {
        let mut line_data = [0u8; 16];
        let bytes_read = reader.read(&mut line_data);
        if bytes_read == 0 {
            break;
        }
        let line_data = &line_data[..bytes_read];

        // Handle line squeezing (consolidate identical lines into one '*')
        if !options.no_squeezing && last_line == line_data {
            if !squeezing {
                println!("*");
                squeezing = true;
            }
        } else {
            for format in &options.formats {
                print_hexdump_line(*format, offset, line_data);
            }
            last_line.clear();
            last_line.extend_from_slice(line_data);
            squeezing = false;
        }

        offset += line_data.len() as u64;
    }

    if offset != 0 {
        // Formatting of end offset must match last format's
        print_offset(offset, *options.formats.last().unwrap());
        println!();
    }

    if reader.open_error_count == reader.file_paths.len() {
        Err(USimpleError::new(1, "all input file arguments failed"))
    } else {
        Ok(())
    }
}

fn print_hexdump_line(format: DisplayFormat, offset: u64, line_data: &[u8]) {
    print_offset(offset, format);
    match format {
        DisplayFormat::Canonical => print_canonical(line_data),
        DisplayFormat::OneByteOctal => print_bytes(line_data, |b| print!(" {:03o}", b)),
        DisplayFormat::OneByteHex => print_bytes(line_data, |b| print!("  {:02x}", b)),
        DisplayFormat::OneByteChar => print_bytes(line_data, print_char_byte),
        DisplayFormat::TwoBytesDecimal => print_words(line_data, 8, |w| print!("   {:05}", w)),
        DisplayFormat::TwoBytesOctal => print_words(line_data, 8, |w| print!("  {:06o}", w)),
        DisplayFormat::TwoBytesHex => print_words(line_data, 8, |w| print!("    {:04x}", w)),
        DisplayFormat::TwoBytesHexDefault => print_words(line_data, 5, |w| print!(" {:04x}", w)),
    }
}

fn print_offset(offset: u64, format: DisplayFormat) {
    if format == DisplayFormat::Canonical {
        print!("{:08x}", offset);
    } else {
        print!("{:07x}", offset);
    }
}

fn print_canonical(line_data: &[u8]) {
    print!("  ");

    for i in 0..16 {
        // Extra space between halfs
        if i == 8 {
            print!(" ");
        }

        if i < line_data.len() {
            print!("{:02x} ", line_data[i]);
        } else {
            print!("   ");
        }
    }

    print!(" |");

    for &byte in line_data {
        if byte.is_ascii_graphic() || byte == b' ' {
            print!("{}", byte as char);
        } else {
            print!(".");
        }
    }
    println!("|");
}

fn print_bytes<F>(line_data: &[u8], byte_printer: F)
where
    F: Fn(u8),
{
    for &byte in line_data {
        byte_printer(byte);
    }
    // Original hexdump pads all lines to same length
    println!("{:width$}", "", width = (16 - line_data.len()) * 4);
}

fn print_char_byte(byte: u8) {
    match byte {
        b'\0' => print!("  \\0"),
        b'\x07' => print!("  \\a"),
        b'\x08' => print!("  \\b"),
        b'\t' => print!("  \\t"),
        b'\n' => print!("  \\n"),
        b'\x0B' => print!("  \\v"),
        b'\x0C' => print!("  \\f"),
        b'\r' => print!("  \\r"),
        b if b.is_ascii_graphic() || b == b' ' => print!("   {}", b as char),
        b => print!(" {:03o}", b),
    }
}

fn print_words<F>(line_data: &[u8], chars_per_word: usize, word_printer: F)
where
    F: Fn(u16),
{
    for i in 0..(line_data.len() / 2) {
        word_printer(u16::from_le_bytes([line_data[i * 2], line_data[i * 2 + 1]]));
    }

    if line_data.len() % 2 == 1 {
        word_printer(*line_data.last().unwrap() as u16);
    }

    // Original hexdump pads all lines to same length
    let word_count = line_data.len().div_ceil(2);
    println!("{:width$}", "", width = (8 - word_count) * chars_per_word);
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("one-byte-octal")
                .short('b')
                .long("one-byte-octal")
                .help("one-byte octal display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("one-byte-hex")
                .short('X')
                .long("one-byte-hex")
                .help("one-byte hexadecimal display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("one-byte-char")
                .short('c')
                .long("one-byte-char")
                .help("one-byte character display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("canonical")
                .short('C')
                .long("canonical")
                .help("canonical hex+ASCII display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("two-bytes-decimal")
                .short('d')
                .long("two-bytes-decimal")
                .help("two-byte decimal display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("two-bytes-octal")
                .short('o')
                .long("two-bytes-octal")
                .help("two-byte octal display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("two-bytes-hex")
                .short('x')
                .long("two-bytes-hex")
                .help("two-byte hexadecimal display")
                .action(ArgAction::Append)
                .num_args(0)
                .default_value("0")
                .default_missing_value("0"),
        )
        .arg(
            Arg::new("length")
                .short('n')
                .long("length")
                .help("interpret only length bytes of input")
                .action(ArgAction::Set)
                .num_args(1)
                .value_parser(|s: &str| -> Result<u64, String> {
                    parse_size::parse_size_u64(s).map_err(|e| format!("invalid length: {}", e))
                }),
        )
        .arg(
            Arg::new("skip")
                .short('s')
                .long("skip")
                .help("skip offset bytes from the beginning")
                .action(ArgAction::Set)
                .num_args(1)
                .value_parser(|s: &str| -> Result<u64, String> {
                    parse_size::parse_size_u64(s).map_err(|e| format!("invalid skip: {}", e))
                }),
        )
        .arg(
            Arg::new("no-squeezing")
                .short('v')
                .long("no-squeezing")
                .help("output identical lines")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("files")
                .help("input files")
                .action(ArgAction::Append)
                .num_args(0..),
        )
}
