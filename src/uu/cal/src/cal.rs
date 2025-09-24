// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use chrono::{Datelike, Local, NaiveDate, Weekday};
use clap::{crate_version, value_parser, Arg, ArgAction, ArgMatches, Command};
use std::io::IsTerminal;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("cal.md");
const USAGE: &str = help_usage!("cal.md");

#[derive(Debug, PartialEq, Eq)]
enum DisplayMode {
    ThreeMonths,
    Year,
    NMonths(u32),
}

#[derive(Debug)]
struct CalOptions {
    date: NaiveDate,
    highlight_date: NaiveDate,
    display_mode: DisplayMode,
    monday_first: bool,
    julian: bool,
    week_numbers: bool,
    color: bool,
}

const NUM_CALENDAR_LINES: usize = 8;
const NUM_SPACES_BETWEEN_CALENDARS: usize = 3;
const MAX_CALENDARS_SIDE_BY_SIDE: usize = 3;

fn calculate_field_widths(options: &CalOptions) -> (usize, usize) {
    let day_width = if options.julian { 3 } else { 2 };
    let mut line_width = 7 * (day_width + 1) - 1;
    if options.week_numbers {
        line_width += 3;
    }
    (day_width, line_width)
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;
    let options = parse_options(&matches)?;

    let date = NaiveDate::from_ymd_opt(options.date.year(), options.date.month(), 1).unwrap();
    let months: Vec<NaiveDate> = match options.display_mode {
        DisplayMode::Year => {
            let (_, line_width) = calculate_field_widths(&options);
            let total_width = MAX_CALENDARS_SIDE_BY_SIDE * line_width
                + (MAX_CALENDARS_SIDE_BY_SIDE - 1) * NUM_SPACES_BETWEEN_CALENDARS;
            println!("{:^width$}", options.date.year(), width = total_width);
            println!();

            (1..=12)
                .map(|month| NaiveDate::from_ymd_opt(options.date.year(), month, 1).unwrap())
                .collect()
        }
        DisplayMode::ThreeMonths => {
            vec![
                date - chrono::Months::new(1),
                date,
                date + chrono::Months::new(1),
            ]
        }
        DisplayMode::NMonths(count) => (0..count).map(|x| date + chrono::Months::new(x)).collect(),
    };
    print_months_side_by_side(&months, &options);
    Ok(())
}

fn parse_options(matches: &ArgMatches) -> UResult<CalOptions> {
    let now = Local::now();

    let args: Vec<&str> = matches
        .get_many::<String>("args")
        .unwrap_or_default()
        .map(|s| s.as_str())
        .collect();

    let mut year_mode = false;
    let mut full_date_provided = false;

    let date = match args.len() {
        0 => now.date_naive(),
        1 => {
            // One argument - if numeric, a year, else a month
            if args[0].parse::<i32>().is_ok() {
                year_mode = true;
                try_parse_date(args[0], "1", "1")?
            } else {
                try_parse_date(&now.year().to_string(), args[0], "1")?
            }
        }
        2 => {
            // month year
            try_parse_date(args[1], args[0], "1")?
        }
        3 => {
            // day month year
            full_date_provided = true;
            try_parse_date(args[2], args[1], args[0])?
        }
        _ => unreachable!(),
    };

    let highlight_date = if full_date_provided {
        date
    } else {
        now.date_naive()
    };

    let display_mode = if year_mode || matches.get_flag("year") {
        DisplayMode::Year
    } else if matches.get_flag("twelve") {
        DisplayMode::NMonths(12)
    } else if matches.get_flag("three") {
        DisplayMode::ThreeMonths
    } else if let Some(count) = matches.get_one::<u32>("months").cloned() {
        DisplayMode::NMonths(count.max(1))
    } else {
        DisplayMode::NMonths(1)
    };

    Ok(CalOptions {
        date,
        highlight_date,
        display_mode,
        monday_first: matches.get_flag("monday"),
        julian: matches.get_flag("julian"),
        week_numbers: matches.get_flag("week"),
        color: match matches.get_one::<String>("color").unwrap().as_str() {
            "always" => true,
            "never" => false,
            "auto" => std::io::stdout().is_terminal(),
            _ => unreachable!(),
        },
    })
}

fn try_parse_date(year: &str, month: &str, day: &str) -> UResult<NaiveDate> {
    let date_str = format!("{}-{}-{}", year, month, day);
    let formats = [
        "%Y-%m-%d", // "1992-8-25"
        "%Y-%B-%d", // "1992-august-25"
    ];

    for format in &formats {
        if let Ok(date) = NaiveDate::parse_from_str(&date_str, format) {
            return Ok(date);
        }
    }

    Err(uucore::error::USimpleError::new(1, "invalid date"))
}

