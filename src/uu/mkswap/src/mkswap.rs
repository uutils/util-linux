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
        io::Write,
        os::{
            fd::AsRawFd, linux::fs::MetadataExt, raw::c_char, raw::c_uchar, unix::fs::FileTypeExt,
            unix::fs::PermissionsExt,
        },
        path::Path,
        str::FromStr,
    };
    use uucore::error::{set_exit_code, USimpleError};

    use crate::*;

    use clap::ArgMatches;
    use linux_raw_sys::ioctl::BLKGETSIZE64;
    use thiserror::Error;
    use uucore::libc::{ioctl, sysconf, _SC_PAGESIZE, _SC_PAGE_SIZE};
    use uuid::Uuid;

    const SWAP_SIGNATURE: &[u8] = "SWAPSPACE2".as_bytes();
    const SWAP_SIGNATURE_SZ: usize = 10;
    const SWAP_VERSION: u32 = 1;
    const MIN_SWAP_PAGES: u64 = 10;
    const SWAP_LABEL_LENGTH: usize = 16;

    #[derive(Debug, Error)]
    pub enum MkSwapError {
        #[error("Invalid UUID: '{0}'")]
        InvalidUuid(#[from] uuid::Error),

        #[error("Swap space is too small. Minimum size is {0}KiB.")]
        DeviceTooSmall(u64),

        #[error("Failed to determine page size.")]
        PageSizeDetection,

        #[error("Failed to determine size of {0}: {1}")]
        DeviceSizeDetection(String, std::io::Error),

        #[error("I/O Error: {0}")]
        Io(#[from] std::io::Error),
    }

    #[repr(C)]
    struct SwapHeader {
        bootbits: [c_char; 1024],
        version: u32,
        last_page: u32,
        nr_badpages: u32,
        uuid: [c_uchar; 16],
        volume_name: [u8; SWAP_LABEL_LENGTH],
        padding: [u32; 117],
        badpages: [u32; 1],
    }

    fn getsize(fd: &File, stat: &Metadata, devname: &str) -> Result<u64, std::io::Error> {
        let devsize: u64;
        /* for block devices, ioctl call with manual size reading as a backup method */
        if stat.file_type().is_block_device() {
            let mut sz: u64 = 0;
            let err = unsafe { ioctl(fd.as_raw_fd(), BLKGETSIZE64 as u64, &mut sz) };

            if sz == 0 || err < 0 {
                let path = format!("/sys/class/block/{devname}/size");

                let buf = fs::read_to_string(path)?;
                sz = buf
                    .trim()
                    .parse::<u64>()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                devsize = sz * 512;
            } else {
                devsize = sz;
            }
        } else {
            devsize = stat.st_size();
        }

        Ok(devsize)
    }

    unsafe fn write_signature_page(
        pagesize: usize,
        pages: u64,
        uuid: Uuid,
        label: &String,
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

        header.volume_name[..lblen].copy_from_slice(&label_bytes[..lblen]);

        let mut buf = vec![0u8; pagesize];

        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                (&header as *const SwapHeader) as *const u8,
                std::mem::size_of::<SwapHeader>(),
            )
        };

        buf[0..header_bytes.len()].copy_from_slice(header_bytes);

        let signature_offset = pagesize - SWAP_SIGNATURE_SZ;
        buf[signature_offset..].copy_from_slice(SWAP_SIGNATURE);

        buf
    }

    fn open_device(
        device: &str,
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

    struct MkswapOptions<'a> {
        verbose: bool,
        createflag: bool,
        filesize: u64,
        dev: &'a Path,
        devname: &'a str,
        label: String,
        uuid: Uuid,
    }

    fn get_mkswap_opts(args: &ArgMatches) -> Result<MkswapOptions, MkSwapError> {
        let verbose = args.get_flag("verbose");
        let createflag: bool = args.get_flag("file");
        let filesize: u64 = *args.get_one::<u64>("filesize").unwrap_or(&0);

        let device = args
            .get_one::<String>("device")
            .expect("missing required argument device");

        let label = match args.get_one::<String>("label") {
            Some(l) => l.clone(),
            None => String::new(),
        };

        let dev = Path::new(device.as_str());
        let devname = if let Some(str) = dev.file_name().unwrap().to_str() {
            str
        } else {
            device.strip_prefix("/dev/").unwrap_or(device)
        };

        let uuid = match args.get_one::<String>("uuid") {
            Some(str) => Uuid::from_str(str).map_err(MkSwapError::InvalidUuid)?,
            None => Uuid::new_v4(),
        };
        Ok(MkswapOptions {
            verbose,
            createflag,
            filesize,
            dev,
            devname,
            label,
            uuid,
        })
    }

    fn mkswap(opts: MkswapOptions) -> Result<(), MkSwapError> {
        let mut fd = open_device(
            opts.dev.to_str().unwrap_or(opts.devname),
            opts.dev,
            opts.createflag,
            opts.filesize,
        )?;

        let stat = fd.metadata()?;
        if stat.st_uid() != 0 {
            println!(
                "{}: {}: insecure file owner {}, fix with: chown 0:0 {}",
                uucore::util_name(),
                opts.devname,
                stat.st_uid(),
                opts.devname,
            );
        }

        let pagesize: u64 = {
            let mut sz = unsafe { sysconf(_SC_PAGESIZE) };
            if sz <= 0 {
                sz = unsafe { sysconf(_SC_PAGE_SIZE) };
                if sz <= 0 {
                    return Err(MkSwapError::PageSizeDetection);
                }
            }
            sz as u64
        };

        let devsize: u64 = if opts.createflag {
            opts.filesize
        } else {
            getsize(&fd, &stat, opts.devname)
                .map_err(|e| MkSwapError::DeviceSizeDetection(opts.devname.to_string(), e))?
        };

        let pages = devsize / pagesize;

        if pages < MIN_SWAP_PAGES {
            if opts.createflag {
                fs::remove_file(opts.dev)?;
            }
            return Err(MkSwapError::DeviceTooSmall(
                MIN_SWAP_PAGES * pagesize / 1024,
            ));
        }

        let badpages: [u32; 1] = [0; 1]; // Checking not implemented

        eprintln!("pagesize: {pagesize}\npages: {pages}\nuuid: {}\nlabel: {}\nbadpages: {badpages:?}\nverbose: {}\n", opts.uuid, opts.label, opts.verbose);
        // initialize and write swap header information to a buffer
        let buf = unsafe {
            write_signature_page(
                pagesize as usize,
                pages,
                opts.uuid,
                &opts.label,
                badpages,
                opts.verbose,
            )
        };

        fd.write_all(&buf)?;
        fd.flush()?;
        fd.sync_all()?;

        println!(
            "Setting up swapspace version 1, size = {}KiB\n{}{}, UUID={}",
            (((pages - 1) * pagesize) / 1024),
            if opts.label.is_empty() {
                "No label"
            } else {
                "LABEL="
            },
            &opts.label[..opts.label.len().min(16)], //truncate given too long of a label.
            opts.uuid
        );

        Ok(())
    }

    pub fn run(args: impl uucore::Args) -> UResult<()> {
        let matches = uu_app().try_get_matches_from(args)?;
        let opts = get_mkswap_opts(&matches).map_err(|e| USimpleError::new(1, format!("{e}")))?;
        if let Err(e) = mkswap(opts) {
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
                .required(true)
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
