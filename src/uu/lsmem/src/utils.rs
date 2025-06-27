pub fn size_to_human_string(bytes: u64) -> String {
    let mut buf = String::with_capacity(32);
    let mut dec;
    let mut frac;
    let letters = "BKMGTPE";
    let mut suffix = String::with_capacity(4);

    let exp = get_exp(bytes);
    let c = letters
        .chars()
        .nth(if exp != 0 { exp / 10 } else { 0 })
        .unwrap_or('B');
    dec = if exp != 0 {
        bytes / (1_u64 << exp)
    } else {
        bytes
    };
    frac = if exp != 0 { bytes % (1_u64 << exp) } else { 0 };

    suffix.push(c);

    // Rounding logic
    if frac != 0 {
        if frac >= u64::MAX / 1000 {
            frac = ((frac / 1024) * 1000) / (1 << (exp - 10));
        } else {
            frac = (frac * 1000) / (1 << exp);
        }

        // Round to 1 decimal place
        frac = ((frac + 50) / 100) * 10;

        // Check for overflow due to rounding
        if frac == 100 {
            dec += 1;
            frac = 0;
        }
    }

    // Format the result
    if frac != 0 {
        let decimal_point = ".";
        buf = format!("{dec}{decimal_point}{frac:02}");
        if buf.ends_with('0') {
            buf.pop(); // Remove extraneous zero
        }
        buf += &suffix;
    } else {
        buf += &format!("{dec}");
        buf += &suffix;
    }

    buf
}

fn get_exp(n: u64) -> usize {
    let mut shft = 10;
    while shft <= 60 {
        if n < (1 << shft) {
            break;
        }
        shft += 10;
    }
    shft - 10
}

#[test]
fn test_size_to_human_string() {
    assert_eq!("11.7K", size_to_human_string(12000));
    assert_eq!("11.4M", size_to_human_string(12000000));
    assert_eq!("11.2G", size_to_human_string(12000000000));
}
