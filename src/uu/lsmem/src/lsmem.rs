// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod utils;

use clap::{crate_version, Command};
use clap::{Arg, ArgAction};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("lsmem.md");
const USAGE: &str = help_usage!("lsmem.md");

mod options {
    pub const ALL: &str = "all";
    pub const BYTES: &str = "bytes";
    pub const NOHEADINGS: &str = "noheadings";
    pub const JSON: &str = "json";
    pub const PAIRS: &str = "pairs";
    pub const RAW: &str = "raw";
}

// const BUFSIZ: usize = 1024;

const PATH_SYS_MEMORY: &str = "/sys/devices/system/memory";
const PATH_BLOCK_SIZE_BYTES: &str = "/sys/devices/system/memory/block_size_bytes";
const PATH_VALID_ZONES: &str = "/sys/devices/system/memory/valid_zones";
const PATH_SUB_REMOVABLE: &str = "removable";
const PATH_SUB_STATE: &str = "state";
const NAME_MEMORY: &str = "memory";

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Columns {
    #[serde(rename = "RANGE")]
    Range,
    #[serde(rename = "SIZE")]
    Size,
    #[serde(rename = "STATE")]
    State,
    #[serde(rename = "REMOVABLE")]
    Removable,
    #[serde(rename = "BLOCK")]
    Block,
    #[serde(rename = "NODE")]
    Node,
    #[serde(rename = "ZONES")]
    Zones,
}

const ALL_COLUMNS: &[Columns] = &[
    Columns::Range,
    Columns::Size,
    Columns::State,
    Columns::Removable,
    Columns::Block,
    Columns::Node,
    Columns::Zones,
];

impl Columns {
    fn get_name(&self) -> String {
        serde_json::to_string(self)
            .unwrap()
            .trim_matches('"')
            .to_string()
    }

    #[allow(dead_code)]
    fn get_float_direction(&self) -> &'static str {
        match self {
            Columns::Range => "<",
            Columns::Size => ">",
            Columns::State => ">",
            Columns::Removable => ">",
            Columns::Block => ">",
            Columns::Node => ">",
            Columns::Zones => ">",
        }
    }

    #[allow(dead_code)]
    fn get_width_hint(&self) -> i8 {
        if self == &Columns::Size {
            5
        } else {
            self.get_name().len() as i8
        }
    }

    fn get_help(&self) -> &'static str {
        match self {
            Columns::Range => "start and end address of the memory range",
            Columns::Size => "size of the memory range",
            Columns::State => "online status of the memory range",
            Columns::Removable => "memory is removable",
            Columns::Block => "memory block number or blocks range",
            Columns::Node => "numa node of memory",
            Columns::Zones => "valid zones for the memory range",
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
enum ZoneId {
    #[serde(rename = "ZONE_DMA")]
    ZoneDma,
    #[serde(rename = "ZONE_DMA32")]
    ZoneDma32,
    #[serde(rename = "ZONE_NORMAL")]
    ZoneNormal,
    #[serde(rename = "ZONE_HIGHMEM")]
    ZoneHighmem,
    #[serde(rename = "ZONE_MOVABLE")]
    ZoneMovable,
    #[serde(rename = "ZONE_DEVICE")]
    ZoneDevice,
    #[serde(rename = "ZONE_NONE")]
    ZoneNone,
    #[serde(rename = "ZONE_UNKNOWN")]
    ZoneUnknown,
    #[serde(rename = "MAX_NR_ZONES")]
    MaxNrZones,
}

#[derive(PartialEq, Clone)]
enum MemoryState {
    Online,
    Offline,
    GoingOffline,
    Unknown,
}

impl core::fmt::Display for MemoryState {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            MemoryState::Online => write!(f, "online"),
            MemoryState::Offline => write!(f, "offline"),
            MemoryState::GoingOffline => write!(f, "going-offline"),
            MemoryState::Unknown => write!(f, "unknown"),
        }
    }
}

