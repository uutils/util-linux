// This file is a part of the uutils util-linux package.
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use glob::glob;
use serde::Serialize;
use std::{
    collections::HashMap, collections::HashSet, fs, os::unix::fs::MetadataExt, path::Path,
    path::PathBuf,
};
use uucore::{error::UResult, format_usage, help_about, help_usage};

mod constants;
use crate::constants::{BIN_DIRS, MAN_DIRS, SRC_DIRS};

mod options {
    pub const BIN: &str = "binaries";
    pub const MAN: &str = "manuals";
    pub const SRC: &str = "sources";
    pub const PATH: &str = "lookups";

    pub const SPECIFIED_BIN: &str = "listed binaries";
    pub const SPECIFIED_MAN: &str = "listed manuals";
    pub const SPECIFIED_SRC: &str = "listed sources";
}

const ABOUT: &str = help_about!("whereis.md");
const USAGE: &str = help_usage!("whereis.md");

// Directories are usually manual pages dirs, binary dirs or source dirs. Hopefully not unknown.
#[derive(Serialize, Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum DirType {
    BIN,
    MAN,
    SRC,
    UNK,
}

// Store the metadata for a file
#[derive(Serialize, Clone, Debug)]
pub struct WhDir {
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    metadata: Option<fs::Metadata>,
    path: PathBuf,
    type_of_dir: DirType,
}

impl WhDir {
    fn new(path: PathBuf, type_of_dir: DirType) -> Self {
        Self {
            metadata: fs::metadata(&path).ok(),
            path,
            type_of_dir,
        }
    }
}

// Use a vector to store the list of directories. Additionally keep a HashSet of the inode number and st_dev ID.
#[derive(Serialize, Debug)]
pub struct WhDirList {
    list: Vec<WhDir>,
    seen_files: HashSet<(u64, u64)>,
}

impl WhDirList {
    fn new() -> Self {
        Self {
            list: Vec::new(),
            seen_files: HashSet::new(),
        }
    }

    fn construct_dir_list(&mut self, dir_type: DirType, paths: &[&str]) {
        for path in paths {
            let pathbuf = PathBuf::from(path);
            if path.contains('*') {
                self.add_sub_dirs(&pathbuf, dir_type);
            } else {
                self.add_dir(WhDir::new(pathbuf, dir_type));
            }
        }
    }

    // Use (ino) inode number and (st_dev) ID of device containing the file to keep track of whats unique.
    fn add_dir(&mut self, dir: WhDir) {
        if self.list.iter().any(|d| d.path == dir.path) {
            return;
        }

        if dir.metadata.is_some() {
            let dev = dir.metadata.clone().unwrap().dev();
            let ino = dir.metadata.clone().unwrap().ino();

            if self.seen_files.insert((dev, ino)) {
                self.list.push(dir);
            }
        }
    }

    #[allow(dead_code)]
    fn remove_dir(&mut self, dir: &WhDir) {
        self.list.retain(|d| d.path != dir.path);
    }

    // TODO: We need to do something with the entry if an error occurs.
    fn add_sub_dirs(&mut self, parent_dir: &Path, dir_type: DirType) {
        for entry in glob(&parent_dir.display().to_string()).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) if path.is_dir() => {
                    self.add_dir(WhDir::new(path, dir_type));
                }
                Ok(_) => todo!(),
                Err(_e) => todo!(),
            }
        }
    }

    // A debug function.
    #[allow(dead_code)]
    fn list_dirs(&self) {
        for dir in &self.list {
            let dir_type = whereis_type_to_name(dir.type_of_dir);
            println!("{:?} : {:?}", dir_type, dir.path.display());
        }
    }

    fn lookup(&self, pattern: &str, dir_type: DirType) -> Vec<String> {
        let mut results = Vec::new();
        let pathbuf_pattern = PathBuf::from(pattern);

        for dir in &self.list {
            if dir.type_of_dir == dir_type {
                find_in(&dir.path, &pathbuf_pattern, &mut results, dir.type_of_dir);
            }
        }

        results
    }
}

pub fn whereis_type_to_name(dir_type: DirType) -> &'static str {
    match dir_type {
        DirType::MAN => "man",
        DirType::BIN => "bin",
        DirType::SRC => "src",
        DirType::UNK => "???",
    }
}

// Almost an exact ripoff from the C source.
fn filename_equal(cp: &PathBuf, dp: &str, dir_type: DirType) -> bool {
    let cp_str = match cp.file_name().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };

    let mut dp_trimmed = dp;

    if dir_type == DirType::SRC && dp_trimmed.starts_with("s.") {
        return filename_equal(cp, &dp_trimmed[2..], dir_type);
    }

    if dir_type == DirType::MAN {
        for ext in [".Z", ".gz", ".xz", ".bz2", ".zst"] {
            if let Some(stripped) = dp_trimmed.strip_suffix(ext) {
                dp_trimmed = stripped;
                break;
            }
        }
    }

    let mut cp_chars = cp_str.chars();
    let mut dp_chars = dp_trimmed.chars();

    loop {
        match (cp_chars.next(), dp_chars.next()) {
            (Some(c1), Some(c2)) if c1 == c2 => continue,
            (None, None) => return true, // both ended
            (None, Some('.')) if dir_type != DirType::BIN => {
                // cp ended, dp has .section
                return true;
            }
            _ => return false,
        }
    }
}

fn find_in(dir: &Path, pathbuf: &PathBuf, results: &mut Vec<String>, dir_type: DirType) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                if filename_equal(pathbuf, filename, dir_type) {
                    results.push(path.display().to_string());
                }
            }
        }
    }
}

