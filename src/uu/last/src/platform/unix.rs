// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::options;
use crate::uu_app;

use uucore::error::UIoError;
use uucore::error::UResult;

use uucore::error::USimpleError;
use uucore::utmpx::time::{OffsetDateTime, UtcOffset};
use uucore::utmpx::{time, Utmpx};

use std::fmt::Write;
use std::fs;
use std::io;
use std::net::Ipv4Addr;

use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use parse_datetime::parse_datetime;

fn get_long_usage() -> String {
    format!("If FILE is not specified, use {WTMP_PATH}.  /var/log/wtmp as FILE is common.")
}

const WTMP_PATH: &str = "/var/log/wtmp";
static TIME_FORMAT_STR: [&str; 4] = ["notime", "short", "full", "iso"];

fn parse_time_value(time_value: &str) -> UResult<OffsetDateTime> {
    parse_datetime(time_value).map_or_else(
        |_| {
            Err(USimpleError::new(
                1,
                format!("invalid time value \"{}\"", time_value),
            ))
        },
        |dt| {
            UtcOffset::from_whole_seconds(dt.offset().local_minus_utc()).map_or_else(
                |_| Err(USimpleError::new(2, "failed to extract time zone offset")),
                |offset| {
                    let naive = dt.naive_local();
                    Ok(
                        OffsetDateTime::from_unix_timestamp(naive.and_utc().timestamp())
                            .expect("Invalid timestamp")
                            .replace_offset(offset),
                    )
                },
            )
        },
    )
}

pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app()
        .after_help(get_long_usage())
        .try_get_matches_from(args)?;

    let system = matches.get_flag(options::SYSTEM);
    let dns = matches.get_flag(options::DNS);
    let hostlast = matches.get_flag(options::HOSTLAST);
    let nohost = matches.get_flag(options::NO_HOST);
    let until = parse_time_value(matches.get_one::<String>(options::UNTIL).unwrap())?;
    let since = parse_time_value(matches.get_one::<String>(options::SINCE).unwrap())?;
    let limit: i32 = if let Some(num) = matches.get_one::<i32>(options::LIMIT) {
        *num
    } else {
        0 // Original implementation has 0 mean no limit (print all values)
    };

    let time_format = if let Some(format) = matches.get_one::<String>(options::TIME_FORMAT) {
        let format_str = format.as_str().trim();
        if TIME_FORMAT_STR.contains(&format_str) {
            Ok(format.to_string())
        } else {
            Err(USimpleError::new(
                0,
                format!("unknown time format: {format}"),
            ))
        }
    } else {
        Ok("short".to_string())
    }?;

    let file: String = if let Some(files) = matches.get_one::<String>(options::FILE) {
        files.to_string()
    } else {
        WTMP_PATH.to_string()
    };

    let user: Option<Vec<String>> =
        if let Some(users) = matches.get_many::<String>(options::USER_TTY) {
            users
                .map(|v| {
                    if is_numeric(v) {
                        Some(format!("tty{v}"))
                    } else {
                        Some(v.to_owned())
                    }
                })
                .collect()
        } else {
            None
        };

    let mut last = Last {
        last_reboot_ut: None,
        last_shutdown_ut: None,
        last_dead_ut: vec![],
        system,
        dns,
        host_last: hostlast,
        no_host: nohost,
        limit,
        file: file.to_string(),
        users: user,
        time_format,
        since,
        until,
    };

    last.exec()
}

const RUN_LEVEL_STR: &str = "runlevel";
const REBOOT_STR: &str = "reboot";
const SHUTDOWN_STR: &str = "shutdown";

struct Last {
    last_reboot_ut: Option<Utmpx>,
    last_shutdown_ut: Option<Utmpx>,
    last_dead_ut: Vec<Utmpx>,
    system: bool,
    dns: bool,
    host_last: bool,
    no_host: bool,
    file: String,
    time_format: String,
    users: Option<Vec<String>>,
    limit: i32,
    since: OffsetDateTime,
    until: OffsetDateTime,
}

fn is_numeric(s: &str) -> bool {
    s.chars().all(|c| c.is_numeric())
}

