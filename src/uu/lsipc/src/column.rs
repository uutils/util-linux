// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, c_uint};
use std::str::FromStr;

use smartcols_sys::{SCOLS_FL_NOEXTREMES, SCOLS_FL_RIGHT, SCOLS_FL_TRUNC};

use crate::errors::LsIpcError;

#[derive(Debug, Copy, Clone)]
pub(crate) struct ColumnInfo {
    pub(crate) id: &'static CStr,
    pub(crate) title: &'static str,
    pub(crate) width_hint: f64,
    pub(crate) flags: c_uint,
}

impl ColumnInfo {
    const fn new(id: &'static CStr, title: &'static str, flags: c_uint) -> Self {
        Self {
            id,
            title,
            width_hint: 1.0,
            flags,
        }
    }
}

pub(crate) static COLUMN_INFOS: [ColumnInfo; 34] = [
    // Generic
    ColumnInfo::new(c"KEY", "Key", 0),
    ColumnInfo::new(c"ID", "ID", 0),
    ColumnInfo::new(c"OWNER", "Owner", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"PERMS", "Permissions", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"CUID", "Creator UID", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"CUSER", "Creator user", 0),
    ColumnInfo::new(c"CGID", "Creator GID", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"CGROUP", "Creator group", 0),
    ColumnInfo::new(c"UID", "UID", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"USER", "User name", 0),
    ColumnInfo::new(c"GID", "GID", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"GROUP", "Group name", 0),
    ColumnInfo::new(c"CTIME", "Last change", SCOLS_FL_RIGHT),
    // Message queues
    ColumnInfo::new(c"USEDBYTES", "Bytes used", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"MSGS", "Messages", 0),
    ColumnInfo::new(c"SEND", "Msg sent", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"RECV", "Msg received", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"LSPID", "Msg sender", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"LRPID", "Msg receiver", SCOLS_FL_RIGHT),
    // Shared memory
    ColumnInfo::new(c"SIZE", "Segment size", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"NATTCH", "Attached processes", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"STATUS", "Status", SCOLS_FL_NOEXTREMES),
    ColumnInfo::new(c"ATTACH", "Attach time", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"DETACH", "Detach time", SCOLS_FL_RIGHT),
    ColumnInfo {
        id: c"COMMAND",
        title: "Creator command",
        width_hint: 0.0,
        flags: SCOLS_FL_TRUNC,
    },
    ColumnInfo::new(c"CPID", "Creator PID", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"LPID", "Last user PID", SCOLS_FL_RIGHT),
    // Semaphores
    ColumnInfo::new(c"NSEMS", "Semaphores", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"OTIME", "Last operation", SCOLS_FL_RIGHT),
    // Summary
    ColumnInfo::new(c"RESOURCE", "Resource", 0),
    ColumnInfo::new(c"DESCRIPTION", "Description", 0),
    ColumnInfo::new(c"USED", "Used", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"USE%", "Use", SCOLS_FL_RIGHT),
    ColumnInfo::new(c"LIMIT", "Limit", SCOLS_FL_RIGHT),
];

mod all {
    pub(crate) static GENERIC: [&str; 13] = [
        "KEY", "ID", "OWNER", "PERMS", "CUID", "CUSER", "CGID", "CGROUP", "UID", "USER", "GID",
        "GROUP", "CTIME",
    ];

    pub(crate) static QUEUES: [&str; 6] = ["USEDBYTES", "MSGS", "SEND", "RECV", "LSPID", "LRPID"];

    pub(crate) static SHARED_MEMORY: [&str; 8] = [
        "SIZE", "NATTCH", "STATUS", "ATTACH", "DETACH", "COMMAND", "CPID", "LPID",
    ];

    pub(crate) static SEMAPHORES: [&str; 2] = ["NSEMS", "OTIME"];

    pub(crate) static SUMMARY: [&str; 5] = ["RESOURCE", "DESCRIPTION", "USED", "USE%", "LIMIT"];
}

mod default {
    pub(crate) static QUEUES: [&str; 8] = [
        "KEY",
        "ID",
        "PERMS",
        "OWNER",
        "USEDBYTES",
        "MSGS",
        "LSPID",
        "LRPID",
    ];

    pub(crate) static SHARED_MEMORY: [&str; 11] = [
        "KEY", "ID", "PERMS", "OWNER", "SIZE", "NATTCH", "STATUS", "CTIME", "CPID", "LPID",
        "COMMAND",
    ];

    pub(crate) static SEMAPHORES: [&str; 5] = ["KEY", "ID", "PERMS", "OWNER", "NSEMS"];

    pub(crate) static GLOBAL: [&str; 5] = ["RESOURCE", "DESCRIPTION", "LIMIT", "USED", "USE%"];

    pub(crate) static CREATOR: [&str; 4] = ["CUID", "CGID", "UID", "GID"];
}

#[derive(Debug, Clone)]
pub(crate) struct OutputColumns {
    pub(crate) append: bool,
    pub(crate) list: Vec<&'static ColumnInfo>,
}

impl Default for OutputColumns {
    fn default() -> Self {
        Self {
            append: true,
            list: Vec::default(),
        }
    }
}

