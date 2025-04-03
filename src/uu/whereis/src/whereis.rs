// This file is a part of the uutils util-linux package.

// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use serde::Serialize;
use std::{fs, path::PathBuf};
use uucore::{error::UResult, format_usage, help_about, help_usage};

mod constants;
use crate::constants::{SRCDIRS, BINDIRS, MANDIRS};

mod options {
    pub const BYTES: &str = "bytes";
    pub const HEX: &str = "hex";
    pub const JSON: &str = "json";
}

const ABOUT: &str = help_about!("whereis.md");
const USAGE: &str = help_usage!("whereis.md");

#[derive(Serialize, Clone, Debug)]
pub enum DirType {
	MAN,
	BIN,
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
	type_of_dir: DirType
} 

impl WhDir {

	fn new (path: PathBuf, type_of_dir: DirType) -> Self {
		Self {
			metadata: fs::metadata(&path).ok(),
			path,
			type_of_dir,
		}
	}

}

// Use a vector to store the list of directories.
#[derive(Serialize)]
pub struct WhDirList {	
	list: Vec<WhDir>
}

impl WhDirList {

	fn new () -> Self {
		Self { 
			list: Vec::new(), 
		}	
	}

	fn construct_dir_list (&mut self, dir_type: DirType, paths: &[&str]) {

		for path in paths {
		 	let pathbuf = PathBuf::from(path);
			if !path.contains('*') {
				self.add_dir (WhDir::new(pathbuf, dir_type.clone()));
			} else {
				self.add_sub_dirs(&pathbuf, dir_type.clone());
			} 
		}
	}


	fn add_dir (&mut self, dir: WhDir)  {

		// use (ino) inode number and (st_dev) 
		if self.list.iter().any(|d| d.path == dir.path) {
			return;
		}	
		
		if let Some(metadata) = &dir.metadata {
			if metadata.permissions().readonly() {
				self.list.push(dir);
			} 
		}
	}

	fn add_sub_dirs (&mut self, parent_dir: &PathBuf, dir_type: DirType) {

		if let Ok(entries) = fs::read_dir(parent_dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if path.is_dir() {
					self.add_dir(WhDir::new(path, dir_type.clone()));
				} 
			} 
		}
	}

	fn remove_dir (&mut self, dir: &WhDir) {
		self.list.retain(|d| d.path != dir.path);
	}

}

// The output options struct
struct OutputOptions {
    bytes: bool,
    json: bool,
    _hex: bool,
}

pub fn whereis_type_to_name (dir_type: &DirType) -> &'static str {

	match dir_type {
		DirType::MAN => "man",
		DirType::BIN => "bin",
		DirType::SRC => "src",
		DirType::UNK => "???",
	}
}


// pub fn build_dir_list (dir_list: &WhDirList)  -> None { }


#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult <()> {

	let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let _output_opts = OutputOptions {
        bytes: matches.get_flag(options::BYTES),
        _hex: matches.get_flag(options::HEX),
        json: matches.get_flag(options::JSON),
    };

	
	Ok(())
}


// Fix this, there is -b -B <dirs> -m -M <dirs> -s -S <dirs> -f -u -g and -i
pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::HEX)
                .short('x')
                .long("hex")
                .action(ArgAction::SetTrue)
                .help(
                    "Use hexadecimal masks for CPU sets (for example 'ff'). \
                    The default is to print the sets in list format (for example 0,1).",
                )
                .required(false),
        )
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long("json")
                .help(
                    "Use JSON output format for the default summary or extended output \
                    (see --extended). For backward compatibility, JSON output follows the \
                    default summary behavior for non-terminals (e.g., pipes) where \
                    subsections are missing. See also --hierarchic.",
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::BYTES)
                .short('B')
                .long("bytes")
                .action(ArgAction::SetTrue)
                .help(
                    "Print the sizes in bytes rather than in a human-readable format. \
                    The default is to print sizes in human-readable format (for example '512 KiB'). \
                    Setting this flag instead prints the decimal amount of bytes with no suffix.",
                ),
        )
}

