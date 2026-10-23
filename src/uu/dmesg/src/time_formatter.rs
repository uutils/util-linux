// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use chrono::{DateTime, FixedOffset, TimeDelta};
#[cfg(feature = "fixed-boot-time")]
use chrono::{NaiveDate, NaiveTime};
use std::sync::OnceLock;
use uucore::error::{UResult, USimpleError};

pub fn raw(timestamp_us: i64) -> String {
    let seconds = timestamp_us / 1000000;
    let sub_seconds = timestamp_us % 1000000;
    format!("{seconds:>5}.{sub_seconds:0>6}")
}

pub fn ctime(timestamp_us: i64) -> UResult<String> {
    let date_time = boot_time()?
        .checked_add_signed(TimeDelta::microseconds(timestamp_us))
        .unwrap();
    Ok(date_time.format("%a %b %d %H:%M:%S %Y").to_string())
}

pub fn iso(timestamp_us: i64) -> UResult<String> {
    let date_time = boot_time()?
        .checked_add_signed(TimeDelta::microseconds(timestamp_us))
        .unwrap();
    Ok(date_time.format("%Y-%m-%dT%H:%M:%S,%6f%:z").to_string())
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

    pub fn format(&mut self, timestamp_us: i64) -> UResult<String> {
        let date_time = boot_time()?
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
        Ok(format_res)
    }

    fn delta(delta_us: i64) -> String {
        let seconds = i64::abs(delta_us / 1000000);
        let sub_seconds = i64::abs(delta_us % 1000000);
        let sign = if delta_us >= 0 { '+' } else { '-' };
        let res = format!("{sign}{seconds}.{sub_seconds:0>6}");
        format!("{res:>11}")
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
        let mut res = format!("{seconds}.{sub_seconds:0>6}");
        if delta_us < 0 {
            res.insert(0, '-');
        }
        format!("<{res:>12}>")
    }
}

pub fn parse_datetime(s: &str) -> UResult<DateTime<FixedOffset>> {
    parse_datetime::parse_datetime(s)
        .map_err(|_| USimpleError::new(1, format!("invalid time value \"{s}\"")))
}

pub fn datetime_from_microseconds_since_boot(microseconds: i64) -> UResult<DateTime<FixedOffset>> {
    Ok(boot_time()?
        .checked_add_signed(TimeDelta::microseconds(microseconds))
        .unwrap())
}

static BOOT_TIME: OnceLock<Option<DateTime<FixedOffset>>> = OnceLock::new();

/// The time the system booted, looked up once.
pub fn boot_time() -> UResult<DateTime<FixedOffset>> {
    BOOT_TIME
        .get_or_init(lookup_boot_time)
        .ok_or_else(|| USimpleError::new(1, "cannot get the system boot time"))
}

#[cfg(feature = "fixed-boot-time")]
fn lookup_boot_time() -> Option<DateTime<FixedOffset>> {
    let date = NaiveDate::from_ymd_opt(2024, 11, 18).unwrap();
    let time = NaiveTime::from_hms_micro_opt(19, 34, 12, 866807).unwrap();
    let tz = FixedOffset::east_opt(7 * 3600).unwrap();
    chrono::NaiveDateTime::new(date, time)
        .and_local_timezone(tz)
        .single()
}

// The current time minus the time since boot (CLOCK_BOOTTIME, which counts
// suspend), like util-linux. Works without a utmp boot record.
#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn lookup_boot_time() -> Option<DateTime<FixedOffset>> {
    use uucore::libc;

    let mut uptime = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `uptime` is a valid timespec for clock_gettime to write.
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut uptime) } != 0 {
        return None;
    }
    // time_t is 32 bits on some targets, such as 32-bit Android
    #[cfg(target_pointer_width = "64")]
    let secs: i64 = uptime.tv_sec;
    #[cfg(not(target_pointer_width = "64"))]
    let secs: i64 = uptime.tv_sec.into();
    let uptime = TimeDelta::new(secs, uptime.tv_nsec as u32)?;
    Some((chrono::Local::now() - uptime).into())
}

// kern.boottime, which `uptime` and `who -b` read there too.
#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn lookup_boot_time() -> Option<DateTime<FixedOffset>> {
    use uucore::libc;

    let mut mib = [libc::CTL_KERN, libc::KERN_BOOTTIME];
    let mut boot = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut len = std::mem::size_of::<libc::timeval>();
    // SAFETY: kern.boottime is a timeval, and `boot` and `len` describe a
    // buffer of that size for sysctl to write.
    let res = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            &mut boot as *mut libc::timeval as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if res != 0 {
        return None;
    }
    #[cfg(target_pointer_width = "64")]
    let secs: i64 = boot.tv_sec;
    #[cfg(not(target_pointer_width = "64"))]
    let secs: i64 = boot.tv_sec.into();
    let nanos = u32::try_from(boot.tv_usec).ok()? * 1000;
    Some(
        chrono::DateTime::from_timestamp(secs, nanos)?
            .with_timezone(&chrono::Local)
            .into(),
    )
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(unix)]
#[cfg(not(any(
    target_os = "openbsd",
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "freebsd"
)))]
fn lookup_boot_time() -> Option<DateTime<FixedOffset>> {
    for record in uucore::utmpx::Utmpx::iter_all_records() {
        if record.record_type() == uucore::utmpx::BOOT_TIME {
            let t = record.login_time();
            return Some(
                chrono::DateTime::from_timestamp(t.unix_timestamp(), t.nanosecond())?
                    .with_timezone(&chrono::Local)
                    .into(),
            );
        }
    }
    None
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(windows)]
fn lookup_boot_time() -> Option<DateTime<FixedOffset>> {
    // TODO: get windows boot time
    Some(chrono::DateTime::from_timestamp(0, 0).unwrap().into())
}

#[cfg(not(feature = "fixed-boot-time"))]
#[cfg(target_os = "openbsd")]
fn lookup_boot_time() -> Option<DateTime<FixedOffset>> {
    // TODO: get openbsd boot time
    Some(chrono::DateTime::from_timestamp(0, 0).unwrap().into())
}

#[cfg(test)]
#[cfg(not(feature = "fixed-boot-time"))]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn boot_time_matches_proc_uptime() {
        let uptime: f64 = std::fs::read_to_string("/proc/uptime")
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let since_boot = chrono::Local::now().fixed_offset() - boot_time().unwrap();
        assert!((since_boot.as_seconds_f64() - uptime).abs() < 2.0);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "freebsd"))]
    fn boot_time_comes_from_kern_boottime() {
        let since_boot = chrono::Local::now().fixed_offset() - boot_time().unwrap();
        // a real boot time, not the epoch
        assert!(since_boot > TimeDelta::zero() && since_boot < TimeDelta::days(3650));
    }
}
