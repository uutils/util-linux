// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::{collections::HashMap, fs};

use clap::{crate_version, Arg, ArgAction, Command};
use serde::Serialize;
use uucore::{error::UResult, format_usage, help_about, help_usage};

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

fn print_output(fs: MountData, options: OutputOptions) {
    if options.json {
        let json = serde_json::to_string_pretty(&fs).unwrap();
        println!("{}", json);
        return;
    }

    fn indent(depth: usize) -> usize {
        depth * 2
    }

    fn print_mount(mount: Mount, depth: usize) {
        println!(
            "{}{}\t{}\t{}\t{}",
            " ".repeat(indent(depth)),
            mount.target,
            mount.source,
            mount.fstype,
            mount.options
        );
        for child in mount.children {
            print_mount(child, depth + 1)
        }
    }

    for mount in fs.filesystems {
        print_mount(mount, 0);
    }
}

struct OutputOptions {
    json: bool,
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let output_opts = OutputOptions {
        json: matches.get_flag(options::JSON),
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