impl FromStr for MemoryState {
    type Err = ();
    fn from_str(input: &str) -> Result<MemoryState, Self::Err> {
        match input {
            "online" => Ok(MemoryState::Online),
            "offline" => Ok(MemoryState::Offline),
            "going-offline" => Ok(MemoryState::GoingOffline),
            "unknown" => Ok(MemoryState::Unknown),
            _ => Err(()),
        }
    }
}

#[derive(Clone)]
struct MemoryBlock {
    index: u64,
    count: u64,
    state: MemoryState,
    node: i32,
    nr_zones: usize,
    zones: [ZoneId; ZoneId::MaxNrZones as usize],
    removable: bool,
}

impl MemoryBlock {
    fn new() -> Self {
        MemoryBlock {
            index: 0,
            count: 0,
            state: MemoryState::Unknown,
            node: 0,
            nr_zones: 0,
            zones: [ZoneId::ZoneUnknown; ZoneId::MaxNrZones as usize],
            removable: true,
        }
    }
}

#[derive(Default, Serialize)]
struct TableRow {
    range: String,
    size: String,
    state: String,
    removable: String,
    block: String,
    node: String,
    #[allow(unused)]
    #[serde(skip_serializing)]
    zones: String,
}

impl TableRow {
    fn to_pairs_string(&self) -> String {
        format!(
            r#"RANGE="{}" SIZE="{}" STATE="{}" REMOVABLE="{}" BLOCK="{}""#,
            self.range, self.size, self.state, self.removable, self.block
        )
    }
    fn to_raw_string(&self) -> String {
        format!(
            r#"{} {} {} {} {}"#,
            self.range, self.size, self.state, self.removable, self.block
        )
    }
}

#[derive(Serialize)]
struct TableRowJson {
    memory: Vec<TableRow>,
}

struct Options {
    all: bool,
    bytes: bool,
    export: bool,
    have_nodes: bool,
    have_zones: bool,
    json: bool,
    list_all: bool,
    noheadings: bool,
    raw: bool,
    split_by_node: bool,
    split_by_removable: bool,
    split_by_state: bool,
    split_by_zones: bool,
    want_summary: bool,
    want_table: bool,
}

struct Lsmem {
    ndirs: usize,
    dirs: Vec<PathBuf>,
    blocks: Vec<MemoryBlock>,
    nblocks: usize,
    block_size: u64,
    mem_online: u64,
    mem_offline: u64,
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
        }
    }
}

impl Options {
    fn new() -> Options {
        Options {
            all: false,
            have_nodes: false,
            raw: false,
            export: false,
            json: false,
            noheadings: false,
            list_all: false,
            bytes: false,
            want_summary: true, // default true
            want_table: true,   // default true
            split_by_node: false,
            split_by_state: false,
            split_by_removable: false,
            split_by_zones: false,
            have_zones: false,
        }
    }
}

fn read_info(lsmem: &mut Lsmem, opts: &mut Options) {
    lsmem.block_size = u64::from_str_radix(
        &read_file_content::<String>(Path::new(PATH_BLOCK_SIZE_BYTES)).unwrap(),
        16,
    )
    .unwrap();
    lsmem.dirs = get_block_paths();
    lsmem.dirs.sort_by(|a, b| {
        let filename_a = a.to_str().unwrap().split('/').last().unwrap();
        let filename_b = b.to_str().unwrap().split('/').last().unwrap();
        let idx_a: u64 = filename_a[NAME_MEMORY.len()..].parse().unwrap();
        let idx_b: u64 = filename_b[NAME_MEMORY.len()..].parse().unwrap();
        idx_a.cmp(&idx_b)
    });
    lsmem.ndirs = lsmem.dirs.len();
    for path in lsmem.dirs.iter() {
        if memory_block_get_node(path).is_ok() {
            opts.have_nodes = true;
        }

        let mut p = path.clone();
        p.push("valid_zones");
        if fs::read_dir(p).is_ok() {
            opts.have_zones = true;
        }

        if opts.have_nodes && opts.have_zones {
            break;
        }
    }

    for i in 0..lsmem.ndirs {
        let blk = memory_block_read_attrs(opts, &lsmem.dirs[i]);
        if blk.state == MemoryState::Online {
            lsmem.mem_online += lsmem.block_size;
        } else {
            lsmem.mem_offline += lsmem.block_size;
        }
        if !opts.all && is_mergeable(lsmem, opts, &blk) {
            lsmem.blocks[lsmem.nblocks - 1].count += 1;
            continue;
        }
        lsmem.nblocks += 1;
        lsmem.blocks.push(blk.clone());
    }
}