fn print_months_side_by_side(months: &[NaiveDate], options: &CalOptions) {
    for chunk in months.chunks(MAX_CALENDARS_SIDE_BY_SIDE) {
        let all_calendars: Vec<_> = chunk
            .iter()
            .map(|&date| generate_month_lines(date, options))
            .collect();

        for line_idx in 0..NUM_CALENDAR_LINES {
            let output_line = all_calendars
                .iter()
                .map(|calendar| calendar[line_idx].as_str())
                .collect::<Vec<_>>()
                .join(&" ".repeat(NUM_SPACES_BETWEEN_CALENDARS));
            println!("{}", output_line);
        }
    }
}

fn us_week_number(date: NaiveDate) -> i64 {
    let jan1 = NaiveDate::from_ymd_opt(date.year(), 1, 1).unwrap();

    let days_before = jan1.weekday().num_days_from_sunday();
    let first_sunday = jan1 - chrono::Duration::days(days_before as i64);

    (date - first_sunday).num_days() / 7 + 1
}

fn get_weekday_abbreviations(start_weekday: Weekday, length: usize) -> Vec<String> {
    let mut weekday = start_weekday;
    let mut ret = vec![];
    for _ in 0..7 {
        ret.push(weekday.to_string()[..length].to_string());
        weekday = weekday.succ();
    }
    ret
}

fn generate_month_lines(date: NaiveDate, options: &CalOptions) -> Vec<String> {
    let (day_width, line_width) = calculate_field_widths(options);

    // Year mode shows year number once at the very top, not per each month
    let fmt = if options.display_mode == DisplayMode::Year {
        "%B"
    } else {
        "%B %Y"
    };
    let mut lines = vec![format!("{:^width$}", date.format(fmt), width = line_width)];

    let week_start = if options.monday_first {
        Weekday::Mon
    } else {
        Weekday::Sun
    };

    lines.push(format!(
        "{}{}",
        if options.week_numbers { "   " } else { "" },
        get_weekday_abbreviations(week_start, day_width).join(" ")
    ));

    let mut d = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).unwrap();
    let mut current_line = String::new();
    while d.month() == date.month() {
        if options.week_numbers && current_line.is_empty() {
            if options.monday_first {
                current_line.push_str(&format!("{:2} ", d.iso_week().week()));
            } else {
                current_line.push_str(&format!("{:2} ", us_week_number(d)));
            }
        }

        // Space pad the days that belong to the same week but in previous month
        if d.day() == 1 {
            let num_padding_days = (d - d.week(week_start).first_day()).num_days() as usize;
            current_line.push_str(&" ".repeat(num_padding_days * (day_width + 1)));
        }

        let day_str = if options.julian {
            format!("{:width$}", d.ordinal(), width = day_width)
        } else {
            format!("{:width$}", d.day(), width = day_width)
        };

        // Apply reverse video attribute to the highlighted day
        let formatted_day = if options.color && options.highlight_date == d {
            format!("\x1b[7m{}\x1b[0m", day_str)
        } else {
            day_str
        };

        current_line.push_str(&format!("{} ", formatted_day));

        d += chrono::Duration::days(1);

        if d.weekday() == week_start {
            lines.push(current_line.trim_end().to_string());
            current_line.clear();
        }
    }

    if !current_line.is_empty() {
        // Original cal pads all lines to fixed length
        // (also print_months_side_by_side relies on this).
        lines.push(format!(
            "{:<width$}",
            current_line.trim_end(),
            width = line_width
        ));
    }
    while lines.len() < NUM_CALENDAR_LINES {
        lines.push(" ".repeat(line_width));
    }

    lines
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("year")
                .short('y')
                .long("year")
                .help("show whole year")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["twelve", "months"]),
        )
        .arg(
            Arg::new("three")
                .short('3')
                .long("three")
                .help("show previous, current and next month")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("twelve")
                .short('Y')
                .long("twelve")
                .help("show the next twelve months")
                .action(ArgAction::SetTrue)
                .conflicts_with_all(["year", "months"]),
        )
        .arg(
            Arg::new("months")
                .short('n')
                .long("months")
                .help("show this many months")
                .value_parser(value_parser!(u32))
                .action(ArgAction::Set)
                .conflicts_with_all(["year", "twelve"]),
        )
        .arg(
            Arg::new("monday")
                .short('m')
                .long("monday")
                .help("Monday as first day of week")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("julian")
                .short('j')
                .long("julian")
                .help("use day-of-year numbering")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("week")
                .short('w')
                .long("week")
                .help("show week numbers")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("color")
                .long("color")
                .help("colorize the output")
                .value_name("when")
                .value_parser(["always", "auto", "never"])
                .default_missing_value("auto")
                .default_value("auto")
                .require_equals(true)
                .num_args(0..=1)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new("args")
                .help("date arguments")
                .action(ArgAction::Append)
                .num_args(0..=3),
        )
}
