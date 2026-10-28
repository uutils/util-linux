// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};
#[cfg(target_os = "linux")]
use std::fs::OpenOptions;
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
use uucore::error::{UResult, USimpleError};
#[cfg(target_os = "linux")]
use uucore::parser::parse_size;
use uucore::{format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("fallocate.md");
const USAGE: &str = help_usage!("fallocate.md");

mod options {
    pub const COLLAPSE_RANGE: &str = "collapse-range";
    pub const DIG_HOLES: &str = "dig-holes";
    pub const INSERT_RANGE: &str = "insert-range";
    pub const LENGTH: &str = "length";
    pub const KEEP_SIZE: &str = "keep-size";
    pub const OFFSET: &str = "offset";
    pub const PUNCH_HOLE: &str = "punch-hole";
    pub const ZERO_RANGE: &str = "zero-range";
    pub const POSIX: &str = "posix";
    pub const VERBOSE: &str = "verbose";
    pub const FILENAME: &str = "filename";
}

#[cfg(target_os = "linux")]
fn parse_length_or_offset(s: &str) -> Result<u64, String> {
    parse_size::parse_size_u64(s).map_err(|e| format!("failed to parse size: {e}"))
}

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let filename = matches.get_one::<String>(options::FILENAME).unwrap();
    let verbose = matches.get_flag(options::VERBOSE);

    let offset = matches
        .get_one::<String>(options::OFFSET)
        .map(|s| parse_length_or_offset(s))
        .transpose()
        .map_err(|e| USimpleError::new(1, e))?
        .unwrap_or(0);

    let collapse = matches.get_flag(options::COLLAPSE_RANGE);
    let dig_holes = matches.get_flag(options::DIG_HOLES);
    let insert = matches.get_flag(options::INSERT_RANGE);
    let punch = matches.get_flag(options::PUNCH_HOLE);
    let zero = matches.get_flag(options::ZERO_RANGE);
    let posix = matches.get_flag(options::POSIX);
    let keep_size = matches.get_flag(options::KEEP_SIZE);

    // -l is required except for dig-holes mode
    let length = matches
        .get_one::<String>(options::LENGTH)
        .map(|s| parse_length_or_offset(s))
        .transpose()
        .map_err(|e| USimpleError::new(1, e))?;

    if !dig_holes && length.is_none() {
        return Err(USimpleError::new(1, "required length was not specified"));
    }

    // Open the file: create if doing default allocation or posix, otherwise must exist
    let needs_create = !collapse && !dig_holes && !insert && !punch && !zero;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(needs_create)
        .open(filename)
        .map_err(|e| USimpleError::new(1, format!("cannot open {filename}: {e}")))?;

    if dig_holes {
        return dig_holes_in_file(&file, offset, length);
    }

    let length = length.unwrap();

    if posix {
        do_posix_fallocate(&file, offset, length, verbose, filename)
    } else {
        let mut mode: libc::c_int = 0;
        if keep_size || punch {
            mode |= libc::FALLOC_FL_KEEP_SIZE;
        }
        if punch {
            mode |= libc::FALLOC_FL_PUNCH_HOLE;
        }
        if collapse {
            mode |= libc::FALLOC_FL_COLLAPSE_RANGE;
        }
        if zero {
            mode |= libc::FALLOC_FL_ZERO_RANGE;
        }
        if insert {
            mode |= libc::FALLOC_FL_INSERT_RANGE;
        }

        let ret = unsafe { libc::fallocate(file.as_raw_fd(), mode, offset as i64, length as i64) };
        if ret != 0 {
            let err = io::Error::last_os_error();
            return Err(USimpleError::new(1, format!("fallocate failed: {err}")));
        }

        if verbose {
            let human = humanize_bytes(length);
            eprintln!("{filename}: {human} ({length} bytes) allocated.");
        }

        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn do_posix_fallocate(
    file: &std::fs::File,
    offset: u64,
    length: u64,
    verbose: bool,
    filename: &str,
) -> UResult<()> {
    let ret = unsafe { libc::posix_fallocate(file.as_raw_fd(), offset as i64, length as i64) };
    if ret != 0 {
        let err = io::Error::from_raw_os_error(ret);
        return Err(USimpleError::new(1, format!("fallocate failed: {err}")));
    }

    if verbose {
        let human = humanize_bytes(length);
        eprintln!("{filename}: {human} ({length} bytes) allocated.");
    }

    Ok(())
}

/// Dig holes: find zero-filled regions and punch them out to make the file sparse.
/// Uses SEEK_DATA/SEEK_HOLE to find data and hole boundaries, then punches out
/// zero-filled data regions.
#[cfg(target_os = "linux")]
fn dig_holes_in_file(file: &std::fs::File, offset: u64, length: Option<u64>) -> UResult<()> {
    use std::os::unix::fs::MetadataExt;

    let fd = file.as_raw_fd();
    let file_size = file.metadata()?.size();
    let end = match length {
        Some(l) => std::cmp::min(offset + l, file_size),
        None => file_size,
    };

    let mut pos = offset as i64;

    loop {
        // Seek to the next data region
        let data_start = unsafe { libc::lseek(fd, pos, libc::SEEK_DATA) };
        if data_start < 0 {
            // ENXIO means no more data regions - we're done
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENXIO) {
                break;
            }
            return Err(USimpleError::new(1, format!("seek failed: {err}")));
        }
        if data_start as u64 >= end {
            break;
        }

        // Find the end of this data region (start of next hole)
        let hole_start = unsafe { libc::lseek(fd, data_start, libc::SEEK_HOLE) };
        if hole_start < 0 {
            let err = io::Error::last_os_error();
            return Err(USimpleError::new(1, format!("seek failed: {err}")));
        }

        let region_start = data_start as u64;
        let region_end = std::cmp::min(hole_start as u64, end);

        if region_start < region_end {
            // Read the data region and check if it's all zeros
            // Process in chunks to avoid huge allocations
            const CHUNK_SIZE: u64 = 64 * 1024;
            let mut chunk_offset = region_start;
            while chunk_offset < region_end {
                let chunk_len = std::cmp::min(CHUNK_SIZE, region_end - chunk_offset) as usize;
                let mut buf = vec![0u8; chunk_len];

                // pread to avoid seeking
                let n = unsafe {
                    libc::pread(fd, buf.as_mut_ptr().cast(), chunk_len, chunk_offset as i64)
                };
                if n < 0 {
                    let err = io::Error::last_os_error();
                    return Err(USimpleError::new(1, format!("read failed: {err}")));
                }
                let n = n as usize;
                buf.truncate(n);

                if buf.iter().all(|&b| b == 0) && !buf.is_empty() {
                    // This chunk is all zeros - punch a hole
                    let mode = libc::FALLOC_FL_KEEP_SIZE | libc::FALLOC_FL_PUNCH_HOLE;
                    let ret = unsafe { libc::fallocate(fd, mode, chunk_offset as i64, n as i64) };
                    if ret != 0 {
                        let err = io::Error::last_os_error();
                        return Err(USimpleError::new(1, format!("fallocate failed: {err}")));
                    }
                }

                chunk_offset += n as u64;
                if n == 0 {
                    break;
                }
            }
        }

        pos = hole_start;
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn humanize_bytes(bytes: u64) -> String {
    const UNITS: &[(u64, &str)] = &[
        (1 << 60, "EiB"),
        (1 << 50, "PiB"),
        (1 << 40, "TiB"),
        (1 << 30, "GiB"),
        (1 << 20, "MiB"),
        (1 << 10, "KiB"),
    ];

    for &(threshold, unit) in UNITS {
        if bytes >= threshold {
            let value = bytes as f64 / threshold as f64;
            // Match util-linux format: whole numbers without decimal
            if bytes.is_multiple_of(threshold) {
                return format!("{} {}", bytes / threshold, unit);
            }
            return format!("{value:.1} {unit}");
        }
    }
    format!("{bytes} B")
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;
    Err(USimpleError::new(
        1,
        "`fallocate` is available only on Linux.",
    ))
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::COLLAPSE_RANGE)
                .short('c')
                .long("collapse-range")
                .help("remove a range from the file")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::DIG_HOLES)
                .short('d')
                .long("dig-holes")
                .help("detect zeroes and replace with holes")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::INSERT_RANGE)
                .short('i')
                .long("insert-range")
                .help("insert a hole at range, shifting existing data")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::LENGTH)
                .short('l')
                .long("length")
                .help("length for range operations, in bytes")
                .value_name("num")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::KEEP_SIZE)
                .short('n')
                .long("keep-size")
                .help("maintain the apparent size of the file")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::OFFSET)
                .short('o')
                .long("offset")
                .help("offset for range operations, in bytes")
                .value_name("num")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::PUNCH_HOLE)
                .short('p')
                .long("punch-hole")
                .help("replace a range with a hole (implies -n)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::ZERO_RANGE)
                .short('z')
                .long("zero-range")
                .help("zero and ensure allocation of a range")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::POSIX)
                .short('x')
                .long("posix")
                .help("use posix_fallocate(3) instead of fallocate(2)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::VERBOSE)
                .short('v')
                .long("verbose")
                .help("verbose mode")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::FILENAME)
                .value_name("filename")
                .help("target file")
                .required(true)
                .index(1)
                .action(ArgAction::Set),
        )
        .group(
            ArgGroup::new("mode")
                .args([
                    options::COLLAPSE_RANGE,
                    options::DIG_HOLES,
                    options::INSERT_RANGE,
                    options::PUNCH_HOLE,
                    options::ZERO_RANGE,
                    options::POSIX,
                ])
                .multiple(false),
        )
}
