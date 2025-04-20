// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgMatches, Command};

use uucore::{
    error::{set_exit_code, UResult, USimpleError, UUsageError},
    format_usage, help_about, help_usage,
};

const ABOUT: &str = help_about!("mkswap.md");
const USAGE: &str = help_usage!("mkswap.md");

#[cfg(target_os = "linux")]
mod platform {

    use std::{
        fs::{self, File, Metadata},
        io::{BufRead, BufReader, Write},
        path::Path,
        str::FromStr,
    };

    use crate::*;
    use std::os::{fd::AsRawFd, linux::fs::MetadataExt};
    use uucore::libc::{ioctl, sysconf, _IO, _SC_PAGESIZE, _SC_PAGE_SIZE};
    use uuid::Uuid;

    const SWAP_SIGNATURE: &[u8] = "SWAPSPACE2".as_bytes();
    const SWAP_SIGNATURE_SZ: usize = 10;
    const SWAP_VERSION: u8 = 1;

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
        badpages: [u32; 1],
    }

    fn getsize(fd: &File, stat: &Metadata, devname: &str) -> Result<u128, std::io::Error> {
        let devsize: u128;
        /* for block devices, ioctl call with manual size reading as a backup method */
        if stat.st_mode() == 25008 {
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

    unsafe fn init_signature_page(
        pagesize: usize,
        pages: u128,
        uuid: Uuid,
        label: &str,
    ) -> Box<[u8]> {
        let mut buf = Box::<[u8]>::new_uninit_slice(pagesize);

        unsafe {
            buf.as_mut_ptr().write_bytes(0, pagesize);

            //fill up swap header
            let swap_hdr = buf.as_mut_ptr() as *mut SwapHeader;
            (*swap_hdr).version = SWAP_VERSION;
            (*swap_hdr).last_page = (pages - 1) as u32;
            if !uuid.is_nil() {
                (*swap_hdr).uuid = *uuid.as_bytes();
            }
            if !label.is_empty() {
                (*swap_hdr)
                    .volume_name
                    .as_mut_ptr()
                    .copy_from(label.as_ptr(), label.len());
            }
            buf.assume_init()
        }
    }

    pub fn mkswap(args: &ArgMatches) -> UResult<()> {
        let devstr;

        match args.get_one::<String>("device") {
            Some(str) => devstr = str,
            None => {
                return Err(UUsageError::new(
                    1,
                    format!("Usage: {} -d device", uucore::util_name()),
                ))
            }
        }

        let label = match args.get_one::<String>("label") {
            Some(l) => l.as_str(),
            None => "",
        };

        let dev = Path::new(devstr.as_str());
        let devname = devstr.strip_prefix("/dev/").unwrap_or("err");

        let mut fd = fs::File::options()
            .create(false)
            .write(true)
            .truncate(false)
            .append(false)
            .open(dev)?;

        let stat = fd.metadata()?;

        let uuid = match args.get_one::<String>("uuid") {
            Some(str) => Uuid::from_str(str).expect("Unable to parse UUID"),
            None => Uuid::new_v4(),
        };

        if stat.st_uid() != 0 {
            println!(
                "{}: {}: insecure file owner {}, fix with: chown 0:0 {}",
                uucore::util_name(),
                devstr,
                stat.st_uid(),
                devstr
            );
        }

        let devsize: u128 = getsize(&fd, &stat, devname)?;

        let mut pagesize: i64 = stat.st_blksize() as i64;
        if pagesize <= 0 {
            pagesize = unsafe { sysconf(_SC_PAGE_SIZE) };
            if pagesize <= 0 {
                pagesize = unsafe { sysconf(_SC_PAGESIZE) };
                if pagesize <= 0 {
                    return Err(USimpleError::new(1, "Can't determine system pagesize"));
                }
            }
        }

        assert!(pagesize > 0);
        if devsize < (10 * pagesize as u128) {
            return Err(USimpleError::new(
                1,
                format!(
                    "swap space needs to be at least {}KiB",
                    (10 * pagesize / 1024)
                ),
            ));
        }
        assert!(devsize > 0);

        let pages: u128 = devsize / pagesize as u128;

        //signature page
        let mut buf = unsafe { init_signature_page(pagesize as usize, pages, uuid, label) };

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
                "Setting up swapspace version 1, size = {}KiB\nLabel={}, UUID={}",
                (((pages - 1) * pagesize as u128) / 1024),
                label,
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
#[cfg(target_os = "linux")]
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
                .help("block device"),
        )
        .arg(
            Arg::new("label")
                .short('l')
                .long("label")
                .action(ArgAction::Set)
                .help("specify a label"),
        )
        .arg(
            Arg::new("uuid")
                .short('u')
                .long("uuid")
                .action(ArgAction::Set)
                .help("set a uuid"),
        )
}

#[cfg(not(target_os = "linux"))]
mod platform {
    #[uucore::main]
    pub fn run(_args: impl uucore::Args) -> UResult<()> {
        let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

        Err(uucore::error::USimpleError::new(
            1,
            "`mkswap` is available only on Linux.",
        ))
    }
}
