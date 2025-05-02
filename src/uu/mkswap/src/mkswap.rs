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
        io::{BufRead, BufReader, Seek, SeekFrom, Write},
        os::{fd::AsRawFd, linux::fs::MetadataExt, unix::fs::FileTypeExt},
        path::Path,
        str::FromStr,
    };

    use crate::*;

    use uucore::error::{set_exit_code, USimpleError, UUsageError};
    use uucore::libc::{ioctl, lseek, read, sysconf, _IO, _SC_PAGESIZE, _SC_PAGE_SIZE};

    use clap::ArgMatches;
    use uuid::Uuid;

    const SWAP_SIGNATURE: &[u8] = "SWAPSPACE2".as_bytes();
    const SWAP_SIGNATURE_SZ: usize = 10;
    const SWAP_VERSION: u8 = 1;
    const MIN_SWAP_PAGES: u128 = 10;

    const BLKGETSIZE: u64 = _IO(0x12, 96) as u64;

    #[repr(C)]
    struct SwapHeader {
        bootbits: [u8; 1024],
        version: u8,
        last_page: u32,
        nr_badpages: u32,
        uuid: [u8; 16],
        volume_name: [u8; 16],
        padding: [u32; 117],
        badpages: [u32; 100],
    }

    fn getsize(fd: &File, stat: &Metadata, devname: &str) -> Result<u128, std::io::Error> {
        let devsize: u128;
        /* for block devices, ioctl call with manual size reading as a backup method */
        if stat.file_type().is_block_device() {
            let mut sectors: u128 = 0;
            let err = unsafe { ioctl(fd.as_raw_fd(), BLKGETSIZE, &mut sectors) };

            if sectors == 0 || err < 0 {
                let f_size = fs::File::open(format!("/sys/class/block/{}/size", devname))?;

                let reader = BufReader::new(f_size);
                let vec: Vec<Result<u128, _>> = reader
                    .lines()
                    .map(|v| v.unwrap().parse::<u128>())
                    .collect::<Vec<Result<u128, _>>>();
                sectors = vec[0].clone().unwrap_or(0);
            }
            devsize = sectors * 512;
        } else {
            devsize = stat.st_size() as u128;
        }

        Ok(devsize)
    }

    unsafe fn check_blocks(
        file: &mut File,
        pagesize: usize,
        pages: u128,
        verbose: bool,
    ) -> Result<Vec<u32>, std::io::Error> {
        let mut bad_pages: Vec<u32> = Vec::new();
        let mut buffer = vec![0u8; pagesize];
        let end = pagesize as u64 * pages as u64;

        let fd = file.as_raw_fd();
        let mut bytes: uucore::libc::ssize_t;

        for current_page in 0..pages {
            let offset = current_page as u64 * pagesize as u64;
            if offset > end {
                break;
            }

            unsafe {
                if lseek(fd, offset as i64, uucore::libc::SEEK_SET) < 0 {
                    panic!("Failed to seek");
                }

                bytes = read(fd, buffer.as_mut_ptr() as *mut std::ffi::c_void, pagesize);
                if bytes < 0 || bytes != pagesize as isize {
                    bad_pages.push(current_page as u32);
                }
            }
            if bad_pages.len() >= 640 {
                panic!("Too many bad pages detected: {}", bad_pages.len());
            }
        }
        if verbose {
            println!("{} bad pages", bad_pages.len())
        }
        file.seek(SeekFrom::Start(0))?;

        Ok(bad_pages)
    }

    unsafe fn init_signature_page(
        pagesize: usize,
        pages: u128,
        uuid: Uuid,
        label: &str,
        badpages: &mut Vec<u32>,
    ) -> Box<[u8]> {
        let mut buf = Box::<[u8]>::new_uninit_slice(pagesize);

        unsafe {
            buf.as_mut_ptr().write_bytes(0, pagesize);

            //fill up swap header
            let swap_hdr = buf.as_mut_ptr() as *mut SwapHeader;
            (*swap_hdr).version = SWAP_VERSION;
            (*swap_hdr).last_page = (pages - 1) as u32;
            (*swap_hdr).nr_badpages = badpages.len() as u32;
            (*swap_hdr).badpages[..badpages.len()].copy_from_slice(badpages.as_mut_slice());
            if !uuid.is_nil() {
                (*swap_hdr).uuid = *uuid.as_bytes();
            }

            if !label.is_empty() {
                let lb = label.as_bytes();
                let lblen = lb.len().min((*swap_hdr).volume_name.len()); //TODO: verbose mode, informs user of truncation
                (*swap_hdr).volume_name[..lblen].copy_from_slice(&lb[..lblen]);
            }

            buf.assume_init()
        }
    }

    pub fn mkswap(args: &ArgMatches) -> UResult<()> {
        let verbose = args.get_flag("verbose");
        let checkflag: bool = args.get_flag("check");

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

        let mut fd = match fs::File::options()
            .create(false)
            .write(true)
            .read(true)
            .truncate(false)
            .append(false)
            .open(dev)
        {
            Ok(f) => f,
            Err(e) => {
                return Err(USimpleError::new(
                    1,
                    format!("failed to open {}: {}", device, e),
                ))
            }
        };

        let stat = fd.metadata()?;

        let uuid = match args.get_one::<String>("uuid") {
            Some(str) => Uuid::from_str(str).expect("Unable to parse UUID"), //TODO: more gracious error handling
            None => Uuid::new_v4(),
        };

        if stat.st_uid() != 0 {
            println!(
                "{}: {}: insecure file owner {}, fix with: chown 0:0 {}",
                uucore::util_name(),
                device,
                stat.st_uid(),
                device
            );
        }

        let devname = if let Some(str) = dev.file_name().unwrap().to_str() {
            str
        } else {
            device.strip_prefix("/dev/").unwrap_or(device)
        };

        let stblksize: u64 = stat.st_blksize();
        let pagesize: u128 = if stblksize == 0 {
            let mut sz = unsafe { sysconf(_SC_PAGESIZE) };
            if sz <= 0 {
                sz = unsafe { sysconf(_SC_PAGE_SIZE) };
                if sz <= 0 {
                    return Err(USimpleError::new(1, "Can't determine system pagesize"));
                }
            }
            (sz as u64).into()
        } else {
            stblksize.into()
        };

        let pages: u128 = getsize(&fd, &stat, devname)? / pagesize;

        if pages < MIN_SWAP_PAGES {
            return Err(USimpleError::new(
                1,
                format!(
                    "swap space needs to be at least {}KiB",
                    MIN_SWAP_PAGES * pagesize / 1024
                ),
            ));
        }

        let mut badpages = if checkflag {
            unsafe { check_blocks(&mut fd, pagesize as usize, pages, verbose)? }
        } else {
            vec![0; 100]
        };

        //initialize and write swap header to signature page
        let mut buf =
            unsafe { init_signature_page(pagesize as usize, pages, uuid, label, &mut badpages) };

        //write swap signature
        let _ = &buf[(pagesize as usize - SWAP_SIGNATURE_SZ)..pagesize as usize]
            .copy_from_slice(SWAP_SIGNATURE);

        fd.write_all(&buf)?;
        fd.flush()?;
        fd.sync_all()?;

        if label.is_empty() {
            println!(
                "Setting up swapspace version 1, size = {}KiB\nNo label, UUID={}",
                (((pages - 1) * pagesize as u128) / 1024),
                uuid
            );
        } else {
            println!(
                "Setting up swapspace version 1, size = {}KiB\nLABEL={}, UUID={}",
                (((pages - 1) * pagesize as u128) / 1024),
                &label[..label.len().min(16)], //truncate given too long of a label.
                uuid
            )
        }

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
                .short('d')
                .long("device")
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
            Arg::new("check")
                .long("check")
                .action(ArgAction::SetTrue)
                .help("check the device for bad pages before writing to it"),
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