#[inline]
fn calculate_time_delta(
    curr_datetime: &OffsetDateTime,
    last_datetime: &OffsetDateTime,
) -> time::Duration {
    let curr_duration = time::Duration::new(
        curr_datetime.unix_timestamp(),
        curr_datetime.nanosecond().try_into().unwrap_or_default(), // nanosecond value is always a value between 0 and 1.000.000.000, shouldn't panic
    );

    let last_duration = time::Duration::new(
        last_datetime.unix_timestamp(),
        last_datetime.nanosecond().try_into().unwrap_or_default(), // nanosecond value is always a value between 0 and 1.000.000.000, shouldn't panic
    );

    last_duration - curr_duration
}

#[inline]
fn duration_string(duration: time::Duration) -> String {
    let mut seconds = duration.whole_seconds();

    let days = seconds / 86400;
    seconds -= days * 86400;
    let hours = seconds / 3600;
    seconds -= hours * 3600;
    let minutes = seconds / 60;

    if days > 0 {
        format!("({days}+{hours:0>2}:{minutes:0>2})")
    } else {
        format!("({hours:0>2}:{minutes:0>2})")
    }
}

fn find_dns_name(ut: &Utmpx) -> String {
    let default = Ipv4Addr::new(0, 0, 0, 0);
    let ip = std::net::IpAddr::V4(Ipv4Addr::from_str(&ut.host()).unwrap_or(default));

    if ip.to_string().trim() == "0.0.0.0" {
        ip.to_string()
    } else {
        dns_lookup::lookup_addr(&ip).unwrap_or_default()
    }
}

impl Last {
    const TIME_FULL_FMT: &'static str = "[weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]:[second] [year]";
    const END_TIME_SHORT_FMT: &'static str = "[hour]:[minute]";
    const START_TIME_SHORT_FMT: &'static str =
        "[weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]";
    const TIME_ISO_FMT: &'static str =
        "[year]-[month]-[day]T[hour]:[minute]:[second]+[offset_hour]:[offset_minute]";

    #[allow(clippy::cognitive_complexity)]
    fn exec(&mut self) -> UResult<()> {
        let mut ut_stack: Vec<Utmpx> = vec![];
        // For 'last' output, older output needs to be printed last (FILO), as
        // UtmpxIter does not implement Rev trait. A better implementation
        // might include implementing UtmpxIter as doubly linked
        Utmpx::iter_all_records_from(&self.file).for_each(|ut| ut_stack.push(ut));

        let mut counter = 0;
        let mut first_ut_time = None;
        while let Some(ut) = ut_stack.pop() {
            if ut.login_time() < self.since || ut.login_time() > self.until {
                continue;
            }

            if ut_stack.is_empty() {
                // By the end of loop we will have the earliest time
                // (This avoids getting into issues with the compiler)
                let first_login_time = ut.login_time();
                first_ut_time = Some(self.utmp_file_time(
                    first_login_time.unix_timestamp(),
                    first_login_time.nanosecond().into(),
                ));
            }

            if counter >= self.limit && self.limit > 0 {
                break;
            }
            if ut.is_user_process() {
                let mut dead_proc: Option<Utmpx> = None;
                if let Some(pos) = self
                    .last_dead_ut
                    .iter()
                    .position(|dead_ut| ut.tty_device() == dead_ut.tty_device())
                {
                    dead_proc = Some(self.last_dead_ut.swap_remove(pos));
                }
                if self.print_user(&ut, dead_proc.as_ref()) {
                    counter += 1;
                }
            } else if ut.user() == RUN_LEVEL_STR {
                if self.print_runlevel(&ut) {
                    counter += 1;
                }
            } else if ut.user() == SHUTDOWN_STR {
                if self.print_shutdown(&ut) {
                    counter += 1;
                }
                self.last_shutdown_ut = Some(ut);
            } else if ut.user() == REBOOT_STR {
                if self.print_reboot(&ut) {
                    counter += 1;
                }
                self.last_reboot_ut = Some(ut);
            } else if ut.user() == "" {
                // Dead process end date
                self.last_dead_ut.push(ut);
            }
        }

        let path = std::path::absolute(&self.file)?;
        let path_str = path
            .file_name()
            .ok_or_else(|| {
                if path.is_dir() {
                    UIoError::new(io::ErrorKind::InvalidData, "Is a directory")
                } else {
                    UIoError::new(io::ErrorKind::Unsupported, "Undefined")
                }
            })?
            .to_str()
            .ok_or(UIoError::new(
                io::ErrorKind::InvalidData,
                "invalid character data (not UTF-8)",
            ))?;

        if let Some(file_time) = first_ut_time {
            println!("\n{path_str} begins {file_time}");
        } else {
            let secs = fs::metadata(&self.file)?.ctime();
            let nsecs = fs::metadata(&self.file)?.ctime_nsec() as u64;
            let file_time = self.utmp_file_time(secs, nsecs);

            println!("\n{path_str} begins {file_time}");
        }

        Ok(())
    }

