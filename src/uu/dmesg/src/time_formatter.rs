// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use chrono::{DateTime, FixedOffset, TimeDelta};
#[cfg(feature = "fixed-boot-time")]
use chrono::{NaiveDate, NaiveTime};
use std::sync::OnceLock;

pub fn raw(timestamp_us: i64) -> String {
    let seconds = timestamp_us / 1000000;
    let sub_seconds = timestamp_us % 1000000;
    format!("{:>5}.{:0>6}", seconds, sub_seconds)
}

pub fn ctime(timestamp_us: i64) -> String {
    let date_time = boot_time()
        .checked_add_signed(TimeDelta::microseconds(timestamp_us))
        .unwrap();
    date_time.format("%a %b %d %H:%M:%S %Y").to_string()
}

pub fn iso(timestamp_us: i64) -> String {
    let date_time = boot_time()
        .checked_add_signed(TimeDelta::microseconds(timestamp_us))
        .unwrap();
    date_time.format("%Y-%m-%dT%H:%M:%S,%6f%:z").to_string()
}

pub struct ReltimeFormatter {
    state: State,
    prev_timestamp_us: i64,
    previous_unix_timestamp: i64,
}

pub struct DeltaFormatter {
    state: State,
    prev_timestamp_us: i64,
}

pub enum State {
    Initial,
    AfterBoot,
    Delta,
}

impl ReltimeFormatter {
    pub fn new() -> Self {
        ReltimeFormatter {
            state: State::Initial,
            prev_timestamp_us: 0,
            previous_unix_timestamp: 0,
        }
    }

    pub fn format(&mut self, timestamp_us: i64) -> String {
        let date_time = boot_time()
            .checked_add_signed(TimeDelta::microseconds(timestamp_us))
            .unwrap();
        let unix_timestamp = date_time.timestamp();
        let minute_changes = (unix_timestamp / 60) != (self.previous_unix_timestamp / 60);
        let format_res = match self.state {
            State::Initial => date_time.format("%b%d %H:%M").to_string(),
            _ if minute_changes => date_time.format("%b%d %H:%M").to_string(),
            State::AfterBoot => Self::delta(0),
            State::Delta => Self::delta(timestamp_us - self.prev_timestamp_us),
        };
        self.prev_timestamp_us = timestamp_us;
        self.previous_unix_timestamp = unix_timestamp;
        self.state = match self.state {
            State::Initial if timestamp_us == 0 => State::AfterBoot,
            _ => State::Delta,
        };
        format_res
    }

    fn delta(delta_us: i64) -> String {
        let seconds = i64::abs(delta_us / 1000000);
        let sub_seconds = i64::abs(delta_us % 1000000);
        let sign = if delta_us >= 0 { '+' } else { '-' };
        let res = format!("{}{}.{:0>6}", sign, seconds, sub_seconds);
        format!("{:>11}", res)
    }
}

impl DeltaFormatter {
    pub fn new() -> Self {
        DeltaFormatter {
            state: State::Initial,
            prev_timestamp_us: 0,
        }
    }

    pub fn format(&mut self, timestamp_us: i64) -> String {
        let format_res = match self.state {
            State::Delta => Self::delta(timestamp_us - self.prev_timestamp_us),
            _ => Self::delta(0),
        };
        self.prev_timestamp_us = timestamp_us;
        self.state = match self.state {
            State::Initial if timestamp_us == 0 => State::AfterBoot,
            _ => State::Delta,
        };
        format_res
    }

    fn delta(delta_us: i64) -> String {
        let seconds = i64::abs(delta_us / 1000000);
        let sub_seconds = i64::abs(delta_us % 1000000);
        let mut res = format!("{}.{:0>6}", seconds, sub_seconds);
        if delta_us < 0 {
            res.insert(0, '-');
        }
        format!("<{:>12}>", res)
    }
}

static BOOT_TIME: OnceLock<DateTime<FixedOffset>> = OnceLock::new();

#[cfg(feature = "fixed-boot-time")]
fn boot_time() -> DateTime<FixedOffset> {
    *BOOT_TIME.get_or_init(|| {
        let date = NaiveDate::from_ymd_opt(2024, 11, 18).unwrap();
        let time = NaiveTime::from_hms_micro_opt(19, 34, 12, 866807).unwrap();
        let tz = FixedOffset::east_opt(7 * 3600).unwrap();
        chrono::NaiveDateTime::new(date, time)
            .and_local_timezone(tz)
            .unwrap()
    })
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(unix)]
#[cfg(not(target_os = "openbsd"))]
fn boot_time() -> DateTime<FixedOffset> {
    *BOOT_TIME.get_or_init(|| boot_time_from_utmpx().unwrap())
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(windows)]
fn boot_time() -> DateTime<FixedOffset> {
    // TODO: get windows boot time
    *BOOT_TIME.get_or_init(|| chrono::DateTime::from_timestamp(0, 0).unwrap().into())
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(target_os = "openbsd")]
fn boot_time() -> DateTime<FixedOffset> {
    // TODO: get openbsd boot time
    *BOOT_TIME.get_or_init(|| chrono::DateTime::from_timestamp(0, 0).unwrap().into())
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(unix)]
#[cfg(not(target_os = "openbsd"))]
fn boot_time_from_utmpx() -> Option<DateTime<FixedOffset>> {
    for record in uucore::utmpx::Utmpx::iter_all_records() {
        if record.record_type() == uucore::utmpx::BOOT_TIME {
            let t = record.login_time();
            return Some(
                chrono::DateTime::from_timestamp(t.unix_timestamp(), t.nanosecond())
                    .unwrap()
                    .with_timezone(&chrono::Local)
                    .into(),
            );
        }
    }
    None
}
