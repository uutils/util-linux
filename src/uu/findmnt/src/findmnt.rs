// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{collections::HashMap, fs, string};

use clap::{crate_version, Arg, ArgAction, Command};
use serde::Serialize;
use uucore::{error::UResult, format_usage, help_about, help_usage};

// Flat mount information structure as described in:
// https://www.man7.org/linux/man-pages/man5/proc_pid_mountinfo.5.html
#[derive(Debug)]
struct MountEntry {
    id: usize,
    parent_id: usize,
    _root: String,
    target: String,
    source: String,
    options: String,
    fstype: String,
}

impl MountEntry {
    // Parses a line from `/proc/self/mountinfo`, which follows the format described under proc_pid_mountinfo(5)
    // We ignore some of the fields as they are not relevant for the purposes of findmnt
    fn parse(input: &str) -> Self {
        let mut parts = input.trim().split(" ");

        let id = parts
            .next()
            .unwrap()
            .parse::<usize>()
            .expect("Could not parse Mount ID");
        let parent_id = parts
            .next()
            .unwrap()
            .parse::<usize>()
            .expect("Could not parse Parent ID");
        parts.next(); // Skip field 3

        let root = parts.next().unwrap().to_string();
        let target = parts.next().unwrap().to_string();
        let options = parts.next().unwrap().to_string();

        // Field 7 is a variable-length list of space-separated optional values, it's end is marked by a `-` separator
        // Skip everything until the separator, and the separator itself
        let mut parts = parts.skip_while(|s| *s != "-").skip(1);

        let fstype = parts.next().unwrap().to_string();
        let source = parts.next().unwrap().to_string();

        // Ignore the rest

        Self {
            id,
            parent_id,
            _root: root,
            source,
            target,
            fstype,
            options,
        }
    }
}

// Data structures used for the final output
#[derive(Debug, Serialize)]
struct MountData {
    filesystems: Vec<Mount>,
}

#[derive(Debug, Serialize, Clone)]
struct Mount {
    target: String,
    source: String,
    fstype: String,
    options: String,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<Mount>,
}

impl Mount {
    fn get_value(&self, field: &Column) -> String {
        match field {
            Column::FsRoot => todo!(),
            Column::FsType => self.fstype.clone(),
            Column::FsOptions => todo!(),
            Column::Id => todo!(),
            Column::Options => self.options.clone(),
            Column::Parent => todo!(),
            Column::Source => self.source.clone(),
            Column::Target => self.target.clone(),
        }
    }
}

fn read_mounts() -> Vec<Mount> {
    let content =
        fs::read_to_string("/proc/self/mountinfo").expect("Could not read /proc/self/mountinfo");
    let mount_entries: HashMap<_, _> = content
        .lines()
        .map(MountEntry::parse)
        .map(|me| (me.id, me))
        .collect();

    // Very odd if this happens but technically possible
    if mount_entries.is_empty() {
        return vec![];
    }

    // Try finding the "proper" root mounts, ie. ones were id == parent_id
    let mut root_mounts: Vec<_> = mount_entries
        .iter()
        .filter(|(_, e)| e.parent_id == e.id)
        .map(|(_, e)| e)
        .collect();

    // In many cases there will be no "proper" roots, so just use the mount with the lowest parent_id
    if root_mounts.is_empty() {
        let root_entry = mount_entries
            .iter()
            .min_by_key(|(_, e)| e.parent_id)
            .unwrap()
            .1;
        root_mounts.push(root_entry)
    }

    fn with_children(m: &MountEntry, haystack: &HashMap<usize, MountEntry>) -> Mount {
        let child_entries: Vec<&MountEntry> = haystack
            .iter()
            .filter(|(_, e)| e.parent_id == m.id)
            .map(|(_, e)| e)
            .collect();

        // TODO: Use iterator
        let mut children: Vec<Mount> = vec![];
        for entry in child_entries {
            children.push(with_children(entry, haystack));
        }

        Mount {
            target: m.target.clone(),
            source: m.source.clone(),
            fstype: m.fstype.clone(),
            options: m.options.clone(),
            children,
        }
    }

    root_mounts
        .iter()
        .map(|e| with_children(e, &mount_entries))
        .collect()
}

// TODO: Add the remaining columns supported by `findmnt`
#[derive(Debug, Clone)]
enum Column {
    FsRoot,
    FsType,
    FsOptions,
    Id,
    Options,
    Parent,
    Source,
    Target,
}

impl Column {
    fn header_text(&self) -> &'static str {
        match self {
            Column::FsRoot => "FSROOT",
            Column::FsType => "FSTYPE",
            Column::FsOptions => "FS-OPTIONS",
            Column::Id => "ID",
            Column::Options => "OPTIONS",
            Column::Parent => "PARENT",
            Column::Source => "SOURCE",
            Column::Target => "TARGET",
        }
    }

    fn header_width(&self) -> usize {
        self.header_text().len()
    }
}

const DEFAULT_COLS: &[Column] = &[
    Column::Target,
    Column::Source,
    Column::FsType,
    Column::Options,
];

struct OutputOptions {
    json: bool,
    cols: Vec<Column>,
}

fn get_column_widths(cols: &Vec<Column>, rows: &Vec<Mount>) -> Vec<usize> {
    // Initialize max_widths with the width of the column headers
    let mut max_widths: Vec<_> = cols.iter().map(|col| col.header_width()).collect();

    // Go through all table rows, and check if any values are wider than the header text
    // Set that as the new max_width for that column
    for row in rows {
        for (i, col) in cols.iter().enumerate() {
            let value_width = row.get_value(col).len();
            max_widths[i] = max_widths[i].max(value_width);
        }
    }

    max_widths
}

fn print_output(fs: MountData, options: OutputOptions) {
    if options.json {
        let json = serde_json::to_string_pretty(&fs).unwrap();
        println!("{}", json);
        return;
    }

    // Before printing, the mount tree needs to be flatten into a single vector of rows
    let mut flattened_mounts: Vec<Mount> = vec![];

    fn flatten(mnt: &Mount, acc: &mut Vec<Mount>) {
        acc.push(mnt.clone());
        for child in &mnt.children {
            flatten(&child, acc);
        }
    }

    for rootfs in &fs.filesystems {
        flatten(rootfs, &mut flattened_mounts);
    }

    let col_widths = get_column_widths(&options.cols, &flattened_mounts);

    // Print headers
    let headers: Vec<_> = options
        .cols
        .iter()
        .enumerate()
        .map(|(i, col)| format!("{:<width$}", col.header_text(), width = col_widths[i]))
        .collect();
    println!("{}", headers.join(" "));

    for row in flattened_mounts {
        let values: Vec<_> = options
            .cols
            .iter()
            .enumerate()
            .map(|(i, col)| format!("{:<width$}", row.get_value(col), width = col_widths[i]))
            .collect();
        println!("{}", values.join(" "));
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let output_opts = OutputOptions {
        json: matches.get_flag(options::JSON),

        // TODO: Use arguments to control which cols are printed out
        cols: Vec::from(DEFAULT_COLS),
    };

    let fs = MountData {
        filesystems: read_mounts(),
    };

    print_output(fs, output_opts);

    Ok(())
}

const ABOUT: &str = help_about!("findmnt.md");
const USAGE: &str = help_usage!("findmnt.md");

mod options {
    pub const JSON: &str = "json";
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long("json")
                .help("Use JSON output format")
                .action(ArgAction::SetTrue),
        )
}
