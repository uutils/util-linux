// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use chrono::{DateTime, FixedOffset};
use clap::{crate_version, Arg, ArgAction, Command};
use regex::Regex;
use std::{
    collections::HashSet,
    fs::File,
    fs::OpenOptions,
    hash::Hash,
    io::{BufRead, BufReader, ErrorKind},
    sync::OnceLock,
};

#[cfg(not(target_os = "windows"))]
use std::{os::fd::AsRawFd, os::unix::fs::OpenOptionsExt};

use uucore::{
    error::{FromIo, UError, UIoError, UResult, USimpleError},
    format_usage, help_about, help_usage,
};

#[cfg(not(target_os = "windows"))]
use uucore::libc;

mod json;
mod time_formatter;

const ABOUT: &str = help_about!("dmesg.md");
const USAGE: &str = help_usage!("dmesg.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let mut dmesg = Dmesg::new();
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
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
    if let Some(list_args) = matches.get_many::<String>(options::FACILITY) {
        let mut facility_filters = HashSet::new();
        for list in list_args {
            for arg in list.split(',') {
                let facility = match arg {
                    "kern" => Facility::Kern,
                    "user" => Facility::User,
                    "mail" => Facility::Mail,
                    "daemon" => Facility::Daemon,
                    "auth" => Facility::Auth,
                    "syslog" => Facility::Syslog,
                    "lpr" => Facility::Lpr,
                    "news" => Facility::News,
                    "uucp" => Facility::Uucp,
                    "cron" => Facility::Cron,
                    "authpriv" => Facility::Authpriv,
                    "ftp" => Facility::Ftp,
                    "res0" => Facility::Res0,
                    "res1" => Facility::Res1,
                    "res2" => Facility::Res2,
                    "res3" => Facility::Res3,
                    "local0" => Facility::Local0,
                    "local1" => Facility::Local1,
                    "local2" => Facility::Local2,
                    "local3" => Facility::Local3,
                    "local4" => Facility::Local4,
                    "local5" => Facility::Local5,
                    "local6" => Facility::Local6,
                    "local7" => Facility::Local7,
                    _ => return Err(USimpleError::new(1, format!("unknown facility '{arg}'"))),
                };
                facility_filters.insert(facility);
            }
        }
        dmesg.facility_filters = Some(facility_filters);
    }
    if let Some(list_args) = matches.get_many::<String>(options::LEVEL) {
        let mut level_filters = HashSet::new();
        for list in list_args {
            for arg in list.split(',') {
                let level = match arg {
                    "emerg" => Level::Emerg,
                    "alert" => Level::Alert,
                    "crit" => Level::Crit,
                    "err" => Level::Err,
                    "warn" => Level::Warn,
                    "notice" => Level::Notice,
                    "info" => Level::Info,
                    "debug" => Level::Debug,
                    _ => return Err(USimpleError::new(1, format!("unknown level '{arg}'"))),
                };
                level_filters.insert(level);
            }
        }
        dmesg.level_filters = Some(level_filters);
    }
    if let Some(since) = matches.get_one::<String>(options::SINCE) {
        dmesg.since_filter = Some(time_formatter::parse_datetime(since)?);
    }
    if let Some(until) = matches.get_one::<String>(options::UNTIL) {
        dmesg.until_filter = Some(time_formatter::parse_datetime(until)?);
    }
    if let Some(kmsg_file) = matches.get_one::<String>(options::KMSG_FILE) {
        dmesg.kmsg_file = kmsg_file;
        dmesg.kmsg_record_separator = 0;
    } else if cfg!(target_os = "windows") {
        return Err(USimpleError::new(1, "Windows requires the use of '-K'"));
    }
    dmesg.print()?;
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
        .arg(
            Arg::new(options::FACILITY)
                .short('f')
                .long("facility")
                .help("restrict output to defined facilities")
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new(options::LEVEL)
                .short('l')
                .long("level")
                .help("restrict output to defined levels")
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new(options::SINCE)
                .long("since")
                .help("display the lines since the specified time")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::UNTIL)
                .long("until")
                .help("display the lines until the specified time")
                .action(ArgAction::Set),
        )
}