    #[inline]
    fn utmp_file_time(&self, secs: i64, nsecs: u64) -> String {
        let description = match self.time_format.as_str() {
            "short" | "full" => Self::TIME_FULL_FMT,
            "iso" => Self::TIME_ISO_FMT,
            _ => return "".to_string(),
        };

        let time_format: Vec<time::format_description::FormatItem> =
            time::format_description::parse(description).unwrap_or_default();

        let time = time::OffsetDateTime::from_unix_timestamp(secs)
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
            + Duration::from_nanos(nsecs);

        let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        let offset_secs: u64 = offset.whole_seconds() as u64;

        // Adding back the time to the offset so that offset_time is correct.
        let offset_time = time.replace_offset(offset) + Duration::from_secs(offset_secs);

        offset_time.format(&time_format).unwrap_or_default()
    }

    #[inline]
    fn time_string(&self, ut: &Utmpx) -> String {
        let description = match self.time_format.as_str() {
            "short" => Self::START_TIME_SHORT_FMT,
            "full" => Self::TIME_FULL_FMT,
            "iso" => Self::TIME_ISO_FMT,
            _ => return "".to_string(),
        };

        // "%b %e %H:%M"
        let time_format: Vec<time::format_description::FormatItem> =
            time::format_description::parse(description).unwrap_or_default();
        ut.login_time().format(&time_format).unwrap_or_default()
    }

    #[inline]
    fn end_time_string(&self, user_process_str: Option<&str>, end_ut: &OffsetDateTime) -> String {
        match user_process_str {
            Some(val) => val.to_string(),
            _ => {
                let description = match self.time_format.as_str() {
                    "short" => format!("- {}", Self::END_TIME_SHORT_FMT),
                    "full" => format!("- {}", Self::TIME_FULL_FMT),
                    "iso" => format!("- {}", Self::TIME_ISO_FMT),
                    _ => return "".to_string(),
                };

                // "%H:%M"
                let time_format: Vec<time::format_description::FormatItem> =
                    time::format_description::parse(&description).unwrap_or_default();
                end_ut.format(&time_format).unwrap_or_default()
            }
        }
    }

    #[inline]
    fn end_state_string(&self, ut: &Utmpx, dead_ut: Option<&Utmpx>) -> (String, String) {
        // This function takes a considerable amount of CPU cycles to complete;
        // root cause seems to be the ut.login_time function, which reads a
        // file to determine local offset for UTC. Perhaps this function
        // should be updated to save that UTC offset for subsequent calls
        let mut proc_status: Option<&str> = None;
        let curr_datetime = ut.login_time();

        if let Some(dead) = dead_ut {
            let dead_datetime = dead.login_time();
            let time_delta = duration_string(calculate_time_delta(&curr_datetime, &dead_datetime));
            return (
                self.end_time_string(proc_status, &dead_datetime),
                time_delta.to_string(),
            );
        }

        let reboot_datetime: Option<OffsetDateTime>;
        let shutdown_datetime: Option<OffsetDateTime>;
        if let Some(reboot) = &self.last_reboot_ut {
            reboot_datetime = Some(reboot.login_time());
        } else {
            reboot_datetime = None;
        }

        if let Some(shutdown) = &self.last_shutdown_ut {
            shutdown_datetime = Some(shutdown.login_time());
        } else {
            shutdown_datetime = None;
        }

        if shutdown_datetime.is_none() {
            if ut.is_user_process() {
                // If a reboot has occurred since the user logged in, but not shutdown is recorded
                // then a crash must have occurred.
                if reboot_datetime.is_some() && reboot_datetime.unwrap() > ut.login_time() {
                    ("- crash".to_string(), "".to_string())
                } else {
                    ("  still logged in".to_string(), "".to_string())
                }
            } else {
                ("  still running".to_string(), "".to_string())
            }
        } else {
            let shutdown = shutdown_datetime
                .unwrap_or_else(|| time::OffsetDateTime::from_unix_timestamp(0).unwrap());
            let time_delta = duration_string(calculate_time_delta(&curr_datetime, &shutdown));
            if ut.is_user_process() {
                proc_status = Some("- down ");
            }
            (
                self.end_time_string(proc_status, &shutdown),
                time_delta.to_string(),
            )
        }
    }

