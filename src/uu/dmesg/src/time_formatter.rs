use chrono::{DateTime, FixedOffset, TimeDelta};
#[cfg(feature = "fixed-boot-time")]
use chrono::{NaiveDate, NaiveTime};
use std::sync::OnceLock;

pub fn raw(timestamp_us: u64) -> String {
    let seconds = timestamp_us / 1000000;
    let sub_seconds = timestamp_us % 1000000;
    format!("{:>5}.{:0>6}", seconds, sub_seconds)
}

pub fn ctime(timestamp_us: u64) -> String {
    let date_time = boot_time()
        .checked_add_signed(TimeDelta::microseconds(timestamp_us as i64))
        .unwrap();
    date_time.format("%a %b %d %H:%M:%S %Y").to_string()
}

pub fn iso(timestamp_us: u64) -> String {
    let date_time = boot_time()
        .checked_add_signed(TimeDelta::microseconds(timestamp_us as i64))
        .unwrap();
    date_time.format("%Y-%m-%dT%H:%M:%S,%6f%:z").to_string()
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
fn boot_time() -> DateTime<FixedOffset> {
    *BOOT_TIME.get_or_init(|| procfs::boot_time().unwrap().into())
}
