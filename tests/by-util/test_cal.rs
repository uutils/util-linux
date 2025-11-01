// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use regex::Regex;
use uutests::new_ucmd;

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

#[test]
fn test_invalid_dates() {
    new_ucmd!().args(&["31", "2", "2000"]).fails().code_is(1);
    new_ucmd!().args(&["13", "2000"]).fails().code_is(1);
}

#[test]
fn test_iso_week_numbers() {
    let expected = [
        "     January 2021      \n",
        "   Mo Tu We Th Fr Sa Su\n",
        "53              1  2  3\n",
        " 1  4  5  6  7  8  9 10\n",
        " 2 11 12 13 14 15 16 17\n",
        " 3 18 19 20 21 22 23 24\n",
        " 4 25 26 27 28 29 30 31\n",
        "                       \n",
    ];
    new_ucmd!()
        .args(&["-w", "-m", "1", "2021"])
        .succeeds()
        .stdout_is(expected.join(""));

    let expected = [
        "     January 2015      \n",
        "   Mo Tu We Th Fr Sa Su\n",
        " 1           1  2  3  4\n",
        " 2  5  6  7  8  9 10 11\n",
        " 3 12 13 14 15 16 17 18\n",
        " 4 19 20 21 22 23 24 25\n",
        " 5 26 27 28 29 30 31   \n",
        "                       \n",
    ];
    new_ucmd!()
        .args(&["-w", "-m", "1", "2015"])
        .succeeds()
        .stdout_is(expected.join(""));
}

#[test]
fn test_us_week_numbers() {
    let expected = [
        "     January 2021      \n",
        "   Su Mo Tu We Th Fr Sa\n",
        " 1                 1  2\n",
        " 2  3  4  5  6  7  8  9\n",
        " 3 10 11 12 13 14 15 16\n",
        " 4 17 18 19 20 21 22 23\n",
        " 5 24 25 26 27 28 29 30\n",
        " 6 31                  \n",
    ];
    new_ucmd!()
        .args(&["-w", "1", "2021"])
        .succeeds()
        .stdout_is(expected.join(""));
}

#[test]
fn test_julian() {
    let expected = [
        "       December 2000       \n",
        "Sun Mon Tue Wed Thu Fri Sat\n",
        "                    336 337\n",
        "338 339 340 341 342 343 344\n",
        "345 346 347 348 349 350 351\n",
        "352 353 354 355 356 357 358\n",
        "359 360 361 362 363 364 365\n",
        "366                        \n",
    ];
    new_ucmd!()
        .args(&["-j", "12", "2000"])
        .succeeds()
        .stdout_is(expected.join(""));

    let expected = [
        "        February 2024         \n",
        "   Mon Tue Wed Thu Fri Sat Sun\n",
        " 5              32  33  34  35\n",
        " 6  36  37  38  39  40  41  42\n",
        " 7  43  44  45  46  47  48  49\n",
        " 8  50  51  52  53  54  55  56\n",
        " 9  57  58  59  60            \n",
        "                              \n",
    ];
    new_ucmd!()
        .args(&["-j", "-w", "-m", "2", "2024"])
        .succeeds()
        .stdout_is(expected.join(""));
}

#[test]
fn test_single_month_param() {
    new_ucmd!()
        .args(&["aug"])
        .succeeds()
        .stdout_contains("August");
}

#[test]
fn test_single_year_param() {
    new_ucmd!()
        .args(&["2024"])
        .succeeds()
        .stdout_contains(" 2024 ")
        .stdout_matches(&Regex::new("January +February +March").unwrap())
        .stdout_matches(&Regex::new("October +November +December").unwrap());
}

#[test]
fn test_year_option() {
    new_ucmd!()
        .args(&["-y", "3", "2024"])
        .succeeds()
        .stdout_matches(&Regex::new("January +February +March").unwrap())
        .stdout_matches(&Regex::new("October +November +December").unwrap());
}

#[test]
fn test_three_option() {
    let re = Regex::new("December 2023 +January 2024 +February 2024").unwrap();
    new_ucmd!()
        .args(&["-3", "1", "2024"])
        .succeeds()
        .stdout_matches(&re);

    let re = Regex::new("November 2023 +December 2023 +January 2024").unwrap();
    new_ucmd!()
        .args(&["-3", "12", "2023"])
        .succeeds()
        .stdout_matches(&re);
}

#[test]
fn test_twelve_option() {
    new_ucmd!()
        .args(&["-Y", "15", "3", "2024"])
        .succeeds()
        .stdout_contains("March 2024")
        .stdout_contains("February 2025");
}

#[test]
fn test_zero_months_displays_one() {
    new_ucmd!()
        .args(&["-n", "0", "12", "2023"])
        .succeeds()
        .stdout_contains("Su Mo Tu We Th Fr Sa");
}

#[test]
fn test_color() {
    let expected = [
        "     March 2024     \n",
        "Su Mo Tu We Th Fr Sa\n",
        "                1  2\n",
        " 3  4  5  6  7  8  9\n",
        "10 11 12 13 14 15 16\n",
        "17 18 19 20 21 22 23\n",
        "24 25 26 27 28 29 30\n",
        "31                  \n",
    ];
    new_ucmd!()
        .args(&["--color=never", "15", "3", "2024"])
        .succeeds()
        .stdout_is(expected.join(""));

    let expected = [
        "     March 2024     \n",
        "Su Mo Tu We Th Fr Sa\n",
        "                1  2\n",
        " 3  4  5  6  7  8  9\n",
        "10 11 12 13 14 \x1b[7m15\x1b[0m 16\n",
        "17 18 19 20 21 22 23\n",
        "24 25 26 27 28 29 30\n",
        "31                  \n",
    ];
    new_ucmd!()
        .args(&["--color=always", "15", "3", "2024"])
        .succeeds()
        .stdout_is(expected.join(""));
}