    #[inline]
    fn print_runlevel(&self, ut: &Utmpx) -> bool {
        if let Some(users) = &self.users {
            if !users
                .iter()
                .any(|val| val.as_str().trim() == ut.user().trim())
            {
                return false;
            }
        }
        if self.system {
            let curr = (ut.pid() % 256) as u8 as char;
            let runlvline = format!("(to lvl {curr})");
            let (end_date, delta) = self.end_state_string(ut, None);
            let host = if self.dns {
                find_dns_name(ut)
            } else {
                ut.host()
            };
            self.print_line(
                RUN_LEVEL_STR,
                &runlvline,
                &self.time_string(ut),
                &host,
                &end_date,
                &delta,
            );
            true
        } else {
            false
        }
    }

    #[inline]
    fn print_shutdown(&self, ut: &Utmpx) -> bool {
        if let Some(users) = &self.users {
            if !users.iter().any(|val| {
                val.as_str().trim() == "system down" || val.as_str().trim() == ut.user().trim()
            }) {
                return false;
            }
        }
        let host = if self.dns {
            find_dns_name(ut)
        } else {
            ut.host()
        };
        if self.system {
            let (end_date, delta) = self.end_state_string(ut, None);
            self.print_line(
                SHUTDOWN_STR,
                "system down",
                &self.time_string(ut),
                &host,
                &end_date,
                &delta,
            );
            true
        } else {
            false
        }
    }

    #[inline]
    fn print_reboot(&self, ut: &Utmpx) -> bool {
        if let Some(users) = &self.users {
            if !users.iter().any(|val| {
                val.as_str().trim() == ut.user().trim() || val.as_str().trim() == "system boot"
            }) {
                return false;
            }
        }
        let (end_date, delta) = self.end_state_string(ut, None);
        let host = if self.dns {
            find_dns_name(ut)
        } else {
            ut.host()
        };
        self.print_line(
            REBOOT_STR,
            "system boot",
            &self.time_string(ut),
            &host,
            &end_date,
            &delta,
        );

        true
    }

    #[inline]
    fn print_user(&self, ut: &Utmpx, dead_ut: Option<&Utmpx>) -> bool {
        if let Some(users) = &self.users {
            if !users.iter().any(|val| {
                val.as_str().trim() == ut.tty_device().as_str().trim()
                    || val.as_str().trim() == ut.user().trim()
            }) {
                return false;
            }
        }
        let mut p = PathBuf::from("/dev");
        p.push(ut.tty_device().as_str());
        let host = if self.dns {
            find_dns_name(ut)
        } else {
            ut.host()
        };

        let (end_date, delta) = self.end_state_string(ut, dead_ut);

        self.print_line(
            ut.user().as_ref(),
            ut.tty_device().as_ref(),
            self.time_string(ut).as_str(),
            &host,
            &end_date,
            &delta,
        );

        true
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn print_line(
        &self,
        user: &str,
        line: &str,
        time: &str,
        host: &str,
        end_time: &str,
        delta: &str,
    ) {
        let mut buf = String::with_capacity(64);
        let host_to_print = host.get(0..16).unwrap_or(host);

        write!(buf, "{user:<8}").unwrap_or_default();
        write!(buf, " {line:<12}").unwrap_or_default();
        if !self.host_last && !self.no_host {
            write!(buf, " {host_to_print:<16}").unwrap_or_default();
        }

        if self.time_format != "notime" {
            let time_fmt = 12;
            let end_time_delta = format!("{end_time:<6} {delta}");
            let end_time_delta_fmt = 18;

            write!(buf, " {time:<time_fmt$}").unwrap_or_default();
            write!(buf, " {end_time_delta:<end_time_delta_fmt$}").unwrap_or_default();
        }

        if self.host_last && !self.no_host {
            write!(buf, " {host_to_print:<16}").unwrap_or_default();
        }
        println!("{}", buf.trim_end());
    }
}
