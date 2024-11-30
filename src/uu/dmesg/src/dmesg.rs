// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use regex::Regex;
use std::fs;
use uucore::{
    error::{FromIo, UResult, USimpleError},
    format_usage, help_about, help_usage,
};

mod json;
mod time_formatter;

const ABOUT: &str = help_about!("dmesg.md");
const USAGE: &str = help_usage!("dmesg.md");

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
    if let Some(time_format) = matches.get_one::<String>(options::TIME_FORMAT) {
        dmesg.time_format = match &time_format[..] {
            "delta" => TimeFormat::Delta,
            "reltime" => TimeFormat::Reltime,
            "ctime" => TimeFormat::Ctime,
            "notime" => TimeFormat::Notime,
            "iso" => TimeFormat::Iso,
            "raw" => TimeFormat::Raw,
            _ => {
                return Err(USimpleError::new(
                    1,
                    format!("unknown time format: {time_format}"),
                ))
            }
        };
    }
    dmesg.parse()?.print();
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .override_usage(format_usage(USAGE))
        .about(ABOUT)
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
        .arg(
            Arg::new(options::TIME_FORMAT)
                .long("time-format")
                .help(
                    "show timestamp using the given format:\n".to_string()
                        + "  [delta|reltime|ctime|notime|iso|raw]",
                )
                .action(ArgAction::Set),
        )
}

mod options {
    pub const KMSG_FILE: &str = "kmsg-file";
    pub const JSON: &str = "json";
    pub const TIME_FORMAT: &str = "time-format";
}

struct Dmesg<'a> {
    kmsg_file: &'a str,
    output_format: OutputFormat,
    time_format: TimeFormat,
    records: Option<Vec<Record>>,
}

impl Dmesg<'_> {
    fn new() -> Self {
        Dmesg {
            kmsg_file: "/dev/kmsg",
            output_format: OutputFormat::Normal,
            time_format: TimeFormat::Raw,
            records: None,
        }
    }

    fn parse(mut self) -> UResult<Self> {
        let mut records = vec![];
        let re = Self::record_regex();
        let lines = self.read_lines_from_kmsg_file()?;
        for line in lines {
            for (_, [pri_fac, seq, time, msg]) in re.captures_iter(&line).map(|c| c.extract()) {
                records.push(Record::from_str_fields(
                    pri_fac,
                    seq,
                    time,
                    msg.to_string(),
                )?);
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
        let kmsg_bytes = fs::read(self.kmsg_file)
            .map_err_context(|| format!("cannot open {}", self.kmsg_file))?;
        let lines = kmsg_bytes
            .split(|&byte| byte == 0)
            .map(|line| String::from_utf8_lossy(line).to_string())
            .collect();
        Ok(lines)
    }

    fn print(&self) {
        match self.output_format {
            OutputFormat::Json => self.print_json(),
            OutputFormat::Normal => self.print_normal(),
        }
    }

    fn print_json(&self) {
        if let Some(records) = &self.records {
            println!("{}", json::serialize_records(records));
        }
    }

    fn print_normal(&self) {
        if let Some(records) = &self.records {
            let mut reltime_formatter = time_formatter::ReltimeFormatter::new();
            let mut delta_formatter = time_formatter::DeltaFormatter::new();
            for record in records {
                match self.time_format {
                    TimeFormat::Delta => {
                        print!("[{}] ", delta_formatter.format(record.timestamp_us))
                    }
                    TimeFormat::Reltime => {
                        print!("[{}] ", reltime_formatter.format(record.timestamp_us))
                    }
                    TimeFormat::Ctime => {
                        print!("[{}] ", time_formatter::ctime(record.timestamp_us))
                    }
                    TimeFormat::Iso => {
                        print!("{} ", time_formatter::iso(record.timestamp_us))
                    }
                    TimeFormat::Raw => {
                        print!("[{}] ", time_formatter::raw(record.timestamp_us))
                    }
                    TimeFormat::Notime => (),
                }
                println!("{}", record.message);
            }
        }
    }
}

enum OutputFormat {
    Normal,
    Json,
}

enum TimeFormat {
    Delta,
    Reltime,
    Ctime,
    Notime,
    Iso,
    Raw,
}

struct Record {
    priority_facility: u32,
    _sequence: u64,
    timestamp_us: i64,
    message: String,
}

impl Record {
    fn from_str_fields(pri_fac: &str, seq: &str, time: &str, msg: String) -> UResult<Record> {
        let pri_fac = str::parse(pri_fac);
        let seq = str::parse(seq);
        let time = str::parse(time);
        match (pri_fac, seq, time) {
            (Ok(pri_fac), Ok(seq), Ok(time)) => Ok(Record {
                priority_facility: pri_fac,
                _sequence: seq,
                timestamp_us: time,
                message: msg,
            }),
            _ => Err(USimpleError::new(1, "Failed to parse record field(s)")),
        }
    }
}
