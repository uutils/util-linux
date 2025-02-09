// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

mod utils;

use clap::builder::{EnumValueParser, PossibleValue, PossibleValuesParser};
use clap::{crate_version, Command, ValueEnum};
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
    pub const JSON: &str = "json";
    pub const NOHEADINGS: &str = "noheadings";
    pub const OUTPUT: &str = "output";
    pub const OUTPUT_ALL: &str = "output-all";
    pub const PAIRS: &str = "pairs";
    pub const RAW: &str = "raw";
    pub const SPLIT: &str = "split";
    pub const SUMMARY: &str = "summary";
    pub const SYSROOT: &str = "sysroot";
}

const PATH_NAME_MEMORY: &str = "memory";
const PATH_NAME_NODE: &str = "node";
const PATH_SUB_BLOCK_SIZE_BYTES: &str = "block_size_bytes";
const PATH_SUB_REMOVABLE: &str = "removable";
const PATH_SUB_STATE: &str = "state";
const PATH_SUB_VALID_ZONES: &str = "valid_zones";
const PATH_SYS_MEMORY: &str = "/sys/devices/system/memory";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Column {
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

impl ValueEnum for Column {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Column::Range,
            Column::Size,
            Column::State,
            Column::Removable,
            Column::Block,
            Column::Node,
            Column::Zones,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.get_name()))
    }
}

/// Default columns to display if none are explicitly specified.
const DEFAULT_COLUMNS: &[Column] = &[
    Column::Range,
    Column::Size,
    Column::State,
    Column::Removable,
    Column::Block,
];
/// Which columns (attributes) are possible to split memory blocks to ranges on.
const SPLIT_COLUMNS: &[Column] = &[
    Column::State,
    Column::Removable,
    Column::Node,
    Column::Zones,
];

impl Column {
    fn get_name(&self) -> &'static str {
        match self {
            Column::Range => "RANGE",
            Column::Size => "SIZE",
            Column::State => "STATE",
            Column::Removable => "REMOVABLE",
            Column::Block => "BLOCK",
            Column::Node => "NODE",
            Column::Zones => "ZONES",
        }
    }

    #[allow(dead_code)]
    fn get_float_right(&self) -> bool {
        self != &Column::Range
    }

    #[allow(dead_code)]
    fn get_width_hint(&self) -> usize {
        if self == &Column::Size {
            5
        } else {
            self.get_name().len()
        }
    }

    fn get_help(&self) -> &'static str {
        match self {
            Column::Range => "start and end address of the memory range",
            Column::Size => "size of the memory range",
            Column::State => "online status of the memory range",
            Column::Removable => "memory is removable",
            Column::Block => "memory block number or blocks range",
            Column::Node => "numa node of memory",
            Column::Zones => "valid zones for the memory range",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
enum ZoneId {
    #[serde(rename = "DMA")]
    ZoneDma,
    #[serde(rename = "DMA32")]
    ZoneDma32,
    #[serde(rename = "Normal")]
    ZoneNormal,
    #[serde(rename = "Highmem")]
    ZoneHighmem,
    #[serde(rename = "Movable")]
    ZoneMovable,
    #[serde(rename = "Device")]
    ZoneDevice,
    #[serde(rename = "None")]
    ZoneNone,
    #[serde(rename = "Unknown")]
    ZoneUnknown,
    #[serde(rename = "MAX_NR_ZONES")]
    MaxNrZones,
}

impl core::fmt::Display for ZoneId {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let value = serde_json::to_string(self).unwrap().replace("\"", "");
        write!(f, "{}", value)
    }
}

impl FromStr for ZoneId {
    type Err = ();

