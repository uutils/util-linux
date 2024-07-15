use crate::options;
use crate::uu_app;

use uucore::error::UResult;

use uucore::error::USimpleError;
use uucore::utmpx::time::OffsetDateTime;
use uucore::utmpx::{time, Utmpx};

use std::fmt::Write;
use std::net::Ipv4Addr;

use std::panic;
use std::path::PathBuf;
use std::str::FromStr;

fn get_long_usage() -> String {
    format!(
        "If FILE is not specified, use {}.  /var/log/wtmp as FILE is common.\n\
         If ARG1 ARG2 given, -m presumed: 'am i' or 'mom likes' are usual.",
        WTMP_PATH,
    )
}

const WTMP_PATH: &str = "/var/log/wtmp";
static TIME_FORMAT_STR: [&str; 4] = ["notime", "short", "full", "iso"];

pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app()
        .after_help(get_long_usage())
        .try_get_matches_from(args)?;

    let system = matches.get_flag(options::SYSTEM);
    let dns = matches.get_flag(options::DNS);
    let hostlast = matches.get_flag(options::HOSTLAST);
    let nohost = matches.get_flag(options::NO_HOST);
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
                    if is_numeric(&v) {
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
}

fn is_numeric(s: &str) -> bool {
    s.chars().all(|c| c.is_numeric())
}

fn is_quoted(s: &str) -> bool {
    s.chars().all(|c| c == '"' || c == '\'')
}

#[inline]
fn calculate_time_delta(
    curr_datetime: &OffsetDateTime,
    last_datetime: &OffsetDateTime,
) -> time::Duration {
    let curr_duration = time::Duration::new(
        curr_datetime.unix_timestamp(),
        curr_datetime.nanosecond().try_into().unwrap(), // nanosecond value is always a value between 0 and 1.000.000.000, shouldn't panic
    );

    let last_duration = time::Duration::new(
        last_datetime.unix_timestamp(),
        last_datetime.nanosecond().try_into().unwrap(), // nanosecond value is always a value between 0 and 1.000.000.000, shouldn't panic
    );

    last_duration - curr_duration
}

#[inline]
fn duration_string(duration: time::Duration) -> String {
    let mut seconds = duration.whole_seconds();

    let days = seconds / 86400;
    seconds = seconds - (days * 86400);
    let hours = seconds / 3600;
    seconds = seconds - (hours * 3600);
    let minutes = seconds / 60;

    if days > 0 {
        format!("({}+{:0>2}:{:0>2})", days, hours, minutes)
    } else {
        format!("({:0>2}:{:0>2})", hours, minutes)
    }
}

fn find_dns_name(ut: &Utmpx) -> String {
    let default = Ipv4Addr::new(0, 0, 0, 0);
    let ip = std::net::IpAddr::V4(Ipv4Addr::from_str(&ut.host()).unwrap_or(default));

    if ip.to_string().trim() == "0.0.0.0" {
        return ip.to_string();
    } else {
        return dns_lookup::lookup_addr(&ip).unwrap();
    }
}

impl Last {
    #[allow(clippy::cognitive_complexity)]
    fn exec(&mut self) -> UResult<()> {
        let mut ut_stack: Vec<Utmpx> = vec![];
        // For 'last' output, older output needs to be printed last (FILO), as
        // UtmpxIter does not implement Rev trait. A better implementation
        // might include implementing UtmpxIter as doubly linked
        Utmpx::iter_all_records_from(&self.file).for_each(|ut| ut_stack.push(ut));

        // let mut last: Option<Utmpx> = None;
        let mut counter = 0;
        while let Some(ut) = ut_stack.pop() {
            if counter >= self.limit && self.limit > 0 {
                break;
            }
            // println!("|{}| |{}| |{}|", ut.user(), time_string(&ut), ut.tty_device());
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

        Ok(())
    }

    #[inline]
    fn time_string(&self, ut: &Utmpx) -> String {
        let description = match self.time_format.as_str() {
            "short" => {"[weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]"}
            "full" => {"[weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]:[second] [year]"}
            "iso" => "[year]-[month]-[day]T[hour]:[minute]:[second]+[offset_hour]:[offset_minute]",
            _ => return "".to_string(),
        };

        // "%b %e %H:%M"
        let time_format: Vec<time::format_description::FormatItem> =
            time::format_description::parse(description).unwrap();
        ut.login_time().format(&time_format).unwrap() // LC_ALL=C
    }

    #[inline]
    fn end_time_string(&self, user_process_str: Option<&str>, end_ut: &OffsetDateTime) -> String {
        match user_process_str {
            Some(val) => val.to_string(),
            _ => {
                let description = match self.time_format.as_str() {
                    "short" => {"- [weekday repr:short] [hour]:[minute]"}
                    "full" => {"- [weekday repr:short] [month repr:short] [day padding:space] [hour]:[minute]:[second] [year]"}
                    "iso" => {"- [year]-[month]-[day]T[hour]:[minute]:[second]+[offset_hour]:[offset_minute]"}
                    _ => {return "".to_string()}
                };

                // "%H:%M"
                let time_format: Vec<time::format_description::FormatItem> =
                    time::format_description::parse(description).unwrap();
                end_ut.format(&time_format).unwrap() // LC_ALL=C
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
                if !reboot_datetime.is_none() && reboot_datetime.unwrap() > ut.login_time() {
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
                proc_status = Some("- down");
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
                find_dns_name(&ut)
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
            find_dns_name(&ut)
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
            find_dns_name(&ut)
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
            find_dns_name(&ut)
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

        write!(buf, "{user:<8}").unwrap();
        write!(buf, " {line:<12}").unwrap();
        if !self.host_last && !self.no_host {
            write!(buf, " {host_to_print:<16}").unwrap();
        }

        let time_size = 3 + 2 + 2 + 1 + 2;
        if self.host_last && !self.no_host && self.time_format != "notime" {
            write!(buf, " {time:<time_size$}").unwrap();
            write!(buf, " {end_time:<8}").unwrap();
            write!(buf, " {host_to_print}").unwrap();
        } else if self.time_format != "notime" {
            write!(buf, " {time:<time_size$}").unwrap();
            write!(buf, " {end_time:<8}").unwrap();
        }
        write!(buf, " {delta:^6}").unwrap();
        println!("{}", buf.trim_end());
    }
}