mod options {
    pub const KMSG_FILE: &str = "kmsg-file";
    pub const JSON: &str = "json";
    pub const TIME_FORMAT: &str = "time-format";
    pub const FACILITY: &str = "facility";
    pub const LEVEL: &str = "level";
    pub const SINCE: &str = "since";
    pub const UNTIL: &str = "until";
}

struct Dmesg<'a> {
    kmsg_file: &'a str,
    kmsg_record_separator: u8,
    output_format: OutputFormat,
    time_format: TimeFormat,
    facility_filters: Option<HashSet<Facility>>,
    level_filters: Option<HashSet<Level>>,
    since_filter: Option<chrono::DateTime<chrono::FixedOffset>>,
    until_filter: Option<chrono::DateTime<chrono::FixedOffset>>,
}

impl Dmesg<'_> {
    fn new() -> Self {
        Dmesg {
            kmsg_file: "/dev/kmsg",
            kmsg_record_separator: b'\n',
            output_format: OutputFormat::Normal,
            time_format: TimeFormat::Raw,
            facility_filters: None,
            level_filters: None,
            since_filter: None,
            until_filter: None,
        }
    }

    fn print(&self) -> UResult<()> {
        match self.output_format {
            OutputFormat::Json => self.print_json(),
            OutputFormat::Normal => self.print_normal(),
        }
    }

    fn print_json(&self) -> UResult<()> {
        let records: UResult<Vec<Record>> = self.try_filtered_iter()?.collect();
        println!("{}", json::serialize_records(&records?));
        Ok(())
    }

    fn print_normal(&self) -> UResult<()> {
        let mut reltime_formatter = time_formatter::ReltimeFormatter::new();
        let mut delta_formatter = time_formatter::DeltaFormatter::new();
        for record in self.try_filtered_iter()? {
            let record = record?;
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
        Ok(())
    }

    fn try_filtered_iter(&self) -> UResult<impl Iterator<Item = UResult<Record>> + '_> {
        Ok(self
            .try_iter()?
            .filter(Self::is_record_in_set(&self.facility_filters))
            .filter(Self::is_record_in_set(&self.level_filters))
            .filter(Self::is_record_since(&self.since_filter))
            .filter(Self::is_record_until(&self.until_filter)))
    }

    fn try_iter(&self) -> UResult<RecordIterator> {
        let mut open_option = OpenOptions::new();
        open_option.read(true);

        #[cfg(not(target_os = "windows"))]
        open_option.custom_flags(libc::O_NONBLOCK);

        let file = open_option
            .open(self.kmsg_file)
            .map_err_context(|| format!("cannot open {}", self.kmsg_file))?;

        #[cfg(not(target_os = "windows"))]
        {
            let fd = file.as_raw_fd();
            unsafe { libc::lseek(fd, 0, libc::SEEK_DATA) };
        }

        let file_reader = BufReader::new(file);
        Ok(RecordIterator {
            file_reader,
            kmsg_record_separator: self.kmsg_record_separator,
        })
    }

    fn is_record_in_set<T>(
        set: &Option<HashSet<T>>,
    ) -> impl Fn(&Result<Record, Box<dyn UError>>) -> bool + '_
    where
        T: From<u32> + Eq + Hash,
    {
        move |record: &UResult<Record>| match (record, set) {
            (Ok(record), Some(set)) => set.contains(&T::from(record.priority_facility)),
            _ => true,
        }
    }

    fn is_record_since(
        since: &Option<DateTime<FixedOffset>>,
    ) -> impl Fn(&UResult<Record>) -> bool + '_ {
        move |record: &UResult<Record>| match (record, since) {
            (Ok(record), Some(since)) => {
                let time =
                    time_formatter::datetime_from_microseconds_since_boot(record.timestamp_us);
                time >= *since
            }
            _ => true,
        }
    }

    fn is_record_until(
        until: &Option<DateTime<FixedOffset>>,
    ) -> impl Fn(&UResult<Record>) -> bool + '_ {
        move |record: &UResult<Record>| match (record, until) {
            (Ok(record), Some(until)) => {
                let time =
                    time_formatter::datetime_from_microseconds_since_boot(record.timestamp_us);
                time <= *until
            }
            _ => true,
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

#[derive(Eq, Hash, PartialEq)]
enum Facility {
    Kern,
    User,
    Mail,
    Daemon,
    Auth,
    Syslog,
    Lpr,
    News,
    Uucp,
    Cron,
    Authpriv,
    Ftp,
    Res0,
    Res1,
    Res2,
    Res3,
    Local0,
    Local1,
    Local2,
    Local3,
    Local4,
    Local5,
    Local6,
    Local7,
    Unknown,
}

#[derive(Eq, Hash, PartialEq)]
enum Level {
    Emerg,
    Alert,
    Crit,
    Err,
    Warn,
    Notice,
    Info,
    Debug,
    Unknown,
}

struct RecordIterator {
    file_reader: BufReader<File>,
    kmsg_record_separator: u8,
}

impl Iterator for RecordIterator {
    type Item = UResult<Record>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_record_line() {
            Err(e) => Some(Err(e)),
            Ok(None) => None,
            Ok(Some(line)) => match self.parse_record(&line) {
                None => self.next(),
                Some(record) => Some(Ok(record)),
            },
        }
    }
}

