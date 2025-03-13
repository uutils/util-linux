// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::borrow::Cow;
use std::ffi::{CStr, CString, c_uint};
use std::io::{Cursor, Write};
use std::ptr::NonNull;
use std::sync::atomic::AtomicPtr;
use std::{io, ptr};

use crate::column::ColumnInfo;
use crate::errors::LsIpcError;
use crate::smartcols::{IterDirection, Table, TableOperations, TableRef};
use crate::utils::{UserDbRecordRef, find_replace_in_vec, local_time};
use crate::{OutputMode, options};

fn decimal_point() -> &'static str {
    use std::sync::atomic::Ordering;

    static DEFAULT: &CStr = c".";
    static VALUE: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

    let mut decimal_point = VALUE.load(Ordering::Acquire);

    if decimal_point.is_null() {
        decimal_point = unsafe { libc::localeconv().as_ref() }
            .and_then(|lc| (!lc.decimal_point.is_null()).then_some(lc.decimal_point))
            .unwrap_or(DEFAULT.as_ptr().cast_mut())
            .cast();

        match VALUE.compare_exchange(
            ptr::null_mut(),
            decimal_point,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_previous_value) => {}
            Err(previous_value) => decimal_point = previous_value,
        }
    }

    unsafe { CStr::from_ptr(decimal_point.cast()) }
        .to_str()
        .unwrap()
}

fn ascii_mode(mode: c_uint) -> [u8; 11] {
    use libc::{
        S_IRGRP, S_IROTH, S_IRUSR, S_ISGID, S_ISUID, S_ISVTX, S_IWGRP, S_IWOTH, S_IWUSR, S_IXGRP,
        S_IXOTH, S_IXUSR,
    };

    let mut buffer = Cursor::new([0_u8; 11]);

    match mode & libc::S_IFMT {
        libc::S_IFDIR => write!(&mut buffer, "d"),
        libc::S_IFLNK => write!(&mut buffer, "l"),
        libc::S_IFCHR => write!(&mut buffer, "c"),
        libc::S_IFBLK => write!(&mut buffer, "b"),
        libc::S_IFSOCK => write!(&mut buffer, "s"),
        libc::S_IFIFO => write!(&mut buffer, "p"),
        libc::S_IFREG => write!(&mut buffer, "-"),
        _ => Ok(()),
    }
    .unwrap();

    for (readable, writable, executable, set_id_flag, set_id, set_id_without_x) in [
        (S_IRUSR, S_IWUSR, S_IXUSR, S_ISUID, 's', 'S'),
        (S_IRGRP, S_IWGRP, S_IXGRP, S_ISGID, 's', 'S'),
        (S_IROTH, S_IWOTH, S_IXOTH, S_ISVTX, 't', 'T'),
    ] {
        let r = if (mode & readable) == 0 { '-' } else { 'r' };
        let w = if (mode & writable) == 0 { '-' } else { 'w' };
        let x = match ((mode & set_id_flag) == 0, (mode & executable) == 0) {
            (true, true) => '-',
            (true, false) => 'x',
            (false, true) => set_id_without_x,
            (false, false) => set_id,
        };
        write!(&mut buffer, "{r}{w}{x}").unwrap();
    }

    write!(&mut buffer, "\x00").unwrap();
    buffer.into_inner()
}

fn strftime(format: &CStr, detailed_time: &libc::tm, buffer: &mut [u8]) -> Result<(), LsIpcError> {
    buffer.fill(0_u8);

    errno::set_errno(errno::Errno(0));

    let r = unsafe {
        libc::strftime(
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            format.as_ptr(),
            detailed_time,
        )
    };

    if r == 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error().is_some_and(|n| n != 0) {
            return Err(LsIpcError::io0("strftime", err));
        }
    }
    Ok(())
}

