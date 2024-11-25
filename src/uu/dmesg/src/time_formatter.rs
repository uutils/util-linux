pub fn raw(timestamp_us: u64) -> String {
    let seconds = timestamp_us / 1000000;
    let sub_seconds = timestamp_us % 1000000;
    format!("{:>5}.{:0>6}", seconds, sub_seconds)
}
