// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

use std::borrow::BorrowMut;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::{default, fs};

const ABOUT: &str = help_about!("lsmem.md");
const USAGE: &str = help_usage!("lsmem.md");

const PATH_SYS_MEMORY: &str = "/sys/devices/system/memory";
const PATH_BLOCK_SIZE_BYTES: &str = "/sys/devices/system/memory/block_size_bytes";

#[derive(PartialEq, Clone)]
enum ZoneId {
    ZoneDma = 0,
    ZoneDma32,
    ZoneNormal,
    ZoneHighmem,
    ZoneMovable,
    ZoneDevice,
    ZoneNone,
    ZoneUnknown,
    MaxNrZones,
}

static ZONE_NAMES: [&str; ZoneId::MaxNrZones as usize] = [
    "DMA", "DMA32", "Normal", "Highmem", "Movable", "Device",
    "None", // Block contains more than one zone, can't be offlined
    "Unknown",
];

#[derive(PartialEq, Clone)]
enum MemoryState {
    Online = 0,
    Offline,
    GoingOffline,
    Unknown,
}

#[derive(Clone)]
struct MemoryBlock {
    index: u64,
    count: u64,
    state: MemoryState,
    node: i32,
    nr_zones: usize,
    zones: [ZoneId; ZoneId::MaxNrZones as usize],
    removable: u8,
}

struct Lsmem {
    ndirs: usize,
    dirs: Vec<String>,
    blocks: Vec<MemoryBlock>,
    nblocks: usize,
    block_size: u64,
    mem_online: u64,
    mem_offline: u64,

    have_nodes: bool,
    raw: bool,
    export: bool,
    json: bool,
    noheadings: bool,
    summary: bool,
    list_all: bool,
    bytes: bool,
    want_summary: bool,
    want_table: bool,
    split_by_node: bool,
    split_by_state: bool,
    split_by_removable: bool,
    split_by_zones: bool,
    have_zones: bool,
}

impl Lsmem {
    fn new() -> Lsmem {
        Lsmem {
            ndirs: 0,
            dirs: Vec::default(),
            blocks: Vec::default(),
            nblocks: 0,
            block_size: 0,
            mem_online: 0,
            mem_offline: 0,

            have_nodes: false,
            raw: false,
            export: false,
            json: false,
            noheadings: false,
            summary: false,
            list_all: false,
            bytes: false,
            want_summary: false,
            want_table: false,
            split_by_node: false,
            split_by_state: false,
            split_by_removable: false,
            split_by_zones: false,
            have_zones: false,
        }
    }
}

fn read_info() -> Lsmem {
    let mut lsmem = Lsmem::new();
    lsmem.block_size = read_block_size_bytes().unwrap();
    let mut blocks = get_blocks();
    lsmem.ndirs = blocks.len();

    for i in 0..lsmem.ndirs {
        let blk = blocks[i].borrow_mut();
        if blk.state == MemoryState::Online {
            lsmem.mem_online += lsmem.block_size;
        } else {
            lsmem.mem_offline += lsmem.block_size;
        }
        if is_mergeable(&lsmem, &blk) {
            blocks[lsmem.nblocks - 1].count += 1;
            continue;
        }
        lsmem.nblocks += 1;
        lsmem.blocks.push(blk.clone());
    }
    return lsmem;
}

fn get_blocks() -> Vec<MemoryBlock> {
    let mut blocks = Vec::<MemoryBlock>::new();

    list_files_and_folders(PATH_SYS_MEMORY).unwrap();

    return blocks;
}

fn is_mergeable(lsmem: &Lsmem, blk: &MemoryBlock) -> bool {
    if lsmem.nblocks == 0 {
        return false;
    }

    if lsmem.list_all {
        return false;
    }

    let curr_block = &lsmem.blocks[lsmem.nblocks - 1];
    if curr_block.index + curr_block.count != blk.index {
        return false;
    }
    if lsmem.split_by_state && curr_block.state != blk.state {
        return false;
    }
    if lsmem.split_by_removable && curr_block.removable != blk.removable {
        return false;
    }
    if lsmem.split_by_node && lsmem.have_nodes {
        if curr_block.node != blk.node {
            return false;
        }
    }
    if lsmem.split_by_zones && lsmem.have_zones {
        if curr_block.nr_zones != blk.nr_zones {
            return false;
        }

        for i in 0..curr_block.nr_zones {
            if curr_block.zones[i] == ZoneId::ZoneUnknown || curr_block.zones[i] != blk.zones[i] {
                return false;
            }
        }
    }

    return true;
}

