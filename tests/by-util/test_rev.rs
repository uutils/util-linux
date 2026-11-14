// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::{at_and_ucmd, new_ucmd};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_piped_in_data() {
    new_ucmd!().pipe_in("a test").succeeds().stdout_is("tset a");
}

#[test]
fn test_utf8_characters() {
    // Combining marks reverse separately because reversal is by code point.
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in("aé中🙂b\ne\u{301}x")
        .succeeds()
        .stdout_is_bytes("b🙂中éa\nx\u{301}e".as_bytes());
}

#[test]
fn test_utf8_zero_records() {
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .arg("-0")
        .pipe_in("é中\0a🙂")
        .succeeds()
        .stdout_is_bytes("中é\0🙂a".as_bytes());
}

#[test]
fn test_utf8_files() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "aé中\n");
    at.write("b.txt", "x🙂");

    ucmd.env("LC_ALL", "C.UTF-8")
        .args(&["a.txt", "b.txt"])
        .succeeds()
        .stdout_is_bytes("中éa\n🙂x".as_bytes());
}

#[test]
fn test_c_locale_keeps_byte_reversal() {
    // util-linux exits 1 here; rev keeps reversing the bytes.
    new_ucmd!()
        .env("LC_ALL", "C")
        .pipe_in("é\n")
        .succeeds()
        .stdout_is_bytes([0xa9, 0xc3, b'\n']);
}

#[test]
fn test_invalid_utf8_keeps_byte_reversal() {
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in([0xc3, b'A', b'\n'])
        .succeeds()
        .stdout_is_bytes([b'A', 0xc3, b'\n']);
}

#[test]
fn test_locale_precedence() {
    let cases: &[(&str, &str, &str, &[u8])] = &[
        ("", "C.UTF-8", "C", "中é\n".as_bytes()),
        ("", "", "C.UTF-8", "中é\n".as_bytes()),
        (
            "C",
            "C.UTF-8",
            "C.UTF-8",
            &[0xad, 0xb8, 0xe4, 0xa9, 0xc3, b'\n'],
        ),
        ("", "C", "C.UTF-8", &[0xad, 0xb8, 0xe4, 0xa9, 0xc3, b'\n']),
        ("C.utf8", "C", "C", "中é\n".as_bytes()),
        // Label parsing does not require the locale to be installed.
        ("C.UTF-8@variant", "C", "C", "中é\n".as_bytes()),
    ];
    for &(all, ctype, lang, expected) in cases {
        new_ucmd!()
            .env("LC_ALL", all)
            .env("LC_CTYPE", ctype)
            .env("LANG", lang)
            .pipe_in("é中\n")
            .succeeds()
            .stdout_is_bytes(expected);
    }
}

#[cfg(target_vendor = "apple")]
#[test]
fn test_macos_dotless_utf8_locale() {
    new_ucmd!()
        .env("LC_ALL", "")
        .env("LC_CTYPE", "UTF-8")
        .pipe_in("é中\n")
        .succeeds()
        .stdout_is_bytes("中é\n".as_bytes());
}

#[cfg(not(target_vendor = "apple"))]
#[test]
fn test_dotless_utf8_locale_keeps_byte_reversal() {
    new_ucmd!()
        .env("LC_ALL", "")
        .env("LC_CTYPE", "UTF-8")
        .pipe_in("é中\n")
        .succeeds()
        .stdout_is_bytes([0xad, 0xb8, 0xe4, 0xa9, 0xc3, b'\n']);
}

#[test]
fn test_mixed_valid_and_invalid_utf8_records() {
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in([
            0xc3, 0xa9, b'\n', 0xc3, b'A', b'\n', 0xe4, 0xb8, 0xad, b'\n',
        ])
        .succeeds()
        .stdout_is_bytes([
            0xc3, 0xa9, b'\n', b'A', 0xc3, b'\n', 0xe4, 0xb8, 0xad, b'\n',
        ]);
}

#[test]
fn test_utf8_records_with_ascii_ends() {
    let ascii: String = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let mut records: Vec<String> = (0..=ascii.len()).map(|n| ascii[..n].to_string()).collect();
    records.extend((0..=ascii.len()).map(|at| format!("{}é{}", &ascii[..at], &ascii[at..])));
    let input: String = records.iter().map(|r| format!("{r}\n")).collect();
    let expected: String = records
        .iter()
        .map(|r| format!("{}\n", r.chars().rev().collect::<String>()))
        .collect();
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in(input)
        .succeeds()
        .stdout_is_bytes(expected.as_bytes());
}

#[test]
fn test_invalid_utf8_between_ascii_ends_keeps_byte_reversal() {
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in(&b"0123456789abcdef\xc3ghijklmnopqrstuv\n"[..])
        .succeeds()
        .stdout_is_bytes(b"vutsrqponmlkjihg\xc3fedcba9876543210\n");
}