fn get_block_paths() -> Vec<PathBuf> {
    let mut paths = Vec::<PathBuf>::new();
    for entry in fs::read_dir(PATH_SYS_MEMORY).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let filename = path.to_str().unwrap().split('/').last().unwrap();
        if path.is_dir() && filename.starts_with(NAME_MEMORY) {
            paths.push(path);
        }
    }
    paths
}

fn is_mergeable(lsmem: &Lsmem, opts: &Options, blk: &MemoryBlock) -> bool {
    if lsmem.nblocks == 0 {
        return false;
    }

    let curr_block = &lsmem.blocks[lsmem.nblocks - 1];
    if opts.list_all {
        return false;
    }
    if curr_block.index + curr_block.count != blk.index {
        return false;
    }
    if opts.split_by_state && curr_block.state != blk.state {
        return false;
    }
    if opts.split_by_removable && curr_block.removable != blk.removable {
        return false;
    }
    if opts.split_by_node && opts.have_nodes && (curr_block.node != blk.node) {
        return false;
    }
    if opts.split_by_zones && opts.have_zones {
        if curr_block.nr_zones != blk.nr_zones {
            return false;
        }

        for i in 0..curr_block.nr_zones {
            if curr_block.zones[i] == ZoneId::ZoneUnknown || curr_block.zones[i] != blk.zones[i] {
                return false;
            }
        }
    }
    true
}

fn memory_block_get_node(path: &PathBuf) -> Result<i32, <i32 as FromStr>::Err> {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let filename = path.to_str().unwrap().split('/').last().unwrap();
        if path.is_dir() && filename.starts_with("node") {
            return filename["node".len()..].parse();
        }
    }
    Ok(-1)
}

fn memory_block_read_attrs(opts: &Options, path: &PathBuf) -> MemoryBlock {
    let mut blk = MemoryBlock::new();
    blk.count = 1;
    blk.state = MemoryState::Unknown;
    let filename = path.to_str().unwrap().split('/').last().unwrap();
    blk.index = filename[NAME_MEMORY.len()..].parse().unwrap();

    let mut removable_path = path.clone();
    removable_path.push(PATH_SUB_REMOVABLE);
    blk.removable = read_file_content::<i32>(&removable_path).is_ok();

    let mut state_path = path.clone();
    state_path.push(PATH_SUB_STATE);
    if let Ok(state_raw) = read_file_content::<String>(&state_path) {
        blk.state = MemoryState::from_str(&state_raw).unwrap();
    }

    if opts.have_nodes {
        blk.node = memory_block_get_node(path).unwrap();
    }

    blk.nr_zones = 0;
    if opts.have_zones {
        if let Ok(raw_content) = read_file_content::<String>(Path::new(PATH_VALID_ZONES)) {
            let zone_toks = raw_content.split(' ').collect::<Vec<&str>>();
            for (i, zone_tok) in zone_toks
                .iter()
                .enumerate()
                .take(std::cmp::min(zone_toks.len(), ZoneId::MaxNrZones as usize))
            {
                blk.zones[i] = serde_json::from_str(zone_tok).unwrap();
                blk.nr_zones += 1;
            }
        }
    }
    blk
}