fn print_summary(lsmem: &Lsmem) {
    if lsmem.bytes {
        println!("{:<23} {:>15}", "Memory block size:", lsmem.block_size);
        println!("{:<23} {:>15}", "Total online memory:", lsmem.mem_online);
        println!("{:<23} {:>15}", "Total offline memory:", lsmem.mem_offline);
    } else {
        println!(
            "{:<23} {:>15}",
            "Memory block size:",
            size_to_human_string(lsmem.block_size)
        );
        println!(
            "{:<23} {:>15}",
            "Total online memory:",
            size_to_human_string(lsmem.mem_online)
        );
        println!(
            "{:<23} {:>15}",
            "Total offline memory:",
            size_to_human_string(lsmem.mem_offline)
        );
    }
}

fn size_to_human_string(bytes: u64) -> String {
    // char buf[32];
    // int dec, exp;
    // uint64_t frac;
    // const char *letters = "BKMGTPE";
    // char suffix[sizeof(" KiB")], *psuf = suffix;
    // char c;

    // if (options & SIZE_SUFFIX_SPACE)
    // 	*psuf++ = ' ';

    // exp  = get_exp(bytes);
    // c    = *(letters + (exp ? exp / 10 : 0));
    // dec  = exp ? bytes / (1ULL << exp) : bytes;
    // frac = exp ? bytes % (1ULL << exp) : 0;

    // *psuf++ = c;

    // if ((options & SIZE_SUFFIX_3LETTER) && (c != 'B')) {
    // 	*psuf++ = 'i';
    // 	*psuf++ = 'B';
    // }

    // *psuf = '\0';

    // /* fprintf(stderr, "exp: %d, unit: %c, dec: %d, frac: %jd\n",
    //  *                 exp, suffix[0], dec, frac);
    //  */
    // /* round */
    // if (frac) {
    // 	/* get 3 digits after decimal point */
    // 	if (frac >= UINT64_MAX / 1000)
    // 		frac = ((frac / 1024) * 1000) / (1ULL << (exp - 10)) ;
    // 	else
    // 		frac = (frac * 1000) / (1ULL << (exp)) ;

    // 	if (options & SIZE_DECIMAL_2DIGITS) {
    // 		/* round 4/5 and keep 2 digits after decimal point */
    // 		frac = (frac + 5) / 10 ;
    // 	} else {
    // 		/* round 4/5 and keep 1 digit after decimal point */
    // 		frac = ((frac + 50) / 100) * 10 ;
    // 	}

    // 	/* rounding could have overflowed */
    // 	if (frac == 100) {
    // 		dec++;
    // 		frac = 0;
    // 	}
    // }

    // if (frac) {
    // 	struct lconv const *l = localeconv();
    // 	char *dp = l ? l->decimal_point : NULL;
    // 	int len;

    // 	if (!dp || !*dp)
    // 		dp = ".";

    // 	len = snprintf(buf, sizeof(buf), "%d%s%02" PRIu64, dec, dp, frac);
    // 	if (len > 0 && (size_t) len < sizeof(buf)) {
    // 		/* remove potential extraneous zero */
    // 		if (buf[len - 1] == '0')
    // 			buf[len--] = '\0';
    // 		/* append suffix */
    // 		xstrncpy(buf+len, suffix, sizeof(buf) - len);
    // 	} else
    // 		*buf = '\0';	/* snprintf error */
    // } else
    // 	snprintf(buf, sizeof(buf), "%d%s", dec, suffix);

    // return strdup(buf);
    todo!("todo")
}

fn list_files_and_folders<P: AsRef<Path>>(path: P) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::<PathBuf>::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let filename_str = path.to_string_lossy();
        if path.is_dir() && filename_str.starts_with("memory") {
            paths.push(path);
        }
    }

    Ok(paths)
}

fn read_block_size_bytes() -> io::Result<u64> {
    // Open the file
    let file = fs::File::open("/sys/devices/system/memory/block_size_bytes")?;

    // Create a buffered reader
    let mut reader = io::BufReader::new(file);

    // Read the contents into a String
    let mut content = String::new();
    reader.read_line(&mut content)?;

    // Return the trimmed content to remove any trailing newline
    Ok(content.trim().to_string().parse().unwrap())
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let lsmem = read_info();
    print_summary(&lsmem);
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