    fn from_str(input: &str) -> Result<ZoneId, Self::Err> {
        match input.to_lowercase().as_str() {
            "dma" => Ok(ZoneId::ZoneDma),
            "dma32" => Ok(ZoneId::ZoneDma32),
            "normal" => Ok(ZoneId::ZoneNormal),
            "highmem" => Ok(ZoneId::ZoneHighmem),
            "movable" => Ok(ZoneId::ZoneMovable),
            "device" => Ok(ZoneId::ZoneDevice),
            "none" => Ok(ZoneId::ZoneNone),
            "unknown" => Ok(ZoneId::ZoneUnknown),
            _ => Err(()),
        }
    }
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

#[derive(Clone, Debug, PartialEq)]
enum Summary {
    Never,
    Always,
    Only,
}

impl ValueEnum for Summary {
    fn value_variants<'a>() -> &'a [Self] {
        &[Summary::Never, Summary::Always, Summary::Only]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Summary::Never => Some(PossibleValue::new("never").help("never show summary")),
            Summary::Always => Some(PossibleValue::new("always").help("always show summary")),
            Summary::Only => Some(PossibleValue::new("only").help("show summary only")),
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
    #[serde(skip_serializing)]
    node: String,
    #[serde(skip_serializing)]
    zones: String,
}

impl TableRow {
    fn get_value(&self, column: &Column) -> String {
        match column {
            Column::Range => self.range.clone(),
            Column::Size => self.size.clone(),
            Column::State => self.state.clone(),
            Column::Removable => self.removable.clone(),
            Column::Block => self.block.clone(),
            Column::Node => self.node.clone(),
            Column::Zones => self.zones.clone(),
        }
    }
}

struct Options {
    // Set by command-line arguments
    all: bool,
    bytes: bool,
    columns: Vec<Column>,
    noheadings: bool,
    json: bool,
    pairs: bool,
    raw: bool,
    split_by_node: bool,
    split_by_removable: bool,
    split_by_state: bool,
    split_by_zones: bool,
    /// Default to PATH_SYS_MEMORY, but a prefix can be prepended
    sysmem: String,

    // Set by read_info
    have_nodes: bool,
    have_zones: bool,

    // Computed from flags above
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
            bytes: false,
            columns: Vec::default(),
            noheadings: false,
            json: false,
            pairs: false,
            raw: false,
            split_by_node: false,
            split_by_removable: false,
            split_by_state: false,
            split_by_zones: false,
            sysmem: Path::new(PATH_SYS_MEMORY).display().to_string(),

            have_nodes: false,
            have_zones: false,

            want_summary: true, // default true
            want_table: true,   // default true
        }
    }
}