impl FromStr for OutputColumns {
    type Err = LsIpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let suffix = s.strip_prefix('+');
        let append = suffix.is_some();

        let list: Vec<_> = suffix
            .unwrap_or(s)
            .split(',')
            .map(|name| {
                COLUMN_INFOS
                    .iter()
                    .find(|&column| column.id.to_str().unwrap() == name)
                    .ok_or_else(|| LsIpcError::InvalidColumnName(name.into()))
            })
            .collect::<Result<_, _>>()?;

        if list.is_empty() {
            Err(LsIpcError::InvalidColumnSequence(s.into()))
        } else {
            Ok(Self { append, list })
        }
    }
}

impl From<&'_ clap::ArgMatches> for OutputColumns {
    fn from(args: &'_ clap::ArgMatches) -> Self {
        let Some(columns) = args.get_one::<Self>(crate::options::OUTPUT) else {
            return Self::default();
        };

        let allowed_column_names = if args.get_flag(crate::options::GLOBAL) {
            &all::SUMMARY[..]
        } else if args.get_flag(crate::options::QUEUES) {
            &all::QUEUES[..]
        } else if args.get_flag(crate::options::SHMEMS) {
            &all::SHARED_MEMORY[..]
        } else if args.get_flag(crate::options::SEMAPHORES) {
            &all::SEMAPHORES[..]
        } else {
            unreachable!()
        };

        let (list, not_applicable): (Vec<&ColumnInfo>, Vec<&ColumnInfo>) =
            columns.list.iter().partition(|&&column| {
                let id = column.id.to_str().unwrap();
                all::GENERIC.contains(&id) || allowed_column_names.contains(&id)
            });

        if !not_applicable.is_empty() {
            let join = move |mut buffer: String, column: &ColumnInfo| {
                if !buffer.is_empty() {
                    buffer.push(',');
                }
                buffer.push_str(column.id.to_str().unwrap());
                buffer
            };

            let not_applicable = not_applicable.into_iter().fold(String::default(), join);
            eprintln!("The following columns do not apply to the specified IPC: {not_applicable}.");
        }

        Self {
            append: columns.append,
            list,
        }
    }
}

pub(crate) fn all_defaults(
    args: &clap::ArgMatches,
) -> Result<Vec<&'static ColumnInfo>, LsIpcError> {
    let mut iter: Box<dyn Iterator<Item = &str>> = Box::new(all::GENERIC.into_iter());

    if args.get_flag(crate::options::QUEUES) {
        iter = Box::new(iter.chain(all::QUEUES));
    }

    if args.get_flag(crate::options::SHMEMS) {
        iter = Box::new(iter.chain(all::SHARED_MEMORY));
    }

    if args.get_flag(crate::options::SEMAPHORES) {
        iter = Box::new(iter.chain(all::SEMAPHORES));
    }

    iter.map(|name| {
        COLUMN_INFOS
            .iter()
            .find(|&column| column.id.to_str().unwrap() == name)
            .ok_or_else(|| LsIpcError::InvalidColumnName(name.into()))
    })
    .collect::<Result<_, _>>()
}

pub(crate) fn filter_defaults(
    args: &clap::ArgMatches,
) -> Result<Vec<&'static ColumnInfo>, LsIpcError> {
    let mut columns = Vec::default();

    if args.get_flag(crate::options::QUEUES) {
        columns.extend(default::QUEUES);
    }

    if args.get_flag(crate::options::SHMEMS) {
        columns.extend(default::SHARED_MEMORY);
    }

    if args.get_flag(crate::options::SEMAPHORES) {
        columns.extend(default::SEMAPHORES);
    }

    if args.get_flag(crate::options::GLOBAL) {
        columns.extend(default::GLOBAL);
    }

    if args.get_flag(crate::options::CREATOR) {
        columns.extend(default::CREATOR);
    }

    if args.get_flag(crate::options::TIME) {
        if args.get_flag(crate::options::QUEUES)
            || (!args.get_flag(crate::options::SHMEMS)
                && !args.get_flag(crate::options::SEMAPHORES))
        {
            columns.extend(["SEND", "RECV", "CTIME"])
        }

        if args.get_flag(crate::options::SHMEMS)
            || (!args.get_flag(crate::options::QUEUES)
                && !args.get_flag(crate::options::SEMAPHORES))
        {
            // If "COMMAND" was the last column, then keep it last.
            let reappend_command = match columns.pop() {
                None => false,
                Some("COMMAND") => true,
                Some(last) => {
                    columns.push(last);
                    false
                }
            };

            columns.extend(["ATTACH", "DETACH"]);

            if reappend_command {
                columns.push("COMMAND");
            }
        }

        if args.get_flag(crate::options::SEMAPHORES)
            || (!args.get_flag(crate::options::QUEUES) && !args.get_flag(crate::options::SHMEMS))
        {
            columns.extend(["OTIME", "CTIME"])
        }
    }

    columns
        .into_iter()
        .map(|name| {
            COLUMN_INFOS
                .iter()
                .find(|&column| column.id.to_str().unwrap() == name)
                .ok_or_else(|| LsIpcError::InvalidColumnName(name.into()))
        })
        .collect::<Result<_, _>>()
}
