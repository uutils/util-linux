// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{CStr, c_uint};
use std::str::FromStr;

use smartcols_sys::{
    SCOLS_FL_RIGHT, SCOLS_FL_TRUNC, SCOLS_FL_WRAP, SCOLS_JSON_ARRAY_STRING, SCOLS_JSON_BOOLEAN,
    SCOLS_JSON_NUMBER, SCOLS_JSON_STRING,
};

use crate::errors::LsLocksError;

#[derive(Debug, Copy, Clone)]
pub(crate) struct ColumnInfo {
    pub(crate) id: &'static CStr,
    pub(crate) width_hint: f64,
    pub(crate) flags: c_uint,
    pub(crate) default_json_type: c_uint,
}

impl ColumnInfo {
    const fn new(
        id: &'static CStr,
        width_hint: f64,
        flags: c_uint,
        default_json_type: c_uint,
    ) -> Self {
        Self {
            id,
            width_hint,
            flags,
            default_json_type,
        }
    }

    pub(crate) fn json_type(&self, in_bytes: bool) -> c_uint {
        if in_bytes && self.id.to_bytes() == b"SIZE" {
            SCOLS_JSON_NUMBER
        } else {
            self.default_json_type
        }
    }
}

pub(crate) static COLUMN_INFOS: [ColumnInfo; 13] = [
    ColumnInfo::new(c"COMMAND", 15.0, 0, SCOLS_JSON_STRING),
    ColumnInfo::new(c"PID", 5.0, SCOLS_FL_RIGHT, SCOLS_JSON_NUMBER),
    ColumnInfo::new(c"TYPE", 5.0, SCOLS_FL_RIGHT, SCOLS_JSON_STRING),
    ColumnInfo::new(c"SIZE", 4.0, SCOLS_FL_RIGHT, SCOLS_JSON_STRING),
    ColumnInfo::new(c"INODE", 5.0, SCOLS_FL_RIGHT, SCOLS_JSON_NUMBER),
    ColumnInfo::new(c"MAJ:MIN", 6.0, 0, SCOLS_JSON_STRING),
    ColumnInfo::new(c"MODE", 5.0, 0, SCOLS_JSON_STRING),
    ColumnInfo::new(c"M", 1.0, 0, SCOLS_JSON_BOOLEAN),
    ColumnInfo::new(c"START", 10.0, SCOLS_FL_RIGHT, SCOLS_JSON_NUMBER),
    ColumnInfo::new(c"END", 10.0, SCOLS_FL_RIGHT, SCOLS_JSON_NUMBER),
    ColumnInfo::new(c"PATH", 0.0, SCOLS_FL_TRUNC, SCOLS_JSON_STRING),
    ColumnInfo::new(c"BLOCKER", 0.0, SCOLS_FL_RIGHT, SCOLS_JSON_NUMBER),
    ColumnInfo::new(c"HOLDERS", 0.0, SCOLS_FL_WRAP, SCOLS_JSON_ARRAY_STRING),
];

pub(crate) static ALL: [&str; 13] = [
    "COMMAND", "PID", "TYPE", "SIZE", "INODE", "MAJ:MIN", "MODE", "M", "START", "END", "PATH",
    "BLOCKER", "HOLDERS",
];

pub(crate) static DEFAULT: [&str; 9] = [
    "COMMAND", "PID", "TYPE", "SIZE", "MODE", "M", "START", "END", "PATH",
];

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
    type Err = LsLocksError;

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
                    .ok_or_else(|| LsLocksError::InvalidColumnName(name.into()))
            })
            .collect::<Result<_, _>>()?;

        if list.is_empty() {
            Err(LsLocksError::InvalidColumnSequence(s.into()))
        } else {
            Ok(Self { append, list })
        }
    }
}

impl From<&'_ clap::ArgMatches> for OutputColumns {
    fn from(args: &clap::ArgMatches) -> Self {
        args.get_one::<Self>(crate::options::OUTPUT)
            .map_or_else(Self::default, |columns| Self {
                append: columns.append,
                list: columns.list.clone(),
            })
    }
}