fn read_info(lsmem: &mut Lsmem, opts: &mut Options) {
    let path_block_size = Path::new(&opts.sysmem).join(PATH_SUB_BLOCK_SIZE_BYTES);
    lsmem.block_size = u64::from_str_radix(
        &read_file_content::<String>(path_block_size.as_path())
            .expect("Failed to read memory block size"),
        16,
    )
    .unwrap();
    lsmem.dirs = get_block_paths(opts);
    lsmem.dirs.sort_by(|a, b| {
        let filename_a = a.file_name().expect("Failed parsing memory block name");
        let filename_a = filename_a.to_str().unwrap();
        let filename_b = b.file_name().expect("Failed parsing memory block name");
        let filename_b = filename_b.to_str().unwrap();
        let idx_a: u64 = filename_a[PATH_NAME_MEMORY.len()..]
            .parse()
            .expect("Failed to parse memory block index");
        let idx_b: u64 = filename_b[PATH_NAME_MEMORY.len()..]
            .parse()
            .expect("Failed to parse memory block index");
        idx_a.cmp(&idx_b)
    });
    lsmem.ndirs = lsmem.dirs.len();
    for path in lsmem.dirs.iter() {
        if memory_block_get_node(path).is_ok() {
            opts.have_nodes = true;
        }

        let mut p = path.clone();
        p.push(PATH_SUB_VALID_ZONES);
        if fs::read(&p).is_ok() {
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
        if is_mergeable(lsmem, opts, &blk) {
            lsmem.blocks[lsmem.nblocks - 1].count += 1;
            continue;
        }
        lsmem.nblocks += 1;
        lsmem.blocks.push(blk.clone());
    }
}

fn get_block_paths(opts: &mut Options) -> Vec<PathBuf> {
    let mut paths = Vec::<PathBuf>::new();
    for entry in fs::read_dir(&opts.sysmem).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let filename = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() && filename.starts_with(PATH_NAME_MEMORY) {
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
    if opts.all {
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
        let filename = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() && filename.starts_with(PATH_NAME_NODE) {
            return filename[PATH_NAME_NODE.len()..].parse();
        }
    }
    Ok(-1)
}

fn memory_block_read_attrs(opts: &Options, path: &PathBuf) -> MemoryBlock {
    let mut blk = MemoryBlock::new();
    blk.count = 1;
    blk.state = MemoryState::Unknown;
    let filename = path.file_name().unwrap().to_str().unwrap();
    blk.index = filename[PATH_NAME_MEMORY.len()..].parse().unwrap();

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
        let vz_path = path.join(PATH_SUB_VALID_ZONES);
        if let Ok(raw_content) = read_file_content::<String>(Path::new(&vz_path)) {
            let zone_toks = raw_content.split(' ').collect::<Vec<&str>>();
            for (i, zone_tok) in zone_toks
                .iter()
                .map(|tok| tok.to_lowercase())
                .enumerate()
                .take(std::cmp::min(zone_toks.len(), ZoneId::MaxNrZones as usize))
            {
                blk.zones[i] = ZoneId::from_str(&zone_tok).unwrap();
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

        // Zones
        if opts.have_zones {
            row.zones = blk
                .zones
                .iter()
                .filter(|zone| **zone != ZoneId::ZoneUnknown)
                .map(|zone| zone.to_string())
                .collect::<Vec<String>>()
                .join("/");
        }

        table_rows.push(row);
    }
    table_rows
}

fn print_table(lsmem: &Lsmem, opts: &Options) {
    let table_rows = create_table_rows(lsmem, opts);
    let mut col_widths = vec![0; opts.columns.len()];

    // Initialize column widths based on pre-defined width hints
    for (i, column) in opts.columns.iter().enumerate() {
        col_widths[i] = column.get_width_hint();
    }

    // Calculate minimum column widths based on the actual data
    for row in &table_rows {
        for (i, column) in opts.columns.iter().enumerate() {
            let width = match column {
                Column::Range => row.range.len(),
                Column::Size => row.size.len(),
                Column::State => row.state.len(),
                Column::Removable => row.removable.len(),
                Column::Block => row.block.len(),
                Column::Node => row.node.len(),
                Column::Zones => row.zones.len(),
            };
            col_widths[i] = col_widths[i].max(width);
        }
    }

    if !opts.noheadings {
        let mut column_names = vec![];
        for (i, column) in opts.columns.iter().enumerate() {
            let formatted = if column.get_float_right() {
                format!("{:>width$}", column.get_name(), width = col_widths[i])
            } else {
                format!("{:<width$}", column.get_name(), width = col_widths[i])
            };
            column_names.push(formatted);
        }
        println!("{}", column_names.join(" "));
    }

    for row in table_rows {
        let mut column_values = vec![];
        for (i, column) in opts.columns.iter().enumerate() {
            let formatted = if column.get_float_right() {
                format!("{:>width$}", row.get_value(column), width = col_widths[i])
            } else {
                format!("{:<width$}", row.get_value(column), width = col_widths[i])
            };
            column_values.push(formatted);
        }
        println!("{}", column_values.join(" "));
    }
}

fn print_json(lsmem: &Lsmem, opts: &Options) {
    let table_rows = create_table_rows(lsmem, opts);
    let mut memory_records = Vec::new();

    for row in table_rows {
        let mut record = serde_json::Map::new();
        for column in &opts.columns {
            record.insert(
                column.get_name().to_lowercase(),
                if column == &Column::Size && opts.bytes {
                    serde_json::Value::Number(row.get_value(column).parse().unwrap())
                } else {
                    serde_json::Value::String(row.get_value(column))
                },
            );
        }
        memory_records.push(serde_json::Value::Object(record));
    }

    let table_json = serde_json::json!({ "memory": memory_records });

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

    for row in table_rows {
        let mut pairs = Vec::new();
        for col in &opts.columns {
            pairs.push(format!("{}=\"{}\"", col.get_name(), row.get_value(col)));
        }
        println!("{}", pairs.join(" "));
    }
}

fn print_raw(lsmem: &Lsmem, opts: &Options) {
    let table_rows = create_table_rows(lsmem, opts);

    if !opts.noheadings {
        let column_names = opts
            .columns
            .iter()
            .map(|col| col.get_name().to_string())
            .collect::<Vec<String>>();
        println!("{}", column_names.join(" "));
    }

    for row in table_rows {
        let column_values = opts
            .columns
            .iter()
            .map(|col| row.get_value(col).to_string())
            .collect::<Vec<String>>();
        println!("{}", column_values.join(" "));
    }
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
    let file = fs::File::open(path).expect("Failed to open file");
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_line(&mut content).expect("Failed to read line");
    content
        .trim()
        .to_string()
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Failed to parse content"))
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
    opts.pairs = matches.get_flag(options::PAIRS);
    opts.raw = matches.get_flag(options::RAW);
    opts.columns = matches
        .get_many::<Column>(options::OUTPUT)
        .unwrap_or_default()
        .map(|c| c.to_owned())
        .collect::<Vec<Column>>();

    // Only respect --output-all if no column list were provided.
    // --output takes priority over --output-all.
    if opts.columns.is_empty() {
        if matches.get_flag(options::OUTPUT_ALL) {
            opts.columns = Column::value_variants().to_vec();
        } else {
            opts.columns = DEFAULT_COLUMNS.to_vec();
        }
    }

    let mut split_columns = matches
        .get_many::<String>(options::SPLIT)
        .unwrap_or_default()
        .map(|c| c.to_uppercase())
        .collect::<Vec<String>>();

    // This seems like a bug in util-linux, but effectively, if split_columns is empty,
    // then DEFAULT it to the value of custom columns. Becase "zones" is not one of the
    // default columns, that means we just happen to not split on it most of the time.
    if split_columns.is_empty() {
        split_columns = opts
            .columns
            .iter()
            .map(|c| c.get_name().to_string())
            .collect();
    }

    opts.split_by_node = split_columns.contains(&Column::Node.get_name().to_string());
    opts.split_by_removable = split_columns.contains(&Column::Removable.get_name().to_string());
    opts.split_by_state = split_columns.contains(&Column::State.get_name().to_string());
    opts.split_by_zones = split_columns.contains(&Column::Zones.get_name().to_string());

    if opts.json || opts.pairs || opts.raw {
        opts.want_summary = false;
    }
    if let Some(summary) = matches.get_one::<Summary>(options::SUMMARY) {
        match summary {
            Summary::Never => opts.want_summary = false,
            Summary::Only => opts.want_table = false,
            Summary::Always => {} // Default (equivalent to if --summary wasn't provided at all)
        }
    }

    if let Some(sysroot) = matches.get_one::<String>(options::SYSROOT) {
        opts.sysmem = Path::new(sysroot).join(opts.sysmem).display().to_string();
    }

    read_info(&mut lsmem, &mut opts);

    if opts.want_table {
        if opts.json {
            print_json(&lsmem, &opts);
        } else if opts.pairs {
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
            Arg::new(options::JSON)
                .short('J')
                .long("json")
                .help("use JSON output format")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([options::PAIRS, options::RAW, options::SUMMARY]),
        )
        .arg(
            Arg::new(options::PAIRS)
                .short('P')
                .long("pairs")
                .help("use key=\"value\" output format")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([options::JSON, options::RAW, options::SUMMARY]),
        )
        .arg(
            Arg::new(options::ALL)
                .short('a')
                .long("all")
                .help("list each individual memory block")
                .action(ArgAction::SetTrue)
                .conflicts_with(options::SPLIT),
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
            Arg::new(options::OUTPUT)
                .short('o')
                .long("output")
                .help("output columns")
                .ignore_case(true)
                .action(ArgAction::Set)
                .value_name("list")
                .value_delimiter(',')
                .value_parser(EnumValueParser::<Column>::new()),
        )
        .arg(
            Arg::new(options::OUTPUT_ALL)
                .long("output-all")
                .help("output all columns")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::RAW)
                .short('r')
                .long("raw")
                .help("use raw output format")
                .action(ArgAction::SetTrue)
                .conflicts_with_all([options::JSON, options::PAIRS, options::SUMMARY]),
        )
        .arg(
            Arg::new(options::SPLIT)
                .short('S')
                .long("split")
                .help("split ranges by specified columns")
                .conflicts_with(options::ALL)
                .ignore_case(true)
                .action(ArgAction::Set)
                .value_name("list")
                .value_delimiter(',')
                .value_parser(PossibleValuesParser::new(
                    SPLIT_COLUMNS
                        .iter()
                        .map(|col| col.to_possible_value().unwrap())
                        .collect::<Vec<_>>(),
                )),
        )
        .arg(
            Arg::new(options::SYSROOT)
                .short('s')
                .long("sysroot")
                .help("use the specified directory as system root")
                .action(ArgAction::Set)
                .value_name("dir"),
        )
        .arg(
            Arg::new(options::SUMMARY)
                .long("summary")
                .help("print summary information")
                .ignore_case(true)
                .action(ArgAction::Set)
                .value_name("when")
                .value_delimiter(',')
                .value_parser(EnumValueParser::<Summary>::new())
                .conflicts_with_all([options::RAW, options::PAIRS, options::JSON])
                .num_args(0..=1)
                .default_missing_value("only"),
        )
        .after_help(format!(
            "Available output columns:\n{}",
            Column::value_variants()
                .iter()
                .map(|col| format!("{:>11}  {}", col.get_name(), col.get_help()))
                .collect::<Vec<_>>()
                .join("\n")
        ))
}
