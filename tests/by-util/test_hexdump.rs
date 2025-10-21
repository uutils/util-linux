// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::{new_ucmd, util::TestScenario};

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
#[cfg(unix)]
fn test_hexdump_empty_input() {
    new_ucmd!().pipe_in("").succeeds().stdout_is("");
}

#[test]
#[cfg(unix)]
fn test_hexdump_default_format() {
    new_ucmd!()
        .pipe_in("ABCD")
        .succeeds()
        .stdout_is("0000000 4241 4443                              \n0000004\n");

    new_ucmd!()
        .pipe_in("ABC")
        .succeeds()
        .stdout_is("0000000 4241 0043                              \n0000003\n");
}

#[test]
#[cfg(unix)]
fn test_hexdump_canonical_format() {
    new_ucmd!()
        .arg("-C")
        .pipe_in("Hello World!")
        .succeeds()
        .stdout_is(
        "00000000  48 65 6c 6c 6f 20 57 6f  72 6c 64 21              |Hello World!|\n0000000c\n",
    );
}

#[test]
#[cfg(unix)]
fn test_hexdump_one_byte_char() {
    let input = b"Hello \t\n\0\x07\x08\x0B\x0C\r\x80\xFF";
    let expected = vec![
        "0000000   H   e   l   l   o      \\t  \\n  \\0  \\a  \\b  \\v  \\f  \\r 200 377\n",
        "0000010\n",
    ];

    new_ucmd!()
        .arg("-c")
        .pipe_in(input)
        .succeeds()
        .stdout_is(expected.join(""));
}

#[test]
#[cfg(unix)]
fn test_hexdump_one_byte_octal() {
    new_ucmd!().arg("-b").pipe_in("ABC").succeeds().stdout_is(
        "0000000 101 102 103                                                    \n0000003\n",
    );
}

#[test]
#[cfg(unix)]
fn test_hexdump_one_byte_hex() {
    new_ucmd!().arg("-X").pipe_in("ABC").succeeds().stdout_is(
        "0000000  41  42  43                                                    \n0000003\n",
    );
}

#[test]
#[cfg(unix)]
fn test_hexdump_two_bytes_hex() {
    new_ucmd!().arg("-x").pipe_in("ABCD").succeeds().stdout_is(
        "0000000    4241    4443                                                \n0000004\n",
    );

    new_ucmd!().arg("-x").pipe_in("ABC").succeeds().stdout_is(
        "0000000    4241    0043                                                \n0000003\n",
    );
}

#[test]
#[cfg(unix)]
fn test_hexdump_two_bytes_decimal() {
    new_ucmd!().arg("-d").pipe_in("ABCD").succeeds().stdout_is(
        "0000000   16961   17475                                                \n0000004\n",
    );

    new_ucmd!().arg("-d").pipe_in("ABC").succeeds().stdout_is(
        "0000000   16961   00067                                                \n0000003\n",
    );
}

#[test]
#[cfg(unix)]
fn test_hexdump_two_bytes_octal() {
    new_ucmd!().arg("-o").pipe_in("ABCD").succeeds().stdout_is(
        "0000000  041101  042103                                                \n0000004\n",
    );

    new_ucmd!().arg("-o").pipe_in("ABC").succeeds().stdout_is(
        "0000000  041101  000103                                                \n0000003\n",
    );
}

#[test]
#[cfg(unix)]
fn test_hexdump_multiple_formats() {
    let expected = vec![
        "00000000  41 42                                             |AB|\n",
        "0000000    4241                                                        \n",
        "00000000  41 42                                             |AB|\n",
        "00000002\n",
    ];

    new_ucmd!()
        .args(&["-C", "-x", "-C"])
        .pipe_in("AB")
        .succeeds()
        .stdout_is(expected.join(""));
}

#[test]
#[cfg(unix)]
fn test_hexdump_squeezing() {
    let input = vec![
        "AAAAAAAAAAAAAAAA",
        "AAAAAAAAAAAAAAAA",
        "AAAAAAAAAAAAAAAA",
        "AAAAAAAA",
    ];

    let expected_no_squeezing = vec![
        "00000000  41 41 41 41 41 41 41 41  41 41 41 41 41 41 41 41  |AAAAAAAAAAAAAAAA|\n",
        "00000010  41 41 41 41 41 41 41 41  41 41 41 41 41 41 41 41  |AAAAAAAAAAAAAAAA|\n",
        "00000020  41 41 41 41 41 41 41 41  41 41 41 41 41 41 41 41  |AAAAAAAAAAAAAAAA|\n",
        "00000030  41 41 41 41 41 41 41 41                           |AAAAAAAA|\n",
        "00000038\n",
    ];
    let expected_with_squeezing = vec![
        "00000000  41 41 41 41 41 41 41 41  41 41 41 41 41 41 41 41  |AAAAAAAAAAAAAAAA|\n",
        "*\n",
        "00000030  41 41 41 41 41 41 41 41                           |AAAAAAAA|\n",
        "00000038\n",
    ];

    new_ucmd!()
        .args(&["-C", "-v"])
        .pipe_in(input.join(""))
        .succeeds()
        .stdout_is(expected_no_squeezing.join(""));

    new_ucmd!()
        .arg("-C")
        .pipe_in(input.join(""))
        .succeeds()
        .stdout_is(expected_with_squeezing.join(""));
}