pub(crate) fn format_time(
    format: crate::TimeFormat,
    now: &libc::timeval,
    time: libc::time_t,
) -> Result<Option<CString>, LsIpcError> {
    if time == 0 {
        return Ok(None);
    }

    let mut buffer: [u8; 256];

    match format {
        crate::TimeFormat::Short => {
            let detailed_time = local_time(time)?;
            let detailed_now = local_time(now.tv_sec)?;

            if detailed_time.tm_yday == detailed_now.tm_yday
                && detailed_time.tm_year == detailed_now.tm_year
            {
                let result = format!("{:02}:{:02}", detailed_time.tm_hour, detailed_time.tm_min);
                return Ok(Some(CString::new(result).unwrap()));
            } else if detailed_time.tm_year == detailed_now.tm_year {
                buffer = [0_u8; 256];
                strftime(c"%b%d", &detailed_time, &mut buffer)?;
            } else {
                buffer = [0_u8; 256];
                strftime(c"%Y-%b%d", &detailed_time, &mut buffer)?;
            }
        }

        crate::TimeFormat::Full => {
            let detailed_time = local_time(time)?;

            buffer = [0_u8; 256];
            if unsafe { libc::asctime_r(&detailed_time, buffer.as_mut_ptr().cast()) }.is_null() {
                return Err(LsIpcError::last_io0("asctime_r"));
            }

            find_replace_in_vec(b'\n', 0_u8, &mut buffer);
        }

        crate::TimeFormat::Iso => {
            let detailed_time = local_time(time)?;

            let tz_minutes = if detailed_time.tm_isdst < 0 {
                0
            } else {
                detailed_time.tm_gmtoff / 60
            };

            let tz_hours = tz_minutes / 60;
            let tz_minutes = (tz_minutes % 60).abs();

            buffer = [0_u8; 256];
            let mut cursor = Cursor::new(buffer);

            write!(
                &mut cursor,
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{tz_hours:+03}:{tz_minutes:02}",
                detailed_time.tm_year + 1900,
                detailed_time.tm_mon + 1,
                detailed_time.tm_mday,
                detailed_time.tm_hour,
                detailed_time.tm_min,
                detailed_time.tm_sec,
            )
            .map_err(|err| LsIpcError::io0("failed to write data", err))?;

            buffer = cursor.into_inner();
        }
    }

    *buffer.last_mut().unwrap() = 0_u8;
    let result = CStr::from_bytes_until_nul(&buffer).unwrap();
    Ok(Some(CString::from(result)))
}

// returns exponent (2^x=n) in range KiB..EiB (2^10..2^60).
fn bytes_exponent(bytes: u64) -> u64 {
    for shift in (10..=60).step_by(10) {
        if bytes < (1 << shift) {
            return shift - 10;
        }
    }
    60
}

fn size_to_human_string(bytes: u64) -> String {
    static LETTERS: [char; 7] = ['B', 'K', 'M', 'G', 'T', 'P', 'E'];

    let exp = bytes_exponent(bytes);
    let unit = LETTERS[if exp == 0 { 0 } else { (exp / 10) as usize }];
    let mut decimal = if exp == 0 { bytes } else { bytes / (1 << exp) };
    let mut fractional = if exp == 0 { 0 } else { bytes % (1 << exp) };

    if fractional != 0 {
        fractional = if fractional >= (u64::MAX / 1000) {
            ((fractional / 1024) * 1000) / (1 << (exp - 10))
        } else {
            (fractional * 1000) / (1 << exp)
        };

        fractional = ((fractional + 50) / 100) * 10;

        if fractional == 100 {
            decimal += 1;
            fractional = 0;
        }
    }

    if fractional == 0 {
        format!("{decimal}{unit}")
    } else {
        format!("{decimal}{}{fractional:02}{unit}", decimal_point())
    }
}

fn size_desc(size: u64, in_bytes: bool) -> String {
    if in_bytes {
        size.to_string()
    } else {
        size_to_human_string(size)
    }
}

pub(crate) fn describe_key(key: libc::key_t) -> Option<Cow<'static, CStr>> {
    Some(Cow::Owned(CString::new(format!("{key:#010x}")).unwrap()))
}

pub(crate) fn describe_integer<T: ToString>(n: T) -> Option<Cow<'static, CStr>> {
    Some(Cow::Owned(CString::new(n.to_string()).unwrap()))
}