fn create_table_rows(lsmem: &Lsmem, opts: &Options) -> Vec<TableRow> {
    let mut table_rows = Vec::<TableRow>::new();

    for i in 0..lsmem.nblocks {
        let mut row = TableRow::default();

        let blk = lsmem.blocks[i].borrow();

        // Range
        let start = blk.index * lsmem.block_size;
        let size = blk.count * lsmem.block_size;
        row.range = format!("0x{:016x}-0x{:016x}", start, start + size - 1);

        // Size
        row.size = if opts.bytes {
            format!("{}", blk.count * lsmem.block_size)
        } else {
            utils::size_to_human_string(blk.count * lsmem.block_size)
        };

        // State
        row.state = match blk.state {
            MemoryState::Online => MemoryState::Online.to_string(),
            MemoryState::Offline => MemoryState::Offline.to_string(),
            MemoryState::GoingOffline => MemoryState::GoingOffline.to_string(),
            MemoryState::Unknown => "?".to_string(),
        };

        // Removable
        row.removable = if blk.removable {
            "yes".to_string()
        } else {
            "no".to_string()
        };

        // Block
        row.block = if blk.count == 1 {
            format!("{}", blk.index)
        } else {
            format!("{}-{}", blk.index, blk.index + blk.count - 1)
        };

        // Node
        if opts.have_nodes {
            row.node = format!("{}", blk.node);
        }

        table_rows.push(row);
    }
    table_rows
}

fn print_table(lsmem: &Lsmem, opts: &Options) {
    let table_rows = create_table_rows(lsmem, opts);
    let mut col_widths = vec![5, 5, 6, 9, 5]; // Initialize with default minimum widths

    // Calculate minimum column widths based on the actual data
    for row in &table_rows {
        col_widths[0] = col_widths[0].max(row.range.len());
        col_widths[1] = col_widths[1].max(row.size.len());
        col_widths[2] = col_widths[2].max(row.state.len());
        col_widths[3] = col_widths[3].max(row.removable.len());
        col_widths[4] = col_widths[4].max(row.block.len());
    }

    if !opts.noheadings {
        println!(
            "{:<col0$} {:>col1$} {:>col2$} {:>col3$} {:>col4$}",
            "RANGE",
            "SIZE",
            "STATE",
            "REMOVABLE",
            "BLOCK",
            col0 = col_widths[0],
            col1 = col_widths[1],
            col2 = col_widths[2],
            col3 = col_widths[3],
            col4 = col_widths[4],
        );
    }

    for row in table_rows {
        let mut columns = vec![];
        columns.push(format!("{:<col0$}", row.range, col0 = col_widths[0]));
        columns.push(format!("{:>col1$}", row.size, col1 = col_widths[1]));
        columns.push(format!("{:>col2$}", row.state, col2 = col_widths[2]));
        columns.push(format!("{:>col3$}", row.removable, col3 = col_widths[3]));
        columns.push(format!("{:>col4$}", row.block, col4 = col_widths[4]));
        // Default version skips NODE and ZONES
        // columns.push(format!("{:>col5$}", row.node, col5 = col_widths[5]));
        // columns.push(format!("{:>col6$}", row.zones, col6 = col_widths[6]));
        println!("{}", columns.join(" "));
    }
}

fn print_json(lsmem: &Lsmem, opts: &Options) {
    let table_json = TableRowJson {
        memory: create_table_rows(lsmem, opts),
    };

    let mut table_json_string = serde_json::to_string_pretty(&table_json)
        .unwrap()
        .replace("  ", "   ") // Ident 3 spaces
        .replace("},\n      {", "},{"); // Remove newlines between '}, {'
    table_json_string = table_json_string.replace("\"yes\"", "true");
    table_json_string = table_json_string.replace("\"no\"", "false");
    println!("{table_json_string}");
}