impl RecordIterator {
    fn read_record_line(&mut self) -> UResult<Option<String>> {
        let mut buf = vec![];
        match self
            .file_reader
            .read_until(self.kmsg_record_separator, &mut buf)
        {
            /*
             * - a read(2) from /dev/kmsg returns WouldBlock if there aren't
             *   any new record
             * - a read(2) from a file returns 0 if the we reached the end
             * In these cases return Ok(None)
             */
            Ok(0) => Ok(None),
            Ok(_) => Ok(Some(String::from_utf8_lossy(&buf).to_string())),
            Err(e) if e.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(Box::new(UIoError::from(e))),
        }
    }

    fn parse_record(&self, record_line: &str) -> Option<Record> {
        record_regex()
            .captures_iter(record_line)
            .map(|c| c.extract())
            .filter_map(|(_, [pri_fac, seq, time, msg])| {
                Record::from_str_fields(pri_fac, seq, time, msg.to_string()).ok()
            })
            .next()
    }
}

fn record_regex() -> &'static Regex {
    RECORD_REGEX.get_or_init(|| {
        let valid_number_pattern = "0|[1-9][0-9]*";
        let additional_fields_pattern = ",^[,;]*";
        let record_pattern = format!(
            "(?m)^({valid_number_pattern}),({valid_number_pattern}),({valid_number_pattern}),.(?:{additional_fields_pattern})*;(.*)$"
        );
        Regex::new(&record_pattern).expect("invalid regex.")
    })
}

static RECORD_REGEX: OnceLock<Regex> = OnceLock::new();

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

impl From<u32> for Level {
    fn from(value: u32) -> Self {
        let priority = value & 0b111;
        match priority {
            0 => Level::Emerg,
            1 => Level::Alert,
            2 => Level::Crit,
            3 => Level::Err,
            4 => Level::Warn,
            5 => Level::Notice,
            6 => Level::Info,
            7 => Level::Debug,
            _ => Level::Unknown,
        }
    }
}

impl From<u32> for Facility {
    fn from(value: u32) -> Self {
        let facility = (value >> 3) as u8;
        match facility {
            0 => Facility::Kern,
            1 => Facility::User,
            2 => Facility::Mail,
            3 => Facility::Daemon,
            4 => Facility::Auth,
            5 => Facility::Syslog,
            6 => Facility::Lpr,
            7 => Facility::News,
            8 => Facility::Uucp,
            9 => Facility::Cron,
            10 => Facility::Authpriv,
            11 => Facility::Ftp,
            12 => Facility::Res0,
            13 => Facility::Res1,
            14 => Facility::Res2,
            15 => Facility::Res3,
            16 => Facility::Local0,
            17 => Facility::Local1,
            18 => Facility::Local2,
            19 => Facility::Local3,
            20 => Facility::Local4,
            21 => Facility::Local5,
            22 => Facility::Local6,
            23 => Facility::Local7,
            _ => Facility::Unknown,
        }
    }
}
