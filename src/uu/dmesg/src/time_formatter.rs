use chrono::{DateTime, FixedOffset, TimeDelta, Timelike};
#[cfg(feature = "fixed-boot-time")]
use chrono::{NaiveDate, NaiveTime};
use std::sync::OnceLock;

pub fn raw(timestamp_us: u64) -> String {
    let seconds = timestamp_us / 1000000;
    let sub_seconds = timestamp_us % 1000000;
    format!("{:>5}.{:0>6}", seconds, sub_seconds)
}

pub fn ctime(timestamp_us: u64) -> String {
    let mut date_time = boot_time()
        .checked_add_signed(TimeDelta::microseconds(timestamp_us as i64))
        .unwrap();
    // dmesg always round up sub seconds.
    if date_time.time().nanosecond() > 0 {
        date_time = date_time.checked_add_signed(TimeDelta::seconds(1)).unwrap();
    }
    date_time.format("%a %b %d %H:%M:%S %Y").to_string()
}

static BOOT_TIME: OnceLock<DateTime<FixedOffset>> = OnceLock::new();

#[cfg(feature = "fixed-boot-time")]
fn boot_time() -> DateTime<FixedOffset> {
    *BOOT_TIME.get_or_init(|| {
        let date = NaiveDate::from_ymd_opt(2024, 11, 18).unwrap();
        let time = NaiveTime::from_hms_opt(19, 34, 12).unwrap();
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