// TODO: Doesn't completely all possible options like specified_bin, etc.
fn print_output(options: &OutputOptions, pattern: &str, results: Vec<String>) {
    let mut grouped: HashMap<DirType, Vec<String>> = HashMap::new();

    // Split results by type, grouping MAN, BIN and SRC.
    for path in results {
        if path.contains("/bin/") {
            grouped.entry(DirType::BIN).or_default().push(path);
        } else if path.contains("/man") || path.contains("/share/man") {
            grouped.entry(DirType::MAN).or_default().push(path);
        } else {
            grouped.entry(DirType::SRC).or_default().push(path);
        }
    }

    print!("{}:", pattern);

    // If *any* of the search flags are set, print according to them
    if options.search_bin || options.search_man || options.search_src {
        if options.search_bin {
            if let Some(paths) = grouped.get(&DirType::BIN) {
                for path in paths {
                    print!(" {}", path);
                }
            }
        }
        if options.search_man {
            if let Some(paths) = grouped.get(&DirType::MAN) {
                for path in paths {
                    print!(" {}", path);
                }
            }
        }
        if options.search_src {
            if let Some(paths) = grouped.get(&DirType::SRC) {
                for path in paths {
                    print!(" {}", path);
                }
            }
        }
    } else {
        // No -b/-m/-s flag given? Print everything
        for paths in grouped.values() {
            for path in paths {
                print!(" {}", path);
            }
        }
    }

    println!();
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let output_options = OutputOptions {
        search_bin: matches.get_flag(options::BIN),
        search_man: matches.get_flag(options::MAN),
        search_src: matches.get_flag(options::SRC),
        path_given: matches.get_flag(options::PATH),

        search_specific_bin: matches.get_flag(options::SPECIFIED_BIN),
        search_specific_man: matches.get_flag(options::SPECIFIED_MAN),
        search_specific_src: matches.get_flag(options::SPECIFIED_SRC),
    };

    let mut dir_list = WhDirList::new();

    dir_list.construct_dir_list(DirType::BIN, &BIN_DIRS);
    dir_list.construct_dir_list(DirType::MAN, &MAN_DIRS);
    dir_list.construct_dir_list(DirType::SRC, &SRC_DIRS);

    let names: Vec<_> = matches
        .get_many::<String>("names")
        .unwrap()
        .map(|s| s.as_str())
        .collect();

    // Search for the names that were passed into the program.
    for pattern in names {
        let mut results = dir_list.lookup(pattern, DirType::BIN);
        results.append(&mut dir_list.lookup(pattern, DirType::MAN));
        results.append(&mut dir_list.lookup(pattern, DirType::SRC));

        print_output(&output_options, pattern, results);
    }

    Ok(())
}

// TODO: Implement the necessary behavior for path_given and other fields with the dead_code macro.
struct OutputOptions {
    search_bin: bool,
    search_man: bool,
    search_src: bool,

    #[allow(dead_code)]
    path_given: bool,

    #[allow(dead_code)]
    search_specific_bin: bool,

    #[allow(dead_code)]
    search_specific_man: bool,

    #[allow(dead_code)]
    search_specific_src: bool,
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)

		.arg(
			Arg::new("names")
				.help("The name of the program [s] to search for.")
				.num_args(1..)
				.required(true)
		)
        .arg(
            Arg::new(options::BIN)
                .short('b')
                .long("binaries")
                .action(ArgAction::SetTrue)
                .help("Search for binaries.")
                .required(false),
        )
        .arg(
            Arg::new(options::MAN)
                .short('m')
                .long("manual")
                .help("Search for manuals.")
                .action(ArgAction::SetTrue)
				.required(false),
        )
        .arg(
            Arg::new(options::SRC)
                .short('s')
                .long("source")
                .action(ArgAction::SetTrue)
                .help("Search for sources.")
				.action(ArgAction::SetTrue)
				.required(false),
        )
		.arg(
            Arg::new(options::SPECIFIED_BIN)
                .short('B')
                .long("bins")
                .action(ArgAction::SetTrue)
                .help(
					"Limit the places where whereis searches for binaries, \
				    by a whitespace-separated list of directories."	
                )
				.action(ArgAction::SetTrue)
				.required(false),
        )
		.arg(
            Arg::new(options::SPECIFIED_MAN)
                .short('M')
                .long("mans")
                .action(ArgAction::SetTrue)
                .help(
					 "Limit the places where whereis searches for manuals and documentation in Info \
           			 format, by a whitespace-separated list of directories."
                )
				.action(ArgAction::SetTrue)
				.required(false),
        )
		.arg(
            Arg::new(options::SPECIFIED_SRC)
                .short('S')
                .long("sources")
                .action(ArgAction::SetTrue)
                .help(
					"Limit the places where whereis searches for sources, by a whitespace-separated \
		            list of directories."
                )
				.action(ArgAction::SetTrue)
				.required(false),
        )

		// Want to rename this in the future. 
		.arg(
            Arg::new(options::PATH)
                .short('u')
                .long("source path")
                .action(ArgAction::SetTrue)
                .help(
					"Only show the command names that have unusual entries. A command is said to be \
				    unusual if it does not have just one entry of each explicitly requested type. \
                    Thus 'whereis -m -u *' asks for those files in the current directory which \
				    have no documentation file, or more than one."
                )
				.action(ArgAction::SetTrue)
				.required(false),
        )
}
