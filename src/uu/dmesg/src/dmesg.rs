// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use regex::Regex;
use std::fs;
use uucore::error::UResult;

mod json;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let mut dmesg = Dmesg::new();
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    if let Some(kmsg_file) = matches.get_one::<String>(options::KMSG_FILE) {
        dmesg.kmsg_file = kmsg_file;
    }
    if matches.get_flag(options::JSON) {
        dmesg.output_format = OutputFormat::Json;
    }
    dmesg.parse()?.print();
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .arg(
            Arg::new(options::KMSG_FILE)
                .short('K')
                .long("kmsg-file")
                .help("use the file in kmsg format")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long("json")
                .help("use JSON output format")
                .action(ArgAction::SetTrue),
        )
}

mod options {
    pub const KMSG_FILE: &str = "kmsg-file";
    pub const JSON: &str = "json";
}

struct Dmesg<'a> {
    kmsg_file: &'a str,
    output_format: OutputFormat,
    records: Option<Vec<Record>>,
}

impl Dmesg<'_> {
    fn new() -> Self {
        Dmesg {
            kmsg_file: "/dev/kmsg",
            output_format: OutputFormat::Normal,
            records: None,
        }
    }

    fn parse(mut self) -> UResult<Self> {
        let mut records = vec![];
        let re = Self::record_regex();
        let lines = self.read_lines_from_kmsg_file()?;
        for line in lines {
            for (_, [pri_fac, seq, time, msg]) in re.captures_iter(&line).map(|c| c.extract()) {
                records.push(Record::from_str_fields(pri_fac, seq, time, msg.to_string()));
            }
        }
        self.records = Some(records);
        Ok(self)
    }

    fn record_regex() -> Regex {
        let valid_number_pattern = "0|[1-9][0-9]*";
        let additional_fields_pattern = ",^[,;]*";
        let record_pattern = format!(
            "(?m)^({0}),({0}),({0}),.(?:{1})*;(.*)$",
            valid_number_pattern, additional_fields_pattern
        );
        Regex::new(&record_pattern).expect("invalid regex.")
    }

    fn read_lines_from_kmsg_file(&self) -> UResult<Vec<String>> {
        let mut lines = vec![];
        let mut line = vec![];
        for byte in fs::read(self.kmsg_file)? {
            if byte == 0 {
                lines.push(String::from_utf8_lossy(&line).to_string());
                line.clear();
            } else {
                line.push(byte);
            }
        }
        Ok(lines)
    }

    fn print(&self) {
        match self.output_format {
            OutputFormat::Json => self.print_json(),
            OutputFormat::Normal => unimplemented!(),
        }
    }

    fn print_json(&self) {
        if let Some(records) = &self.records {
            println!("{}", json::serialize_records(records));
        }
    }
}

enum OutputFormat {
    Normal,
    Json,
}

struct Record {
    priority_facility: u32,
    _sequence: u64,
    timestamp_us: u64,
    message: String,
}

impl Record {
    fn from_str_fields(pri_fac: &str, seq: &str, time: &str, msg: String) -> Record {
        let pri_fac = str::parse(pri_fac);
        let seq = str::parse(seq);
        let time = str::parse(time);
        match (pri_fac, seq, time) {
            (Ok(pri_fac), Ok(seq), Ok(time)) => Record {
                priority_facility: pri_fac,
                _sequence: seq,
                timestamp_us: time,
                message: msg,
            },
            _ => panic!("parse error."),
        }
    }
}
