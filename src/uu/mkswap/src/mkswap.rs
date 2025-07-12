// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};

use uucore::error::UResult;
use uucore::{format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("mkswap.md");
const USAGE: &str = help_usage!("mkswap.md");

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        fs::{self, File, Metadata},
        io::{BufRead, BufReader, Write},
        os::{
            fd::AsRawFd, linux::fs::MetadataExt, raw::c_char, raw::c_uchar, unix::fs::FileTypeExt,
            unix::fs::PermissionsExt,
        },
        path::Path,
        str::FromStr,
    };
    use uucore::error::{set_exit_code, USimpleError, UUsageError};

    use crate::*;

    use clap::ArgMatches;
    use linux_raw_sys::ioctl::BLKGETSIZE64;
    use uucore::libc::{ioctl, sysconf, _SC_PAGESIZE, _SC_PAGE_SIZE};
    use uuid::Uuid;

    const SWAP_SIGNATURE: &[u8] = "SWAPSPACE2".as_bytes();
    const SWAP_SIGNATURE_SZ: usize = 10;
    const SWAP_VERSION: u32 = 1;
    const MIN_SWAP_PAGES: u128 = 10;
    const SWAP_LABEL_LENGTH: usize = 16;

    #[repr(C)]
    struct SwapHeader {
        bootbits: [c_char; 1024],
        version: u32,
        last_page: u32,
        nr_badpages: u32,
        uuid: [c_uchar; 16],
        volume_name: [c_char; SWAP_LABEL_LENGTH],
        padding: [u32; 117],
        badpages: [u32; 1],
    }

    fn getsize(fd: &File, stat: &Metadata, devname: &str) -> Result<u128, std::io::Error> {
        let devsize: u128;
        /* for block devices, ioctl call with manual size reading as a backup method */
        if stat.file_type().is_block_device() {
            let mut sz: u128 = 0;
            let err = unsafe { ioctl(fd.as_raw_fd(), BLKGETSIZE64 as u64, &mut sz) };

            if sz == 0 || err < 0 {
                let f_size = fs::File::open(format!("/sys/class/block/{devname}/size"))?;

                let reader = BufReader::new(f_size);
                let vec: Vec<Result<u128, _>> = reader
                    .lines()
                    .map(|v| v.unwrap().parse::<u128>())
                    .collect::<Vec<Result<u128, _>>>();
                sz = vec[0].clone().unwrap_or(0);
                devsize = sz * 512;
            } else {
                devsize = sz;
            }
        } else {
            devsize = stat.st_size() as u128;
        }

        Ok(devsize)
    }

    unsafe fn write_signature_page(
        pagesize: usize,
        pages: u128,
        uuid: Uuid,
        label: &str,
        badpages: [u32; 1],
        verbose: bool,
    ) -> Vec<u8> {
        let mut header = SwapHeader {
            bootbits: [0; 1024],
            version: SWAP_VERSION,
            last_page: (pages - 1) as u32,
            nr_badpages: 0, // Assumes no bad pages
            uuid: *uuid.as_bytes(),
            volume_name: [0; SWAP_LABEL_LENGTH],
            padding: [0; 117],
            badpages,
        };

        let label_bytes = label.as_bytes();
        let lblen = label_bytes.len().min(SWAP_LABEL_LENGTH);
        if label.len() > SWAP_LABEL_LENGTH && verbose {
            eprintln!("swap label was truncated");
        }

        let label_buf = unsafe {
            std::slice::from_raw_parts_mut(
                header.volume_name.as_mut_ptr() as *mut u8,
                SWAP_LABEL_LENGTH,
            )
        };
        label_buf[..lblen].copy_from_slice(&label_bytes[..lblen]);

        let mut buf = vec![0u8; pagesize];

        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                (&header as *const SwapHeader) as *const u8,
                std::mem::size_of::<SwapHeader>(),
            )
        };

        buf[0..header_bytes.len()].copy_from_slice(header_bytes);

        let signature_offset = pagesize - SWAP_SIGNATURE.len();
        buf[signature_offset..].copy_from_slice(SWAP_SIGNATURE);

        buf
    }

    fn open_device(
        device: &String,
        dev: &Path,
        createflag: bool,
        filesize: u64,
    ) -> Result<File, std::io::Error> {
        let mut options = fs::OpenOptions::new();
        let fd = match options
            .create(false)
            .create_new(createflag)
            .write(true)
            .read(true)
            .truncate(false)
            .append(false)
            .open(dev)
        {
            Ok(f) => f,
            Err(e) => {
                return Err(std::io::Error::other(format!(
                    "failed to open {device}: {e}"
                )))
            }
        };

        if createflag {
            fd.set_permissions(fs::Permissions::from_mode(0o600))?;
            fd.set_len(filesize)?;
        }

        Ok(fd)
    }

    pub fn mkswap(args: &ArgMatches) -> UResult<()> {
        let verbose = args.get_flag("verbose");
        let createflag: bool = args.get_flag("file");
        let filesize: u64 = *args.get_one::<u64>("filesize").unwrap_or(&0);

        let device = match args.get_one::<String>("device") {
            Some(str) => str,
            None => {
                return Err(UUsageError::new(
                    1,
                    format!(
                        "Usage: {}\nFor more information, try '--help'.",
                        format_usage(USAGE)
                    ),
                ))
            }
        };

        let label = match args.get_one::<String>("label") {
            Some(l) => l.as_str(),
            None => "",
        };

        let dev = Path::new(device.as_str());
        let devname = if let Some(str) = dev.file_name().unwrap().to_str() {
            str
        } else {
            device.strip_prefix("/dev/").unwrap_or(device)
        };

        let uuid = match args.get_one::<String>("uuid") {
            Some(str) => Uuid::from_str(str)
                .map_err(|e| USimpleError::new(1, format!("Invalid UUID '{str}': {e}")))?, //TODO: more gracious error handling
            None => Uuid::new_v4(),
        };

        let mut fd = open_device(device, dev, createflag, filesize)?;

        let stat = fd.metadata()?;
        if stat.st_uid() != 0 {
            println!(
                "{}: {}: insecure file owner {}, fix with: chown 0:0 {}",
                uucore::util_name(),
                device,
                stat.st_uid(),
                device,
            );
        }

        let pagesize: u128 = {
            let mut sz = unsafe { sysconf(_SC_PAGESIZE) };
            if sz <= 0 {
                sz = unsafe { sysconf(_SC_PAGE_SIZE) };
                if sz <= 0 {
                    return Err(USimpleError::new(1, "Can't determine system pagesize"));
                }
            }
            (sz as u64).into()
        };

        let devsize: u128 = if createflag {
            filesize as u128
        } else {
            getsize(&fd, &stat, devname).map_err(|e| {
                USimpleError::new(1, format!("failed to determine size of {devname}: {e}"))
            })?
        };

        let pages: u128 = devsize / pagesize;

        if pages < MIN_SWAP_PAGES {
            if createflag {
                fs::remove_file(dev)?;
            }
            return Err(USimpleError::new(
                1,
                format!(
                    "swap space needs to be at least {}KiB",
                    MIN_SWAP_PAGES * pagesize / 1024
                ),
            ));
        }

        let badpages: [u32; 1] = [0; 1]; // Checking not implemented

        eprintln!("pagesize: {pagesize}\npages: {pages}\nuuid: {uuid}\nlabel: {label}\nbadpages: {badpages:?}\nverbose: {verbose}\n");
        // initialize and write swap header information to a buffer
        let mut buf = unsafe {
            write_signature_page(pagesize as usize, pages, uuid, label, badpages, verbose)
        };

        //write swap signature to buffer
        let _ = &buf[(pagesize as usize - SWAP_SIGNATURE_SZ)..pagesize as usize]
            .copy_from_slice(SWAP_SIGNATURE);

        fd.write_all(&buf)?;
        fd.flush()?;
        fd.sync_all()?;

        println!(
            "Setting up swapspace version 1, size = {}KiB\n{}{}, UUID={}",
            (((pages - 1) * pagesize as u128) / 1024),
            if label.is_empty() {
                "No label"
            } else {
                "LABEL="
            },
            &label[..label.len().min(16)], //truncate given too long of a label.
            uuid
        );

        Ok(())
    }

    pub fn run(args: impl uucore::Args) -> UResult<()> {
        let matches = uu_app().try_get_matches_from(args)?;
        if let Err(e) = mkswap(&matches) {
            set_exit_code(2);
            uucore::show_error!("{}", e);
        };
        Ok(())
    }
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    platform::run(args)
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("device")
                .action(ArgAction::Set)
                .help("block device or swap file"),
        )
        .arg(
            Arg::new("label")
                .short('l')
                .long("label")
                .action(ArgAction::Set)
                .help("set a label"),
        )
        .arg(
            Arg::new("uuid")
                .short('u')
                .long("uuid")
                .action(ArgAction::Set)
                .help("set the UUID to use"),
        )
        .arg(
            Arg::new("file")
                .short('F')
                .long("file")
                .action(ArgAction::SetTrue)
                .help("create a swap file"),
        )
        .arg(
            Arg::new("filesize")
                .short('s')
                .long("size")
                .action(ArgAction::Set)
                .value_parser(clap::value_parser!(u64))
                .value_name("SIZE")
                .help("size of the swap file in bytes"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("verbose output"),
        )
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use crate::uu_app;
    use uucore::error::UResult;
    pub fn run(_args: impl uucore::Args) -> UResult<()> {
        let _matches: clap::ArgMatches = uu_app().try_get_matches_from(_args)?;

        Err(uucore::error::USimpleError::new(
            1,
            "`mkswap` is available only on Linux.",
        ))
    }
}