#[test]
fn test_long_ascii_record_in_utf8_locale() {
    let input: Vec<u8> = (0..65_535).map(|i| b'a' + (i % 26) as u8).collect();
    let mut expected = input.clone();
    expected.reverse();
    expected.push(b'\n');
    let mut input = input;
    input.push(b'\n');
    new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in(input)
        .succeeds()
        .stdout_is_bytes(expected);
}

fn utf8_reversed(record: &[u8]) -> Vec<u8> {
    match std::str::from_utf8(record) {
        Ok(text) => text.chars().rev().collect::<String>().into_bytes(),
        Err(_) => record.iter().rev().copied().collect(),
    }
}

// Compare records separately to identify content mismatches.
fn check_utf8_records(records: &[Vec<u8>]) {
    let mut input = Vec::new();
    for record in records {
        input.extend_from_slice(record);
        input.push(b'\n');
    }
    let result = new_ucmd!()
        .env("LC_ALL", "C.UTF-8")
        .pipe_in(input)
        .succeeds();
    let mut lines: Vec<&[u8]> = result.stdout().split(|&b| b == b'\n').collect();
    assert_eq!(lines.pop(), Some(&b""[..]));
    assert_eq!(lines.len(), records.len());
    for (line, record) in lines.iter().zip(records) {
        assert_eq!(*line, &utf8_reversed(record)[..], "record {record:02x?}");
    }
}

#[test]
fn test_long_ascii_record_ending_in_invalid_utf8() {
    let mut record = vec![b'a'; 65_534];
    record.push(0xe9);
    check_utf8_records(&[record]);
}

#[test]
fn test_long_records_with_ascii_ends() {
    let ascii: Vec<u8> = (0..150).map(|i| b'a' + (i % 26) as u8).collect();
    let pieces: [&[u8]; 5] = [
        "é".as_bytes(),
        "中".as_bytes(),
        "🙂".as_bytes(),
        b"\xe9",
        b"\x80",
    ];
    let records: Vec<Vec<u8>> = pieces
        .iter()
        .flat_map(|piece| (1..ascii.len()).map(|at| [&ascii[..at], piece, &ascii[at..]].concat()))
        .collect();
    check_utf8_records(&records);
}

#[test]
fn test_last_non_ascii_character() {
    // The last-character check must not accept a record that starts with a stray
    // continuation byte.
    let ends: [&[u8]; 8] = [
        "é".as_bytes(),
        "中".as_bytes(),
        "🙂".as_bytes(),
        b"\xe9",
        b"\xe4\xb8",
        b"\xf0\x9f\x99",
        b"\x80",
        b"\xc3\xa9\x80\x80\x80",
    ];
    let mut records = Vec::new();
    for len in [0, 1, 31, 32, 33, 130] {
        for end in ends {
            for after in [&b""[..], b"\r", b".  ", b"1234567", b"12345678"] {
                let record = [&vec![b'x'; len][..], end, after].concat();
                records.push([&b"\x80"[..], &record].concat());
                records.push(record);
            }
        }
    }
    check_utf8_records(&records);
}

#[test]
fn test_non_ascii_after_ascii_prefix() {
    // Exercise ASCII-prefix skipping across 64-byte boundaries.
    let mut records = Vec::new();
    for at in 0..=140 {
        let ascii = vec![b'x'; at];
        records.push([&ascii[..], "é-中".as_bytes()].concat());
        records.push([&ascii[..], b"\x80", "é-中".as_bytes()].concat());
    }
    check_utf8_records(&records);
}

#[test]
fn test_existing_file() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "line A\nline B");

    ucmd.arg("a.txt").succeeds().stdout_is("A enil\nB enil");
}

#[test]
fn test_zero() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "line A\0line B");

    ucmd.arg("a.txt")
        .arg("--zero")
        .succeeds()
        .stdout_is("A enil\0B enil");
}

#[test]
fn test_multiple_files() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "file A\n");
    at.write("b.txt", "file B\n");

    ucmd.args(&["a.txt", "b.txt"])
        .succeeds()
        .stdout_is("A elif\nB elif\n");
}

#[test]
fn test_empty_file() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.touch("empty.txt");

    ucmd.arg("empty.txt").succeeds().no_output();
}

#[test]
fn test_non_existing_file() {
    new_ucmd!()
        .arg("non_existing_file")
        .fails()
        .code_is(1)
        .no_stdout()
        .stderr_contains("cannot open non_existing_file: No such file or directory");
}

#[test]
fn test_non_existing_and_existing_file() {
    let (at, mut ucmd) = at_and_ucmd!();

    at.write("a.txt", "file A");

    ucmd.arg("non_existing_file")
        .arg("a.txt")
        .fails()
        .code_is(1)
        .stderr_contains("cannot open non_existing_file: No such file or directory")
        .stdout_is("A elif");
}