#[test]
fn test_hexdump_multiple_files() {
    let scene = TestScenario::new("hexdump");
    scene.fixtures.write("file1.txt", "hello");
    scene.fixtures.write("file2.txt", "world");

    scene
        .ucmd()
        .args(&["-C", "file1.txt", "file2.txt"])
        .succeeds()
        .stdout_is(
            "00000000  68 65 6c 6c 6f 77 6f 72  6c 64                    |helloworld|\n0000000a\n",
        );
}

#[test]
fn test_hexdump_multiple_files_with_skip() {
    let scene = TestScenario::new("hexdump");
    scene.fixtures.write("file1.txt", "abc");
    scene.fixtures.write("file2.txt", "def");

    scene
        .ucmd()
        .args(&["-C", "-s", "2", "file1.txt", "file2.txt"])
        .succeeds()
        .stdout_is(
            "00000002  63 64 65 66                                       |cdef|\n00000006\n",
        );
}

#[test]
fn test_hexdump_multiple_files_with_length() {
    let scene = TestScenario::new("hexdump");
    scene.fixtures.write("file1.txt", "abcdefgh");
    scene.fixtures.write("file2.txt", "ijklmnop");

    scene
        .ucmd()
        .args(&["-C", "-n", "10", "file1.txt", "file2.txt"])
        .succeeds()
        .stdout_is(
            "00000000  61 62 63 64 65 66 67 68  69 6a                    |abcdefghij|\n0000000a\n",
        );
}

#[test]
fn test_hexdump_multiple_files_with_skip_and_length() {
    let scene = TestScenario::new("hexdump");
    scene.fixtures.write("file1.txt", "abcdefgh");
    scene.fixtures.write("file2.txt", "ijklmnop");

    scene
        .ucmd()
        .args(&["-C", "-s", "3", "-n", "6", "file1.txt", "file2.txt"])
        .succeeds()
        .stdout_is(
            "00000003  64 65 66 67 68 69                                 |defghi|\n00000009\n",
        );
}

#[test]
fn test_hexdump_open_error() {
    let scene = TestScenario::new("hexdump");
    scene.fixtures.write("valid1.txt", "ABC");
    scene.fixtures.write("valid2.txt", "DEF");

    scene
        .ucmd()
        .args(&["-C", "valid1.txt", "nonexistent.txt", "valid2.txt"])
        .fails()
        .code_is(1)
        .stderr_contains("cannot open 'nonexistent.txt'")
        .stdout_is(
            "00000000  41 42 43 44 45 46                                 |ABCDEF|\n00000006\n",
        );
}

#[test]
#[cfg(target_os = "linux")]
fn test_hexdump_read_error() {
    let scene = TestScenario::new("hexdump");
    scene.fixtures.write("file1.txt", "hello");
    scene.fixtures.write("file2.txt", "world");

    scene
        .ucmd()
        .args(&["-C", "file1.txt", "/proc/self/mem", "file2.txt"])
        .fails()
        .code_is(1)
        .stderr_contains("cannot read '/proc/self/mem'")
        .stdout_is(
            "00000000  68 65 6c 6c 6f 77 6f 72  6c 64                    |helloworld|\n0000000a\n",
        );
}

#[test]
fn test_hexdump_all_files_nonexistent() {
    new_ucmd!()
        .args(&["missing1.txt", "missing2.txt"])
        .fails()
        .code_is(1)
        .stderr_contains("cannot open 'missing1.txt'")
        .stderr_contains("cannot open 'missing2.txt'")
        .stderr_contains("all input file arguments failed");
}

#[test]
#[cfg(unix)]
fn test_hexdump_size_and_length_suffixes() {
    new_ucmd!()
        .args(&["-n", "1K"])
        .pipe_in("A".repeat(2048))
        .succeeds()
        .stdout_is("0000000 4141 4141 4141 4141 4141 4141 4141 4141\n*\n0000400\n");

    new_ucmd!()
        .args(&["-s", "1K"])
        .pipe_in("A".repeat(2048))
        .succeeds()
        .stdout_is("0000400 4141 4141 4141 4141 4141 4141 4141 4141\n*\n0000800\n");
}

#[test]
fn test_hexdump_invalid_skip_and_length() {
    new_ucmd!()
        .args(&["-C", "-s", "invalid"])
        .fails()
        .code_is(1)
        .stderr_contains("invalid skip");

    new_ucmd!()
        .args(&["-C", "-n", "invalid"])
        .fails()
        .code_is(1)
        .stderr_contains("invalid length");
}

#[test]
#[cfg(unix)]
fn test_hexdump_skip_beyond_file() {
    new_ucmd!()
        .args(&["-C", "-s", "100"])
        .pipe_in("ABC")
        .succeeds()
        .stdout_is("00000064\n");
}
