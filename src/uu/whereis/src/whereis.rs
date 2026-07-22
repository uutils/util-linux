// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use std::env;
use std::fs;
use std::path::Path;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("whereis.md");
const USAGE: &str = help_usage!("whereis.md");

mod options {
    pub const BINARIES: &str = "binaries";
    pub const SOURCE: &str = "source";
    pub const MAN: &str = "man";
    pub const ALL: &str = "all";
    pub const PROGRAM: &str = "program";
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let has_any_flag = matches.get_flag(options::BINARIES)
        || matches.get_flag(options::SOURCE)
        || matches.get_flag(options::MAN)
        || matches.get_flag(options::ALL);

    let show_bin =
        matches.get_flag(options::BINARIES) || matches.get_flag(options::ALL) || !has_any_flag;
    let show_source =
        matches.get_flag(options::SOURCE) || matches.get_flag(options::ALL) || !has_any_flag;
    let show_man =
        matches.get_flag(options::MAN) || matches.get_flag(options::ALL) || !has_any_flag;

    let programs: Vec<&str> = matches
        .get_many::<String>(options::PROGRAM)
        .unwrap()
        .map(|s| s.as_str())
        .collect();

    for program in &programs {
        let mut results = Vec::new();

        if show_bin {
            if let Some(path) = find_binary(program) {
                results.push(path);
            }
        }
        if show_source {
            if let Some(path) = find_source(program) {
                results.push(path);
            }
        }
        if show_man {
            if let Some(path) = find_man(program) {
                results.push(path);
            }
        }

        if results.is_empty() {
            println!("{program}:");
        } else {
            println!("{program}: {}", results.join(" "));
        }
    }

    Ok(())
}

fn find_binary(name: &str) -> Option<String> {
    let path_var = env::var("PATH").ok()?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

fn find_source(name: &str) -> Option<String> {
    let source_dirs = ["/usr/src", "/usr/local/src"];
    for dir in &source_dirs {
        let path = Path::new(dir);
        if path.exists() {
            if let Some(found) = search_source_dir(path, name) {
                return Some(found);
            }
        }
    }
    None
}

fn search_source_dir(dir: &Path, name: &str) -> Option<String> {
    let extensions = [".c", ".cpp", ".cc", ".h", ".rs", ".go"];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = search_source_dir(&path, name) {
                    return Some(found);
                }
            } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                for ext in &extensions {
                    if file_name == format!("{name}{ext}")
                        || file_name.starts_with(&format!("{name}."))
                    {
                        return Some(path.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    None
}

fn find_man(name: &str) -> Option<String> {
    // GNU whereis man directories (with glob support)
    let man_patterns = [
        "/usr/man",
        "/usr/share/man",
        "/usr/local/share/man",
        "/usr/X11/man",
        "/usr/TeX/man",
    ];

    let mut results = Vec::new();

    for man_dir in &man_patterns {
        let path = Path::new(man_dir);
        if path.exists() {
            search_man_dir_recursive(path, name, &mut results);
            if !results.is_empty() {
                break;
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results[0].clone())
    }
}

fn search_man_dir_recursive(dir: &Path, name: &str, results: &mut Vec<String>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                search_man_dir_recursive(&path, name, results);
            } else if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if man_file_matches(name, file_name) {
                    results.push(path.to_string_lossy().into_owned());
                }
            }
        }
    }
}

fn man_file_matches(name: &str, file_name: &str) -> bool {
    // Remove compression extensions for comparison
    let uncompressed = file_name
        .strip_suffix(".zst")
        .or_else(|| file_name.strip_suffix(".bz2"))
        .or_else(|| file_name.strip_suffix(".xz"))
        .or_else(|| file_name.strip_suffix(".gz"))
        .or_else(|| file_name.strip_suffix(".Z"))
        .unwrap_or(file_name);

    // Match patterns like: name.1, name.1.gz, name.1.bz2, name.1.xz, name.1.zst
    if let Some(dot_pos) = uncompressed.rfind('.') {
        let base_name = &uncompressed[..dot_pos];
        let section = &uncompressed[dot_pos + 1..];
        // Section should be a number (1-9) or number followed by letter (e.g., 3pm)
        if base_name == name
            && !section.is_empty()
            && section
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
        {
            return true;
        }
    }
    false
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::BINARIES)
                .short('b')
                .long("binaries")
                .help("Search only for binaries")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::SOURCE)
                .short('s')
                .long("source")
                .help("Search only for source files")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAN)
                .short('m')
                .long("man")
                .help("Search only for manual entries")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::ALL)
                .short('a')
                .long("all")
                .help("Search for all three types")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::PROGRAM)
                .required(true)
                .num_args(1..)
                .help("Programs to locate"),
        )
}
