// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
#[cfg(feature = "selinux")]
use selinux::SecurityContext;
use std::env;
use std::process;
use std::str::FromStr;
use std::{cmp::max, fs, os::unix::fs::MetadataExt, path::Path};
use uucore::entries::{gid2grp, uid2usr};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("namei.md");
const USAGE: &str = help_usage!("namei.md");

const MAXSYMLINKS: usize = 256;

mod options {
    pub const LONG: &str = "long";
    pub const MODES: &str = "modes";
    pub const NOSYMLINKS: &str = "nosymlinks";
    pub const OWNERS: &str = "owners";
    pub const VERTICAL: &str = "vertical";
    pub const MOUNTPOINTS: &str = "mountpoints";
    pub const PATHNAMES: &str = "pathnames";

    #[cfg(feature = "selinux")]
    pub const CONTEXT: &str = "context";
}

struct OutputOptions {
    long: bool,
    modes: bool,
    nosymlinks: bool,
    owners: bool,
    vertical: bool,
    mountpoints: bool,

    #[cfg(feature = "selinux")]
    context: bool,
}

pub fn uu_app() -> Command {
    let cmd = Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .arg(
            Arg::new(options::PATHNAMES)
                .value_name("PATH")
                .help("Paths to follow")
                .hide(true)
                .action(ArgAction::Append)
                .required(true)
                .num_args(1..),
        )
        .arg(
            Arg::new(options::LONG)
                .short('l')
                .long("long")
                .help("use a long listing format (-m -o -v)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MODES)
                .short('m')
                .long("modes")
                .help("show the mode bits of each file")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::NOSYMLINKS)
                .short('n')
                .long("nosymlinks")
                .help("don't follow symlinks")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::OWNERS)
                .short('o')
                .long("owners")
                .help("show owner and group name of each file")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::VERTICAL)
                .short('v')
                .long("vertical")
                .help("vertical align of modes and owners")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MOUNTPOINTS)
                .short('x')
                .long("mountpoints")
                .help("show mount point directories with a 'D'")
                .action(ArgAction::SetTrue),
        );

    #[cfg(feature = "selinux")]
    return cmd.arg(
        Arg::new(options::CONTEXT)
            .short('Z')
            .long("context")
            .help("print any security context of each file")
            .action(ArgAction::SetTrue),
    );

    #[cfg(not(feature = "selinux"))]
    return cmd;
}

fn max_owner_length(path: &Path) -> usize {
    let mut max_length = 0;

    for entry in path.ancestors() {
        if let Err(_e) = entry.metadata() {
            continue;
        }
        let metadata = entry.metadata().unwrap();
        let uid = metadata.uid();

        let owner = uid2usr(uid).unwrap();
        let max_entry_length = owner.len();
        max_length = max(max_entry_length, max_length);
    }

    max_length
}

fn max_group_length(path: &Path) -> usize {
    let mut max_length = 0;

    for entry in path.ancestors() {
        if let Err(_e) = entry.metadata() {
            continue;
        }
        let metadata = entry.metadata().unwrap();
        let gid = metadata.gid();

        let group = gid2grp(gid).unwrap();
        let max_entry_length = group.len();
        max_length = max(max_entry_length, max_length);
    }

    max_length
}

fn get_file_name(input: &str) -> &str {
    let stripped = input.trim_end_matches('/');
    if stripped.is_empty() {
        return "/";
    }
    stripped.rsplit('/').next().unwrap_or("/")
}

fn is_mount_point(input_path: &Path) -> bool {
    let canonical_path = input_path.canonicalize().unwrap().into_os_string();
    let path = Path::new(&canonical_path);
    if let Some(parent) = path.parent() {
        let metadata = fs::metadata(path).unwrap();
        if let Err(_e) = fs::metadata(parent) {
            return false;
        }
        let parent_metadata = fs::metadata(parent).unwrap();
        metadata.dev() != parent_metadata.dev()
    } else {
        true
    }
}