fn print_pairs(lsmem: &Lsmem, opts: &Options) {
    let table_rows = create_table_rows(lsmem, opts);
    let table_pairs_string = table_rows
        .into_iter()
        .map(|row| row.to_pairs_string())
        .collect::<Vec<_>>()
        .join("\n");
    println!("{table_pairs_string}");
}

fn print_raw(lsmem: &Lsmem, opts: &Options) {
    let table_rows = create_table_rows(lsmem, opts);
    let mut table_raw_string = String::new();
    for row in table_rows {
        table_raw_string += &row.to_raw_string();
        table_raw_string += "\n";
    }
    // remove the last newline
    table_raw_string.pop();
    println!("RANGE SIZE STATE REMOVABLE BLOCK");
    println!("{table_raw_string}");
}

fn print_summary(lsmem: &Lsmem, opts: &Options) {
    if opts.bytes {
        println!("{:<23} {:>15}", "Memory block size:", lsmem.block_size);
        println!("{:<23} {:>15}", "Total online memory:", lsmem.mem_online);
        println!("{:<23} {:>15}", "Total offline memory:", lsmem.mem_offline);
    } else {
        let block_size_str = utils::size_to_human_string(lsmem.block_size);
        let mem_online_str = utils::size_to_human_string(lsmem.mem_online);
        let mem_offline_str = utils::size_to_human_string(lsmem.mem_offline);

        println!("{:<23} {:>5}", "Memory block size:", block_size_str);
        println!("{:<23} {:>5}", "Total online memory:", mem_online_str);
        println!("{:<23} {:>5}", "Total offline memory:", mem_offline_str);
    }
}

fn read_file_content<T: core::str::FromStr>(path: &Path) -> io::Result<T>
where
    T::Err: std::fmt::Debug, // Required to unwrap the result of T::from_str
{
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_line(&mut content)?;
    Ok(content.trim().to_string().parse().unwrap())
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    let mut lsmem = Lsmem::new();
    let mut opts = Options::new();
    opts.all = matches.get_flag(options::ALL);
    opts.bytes = matches.get_flag(options::BYTES);
    opts.noheadings = matches.get_flag(options::NOHEADINGS);
    opts.json = matches.get_flag(options::JSON);
    opts.export = matches.get_flag(options::PAIRS);
    opts.raw = matches.get_flag(options::RAW);

    if opts.json || opts.export || opts.raw {
        opts.want_summary = false;
    }

    read_info(&mut lsmem, &mut opts);

    if opts.want_table {
        if opts.json {
            print_json(&lsmem, &opts);
        } else if opts.export {
            print_pairs(&lsmem, &opts);
        } else if opts.raw {
            print_raw(&lsmem, &opts);
        } else {
            print_table(&lsmem, &opts);
        }
    }

    // Padding line between table and summary if both are shown
    if opts.want_table && opts.want_summary {
        println!();
    }

    if opts.want_summary {
        print_summary(&lsmem, &opts);
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::ALL)
                .short('a')
                .long("all")
                .help("list each individual memory block")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::BYTES)
                .short('b')
                .long("bytes")
                .help("print SIZE in bytes rather than in human readable format")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::NOHEADINGS)
                .short('n')
                .long("noheadings")
                .help("don't print headings")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long("json")
                .help("use JSON output format")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([options::PAIRS, options::RAW]),
        )
        .arg(
            Arg::new(options::PAIRS)
                .short('P')
                .long("pairs")
                .help("use key=\"value\" output format")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([options::JSON, options::RAW]),
        )
        .arg(
            Arg::new(options::RAW)
                .short('r')
                .long("raw")
                .help("use raw output format")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([options::JSON, options::PAIRS]),
        )
        .after_help(&format!(
            "Available output columns:\n{}",
            ALL_COLUMNS
                .iter()
                .map(|col| format!("{:>11}  {}", col.get_name(), col.get_help()))
                .collect::<Vec<_>>()
                .join("\n")
        ))
}