pub(crate) fn describe_size(size: u64, in_bytes: bool) -> Option<Cow<'static, CStr>> {
    Some(Cow::Owned(CString::new(size_desc(size, in_bytes)).unwrap()))
}

pub(crate) fn describe_owner(users: &mut UserDbRecordRef, uid: libc::uid_t) -> Option<Cow<CStr>> {
    if let Some(name) = users.for_id(uid).name() {
        Some(Cow::Borrowed(name))
    } else {
        Some(Cow::Owned(CString::new(uid.to_string()).unwrap()))
    }
}

pub(crate) fn describe_permissions(
    args: &clap::ArgMatches,
    permissions: c_uint,
) -> Option<Cow<'static, CStr>> {
    if args.get_flag(options::NUMERIC_PERMS) {
        let s = format!("{permissions:04o}");
        Some(Cow::Owned(CString::new(s).unwrap()))
    } else {
        let s = ascii_mode(permissions);
        let s = CStr::from_bytes_until_nul(&s).unwrap();
        Some(Cow::Owned(s.into()))
    }
}

pub(crate) fn new_global_line(
    columns: &[&ColumnInfo],
    table: &mut Table,
    resource: &CStr,
    description: &CStr,
    used: Option<u64>,
    limit: u64,
    in_bytes: bool,
) -> Result<(), LsIpcError> {
    let mut line = table.new_line(None)?;

    for (cell_index, &column) in columns.iter().enumerate() {
        let data_str = match column.id.to_bytes() {
            b"RESOURCE" => Cow::Borrowed(resource),
            b"DESCRIPTION" => Cow::Borrowed(description),

            b"LIMIT" => Cow::Owned(CString::new(size_desc(limit, in_bytes)).unwrap()),

            b"USED" => used.map_or(Cow::Borrowed(c"-"), |used| {
                Cow::Owned(CString::new(size_desc(used, in_bytes)).unwrap())
            }),

            b"USE%" => used.map_or(Cow::Borrowed(c"-"), |used| {
                let percent = (used as f64) / (limit as f64) * 100.0;
                Cow::Owned(CString::new(format!("{percent:2.2}%")).unwrap())
            }),

            _ => continue,
        };

        line.set_data(cell_index, &data_str)?;
    }
    Ok(())
}

pub(crate) fn new_table(
    args: &clap::ArgMatches,
    output_mode: OutputMode,
) -> Result<Table, LsIpcError> {
    let mut table = Table::new()?;

    if args.get_flag(options::NO_HEADINGS) {
        table.enable_headings(false)?;
    }

    if args.get_flag(options::SHELL) {
        table.enable_shell_variable(true)?;
    }

    match output_mode {
        OutputMode::Export => table.enable_export(true)?,
        OutputMode::NewLine => {
            table.set_column_separator(c"\n")?;
            table.enable_export(true)?
        }
        OutputMode::Raw => table.enable_raw(true)?,
        OutputMode::Json => table.enable_json(true)?,
        OutputMode::Pretty => table.enable_headings(false)?,
        OutputMode::None | OutputMode::List => {}
    }

    Ok(table)
}

pub(crate) fn print_pretty_table(table: &Table) -> Result<(), LsIpcError> {
    let line = table.line(0)?;

    for (cell_index, _column) in table.column_iter(IterDirection::Forward)?.enumerate() {
        if let Some(dstr) = line
            .cell(cell_index)?
            .data_as_c_str()
            .filter(|&s| !s.is_empty())
            .map(CStr::to_bytes)
            .and_then(|b| std::str::from_utf8(b).ok())
        {
            let title = crate::column::COLUMN_INFOS[cell_index].title;
            println!("{title}:{}{dstr: <36}", " ".repeat(35 - title.len()));
        }
    }

    if let Some(sub_table) = NonNull::new(line.user_data().cast()).map(TableRef::from) {
        println!("Elements:");
        println!();
        sub_table.print()?;
    }
    Ok(())
}