fn get_prefix(
    level: usize,
    path: &Path,
    output_opts: &OutputOptions,
    maximum_owner_length: usize,
    maximum_group_length: usize,
) -> String {
    let mut prefix = String::new();

    if !output_opts.vertical {
        let mut st = String::from(" ");
        st.push_str(&" ".repeat(level * 2));
        prefix.push_str(&st);
    }

    if let Err(_e) = fs::metadata(path) {
        let mut blanks = 1 + level * 2;
        if output_opts.modes {
            blanks += 9;
        }
        if output_opts.owners {
            blanks += maximum_owner_length + maximum_group_length + 2;
        }
        if output_opts.vertical {
            blanks += 1;
        }

        #[cfg(feature = "selinux")]
        if !output_opts.context {
            blanks += 1;
        }

        prefix = " ".repeat(blanks);
        return prefix;
    }

    let metadata = fs::metadata(path).unwrap();

    let mode = metadata.mode();

    let file_type = match mode & 0o170000 {
        0o100000 => '-',                                                    // Regular file
        0o040000 if output_opts.mountpoints && is_mount_point(path) => 'D', // Directory
        0o040000 => 'd',                                                    // Directory
        0o120000 => 'l',                                                    // Symbolic link
        0o020000 => 'c',                                                    // Character device
        0o060000 => 'b',                                                    // Block device
        0o010000 => 'p',                                                    // FIFO
        0o140000 => 's',                                                    // Socket
        _ => '?',                                                           // Unknown
    };

    if path.is_symlink() {
        prefix.push('l');
    } else {
        prefix.push(file_type);
    }

    if output_opts.modes || output_opts.long {
        let permissions = [
            (mode & 0o400, 'r'),
            (mode & 0o200, 'w'),
            (mode & 0o100, 'x'), // Owner
            (mode & 0o040, 'r'),
            (mode & 0o020, 'w'),
            (mode & 0o010, 'x'), // Group
            (mode & 0o004, 'r'),
            (mode & 0o002, 'w'),
            (mode & 0o001, 'x'), // Others
        ];
        let mut perm_string = String::new();
        for &(bit, ch) in &permissions {
            perm_string.push(if bit != 0 { ch } else { '-' });
        }
        if mode & 0o4000 != 0 {
            // Set UID
            perm_string.replace_range(
                2..3,
                if perm_string.chars().nth(3) == Some('x') {
                    "s"
                } else {
                    "S"
                },
            );
        }
        if mode & 0o2000 != 0 {
            // Set GID
            perm_string.replace_range(
                5..6,
                if perm_string.chars().nth(6) == Some('x') {
                    "s"
                } else {
                    "S"
                },
            );
        }
        if mode & 0o1000 != 0 {
            // Sticky Bit
            perm_string.replace_range(
                8..9,
                if perm_string.chars().nth(9) == Some('x') {
                    "t"
                } else {
                    "T"
                },
            );
        }

        prefix.push_str(&perm_string);
    }
    prefix.push(' ');

    if output_opts.owners {
        let uid = metadata.uid();
        let gid = metadata.gid();
        let mut owner = uid2usr(uid).unwrap();
        let str1 = " ".repeat(maximum_owner_length - owner.len() + 1);
        owner = format!("{}{}", owner, str1);
        let mut group = gid2grp(gid).unwrap();
        let str2 = " ".repeat(maximum_group_length - group.len() + 1);
        group = format!("{}{}", group, str2);

        prefix = format!("{}{}{}", prefix, owner, group);
    }

    #[cfg(feature = "selinux")]
    if output_opts.context {
        let context_not_available_string: String = '?'.to_string();
        match SecurityContext::of_path(path, !output_opts.nosymlinks, false) {
            Err(_r) => prefix.push_str(context_not_available_string.as_str()),
            Ok(None) => prefix.push_str(context_not_available_string.as_str()),
            Ok(Some(cntxt)) => {
                let context = cntxt.as_bytes();
                let context = context.strip_suffix(&[0]).unwrap_or(context);
                prefix.push_str(
                    String::from_utf8(context.to_vec())
                        .unwrap_or_else(|_e| String::from_utf8_lossy(context).into_owned())
                        .as_str(),
                )
            }
        }
        prefix.push_str("  ");
    }

    if output_opts.vertical {
        let mut st = String::new();
        st.push_str(&" ".repeat(level * 2));
        prefix.push_str(&st);
    }

    prefix
}

fn print_files(
    level: usize,
    path: &Path,
    output_opts: &OutputOptions,
    maximum_owner_length: usize,
    maximum_group_length: usize,
) {
    if let Some(pt) = path.parent() {
        print_files(
            level,
            pt,
            output_opts,
            maximum_owner_length,
            maximum_group_length,
        );
    }

    let prefix = get_prefix(
        level,
        path,
        output_opts,
        maximum_owner_length,
        maximum_group_length,
    );

    let symlinksuffix = if path.is_symlink() {
        let mut suffix = String::from_str(" -> ").unwrap();
        let target = fs::read_link(path).unwrap();
        suffix.push_str(target.to_str().unwrap());
        suffix
    } else {
        String::new()
    };

    match fs::metadata(path) {
        Err(e) => {
            eprintln!(
                "{}{} - {}",
                prefix,
                get_file_name(path.to_str().unwrap()),
                e
            );
            process::exit(1);
        }
        _ => println!(
            "{}{}{}",
            prefix,
            get_file_name(path.to_str().unwrap()),
            symlinksuffix
        ),
    }

    if !output_opts.nosymlinks && path.is_symlink() && level < MAXSYMLINKS - 1 {
        let target_pathbuf = fs::read_link(path).unwrap();
        if target_pathbuf.is_relative() {
            let target_pathrel = Path::new(target_pathbuf.to_str().unwrap());
            let symlink_dir = path.parent().unwrap();
            let joindir = symlink_dir.join(target_pathrel);
            let target_path = joindir.as_path();
            print_files(
                level + 1,
                target_path,
                output_opts,
                maximum_owner_length,
                maximum_group_length,
            );
        } else {
            let osstr = fs::read_link(path).unwrap().into_os_string();
            print_files(
                level + 1,
                Path::new(&osstr),
                output_opts,
                maximum_owner_length,
                maximum_group_length,
            );
        }
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let pathlist = matches.get_many::<String>(options::PATHNAMES);

    let output_opts = OutputOptions {
        long: matches.get_flag(options::LONG),
        modes: matches.get_flag(options::MODES) || matches.get_flag(options::LONG),
        nosymlinks: matches.get_flag(options::NOSYMLINKS),
        owners: matches.get_flag(options::OWNERS) || matches.get_flag(options::LONG),
        vertical: matches.get_flag(options::VERTICAL) || matches.get_flag(options::LONG),
        mountpoints: matches.get_flag(options::MOUNTPOINTS),

        #[cfg(feature = "selinux")]
        context: matches.get_flag(options::CONTEXT),
    };

    if let Some(paths) = pathlist {
        for path_str in paths {
            let path = Path::new(path_str);
            println!("f: {}", path.to_str().unwrap());
            let maximum_owner_length = if output_opts.owners {
                max_owner_length(path)
            } else {
                0
            };
            let maximum_group_length = if output_opts.owners {
                max_group_length(path)
            } else {
                0
            };
            print_files(
                0,
                path,
                &output_opts,
                maximum_owner_length,
                maximum_group_length,
            );
        }
    }
    // Handling the case where path is not provided is not necessary
    // because in path arguments have been made necessary in clap so
    // it will automatically show an error in stdout

    Ok(())
}
