// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

mod errors;
#[cfg(target_os = "linux")]
mod smartcols;

use std::fs::DirEntry;

use clap::{Command, crate_version};
use std::fs;
#[cfg(target_os = "linux")]
use std::os::linux::fs::MetadataExt;
#[cfg(target_os = "linux")]
use uucore::entries;
use uucore::{error::UResult, format_usage, help_about, help_usage};

use crate::errors::LsnsError;
#[cfg(target_os = "linux")]
use crate::smartcols::{Table, TableOperations};

const ABOUT: &str = help_about!("lsns.md");
const USAGE: &str = help_usage!("lsns.md");
const PATH_PROC: &str = "/proc";
const NSNAMES: [&str; 8] = ["cgroup", "ipc", "mnt", "net", "pid", "user", "uts", "time"];

#[derive(Debug, Clone, Copy)]
enum NamespaceType {
    Cgroup = 0,
    Ipc = 1,
    Mnt = 2,
    Net = 3,
    Pid = 4,
    User = 5,
    Uts = 6,
    Time = 7,
}

// Struct to store process information
struct Process {
    // Process ID - unique identifier for this process
    pid: u32,
    // User ID - the user that owns this process
    uid: u32,
    // Namespace inode IDs for each namespace type
    ns_ids: [u64; 8],
    // Command name of the process
    command: String,
}

impl Process {
    /// Creates a new instance with the given PID
    pub fn new() -> Self {
        Self {
            pid: 0,
            uid: 0,
            ns_ids: [0; 8],
            command: String::new(),
        }
    }
}

struct Namespace {
    // Namespace ID - unique identifier for this namespace
    id: u32,
    // Namespace type
    ns_type: NamespaceType,
    // Number of processes in this namespace
    nprocs: u32,
    // Representative process (lowest PID) - used for display
    representative_pid: Option<u32>,
    // Fallback UID for namespaces without processes (persistent namespaces)
    uid_fallback: u32,
}

struct Lsns {
    processes: Vec<Process>,
    namespaces: Vec<Namespace>,
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;

    let mut lsns = Lsns {
        processes: Vec::new(),
        namespaces: Vec::new(),
    };

    read_processes(PATH_PROC, &mut lsns)?;

    read_namespaces(&mut lsns)?;

    display_namespaces(&lsns)?;

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}

/// Read information of all the processes from /proc
fn read_processes(path: &str, lsns: &mut Lsns) -> Result<(), LsnsError> {
    let entries = std::fs::read_dir(path)?;

    for entry in entries {
        let _entry: DirEntry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let pid: u64 = match get_pid_from_entry(&_entry) {
            Some(p) => p,
            None => continue,
        };

        let process = match read_process(&_entry, pid as i32) {
            Some(p) => p,
            None => continue,
        };
        lsns.processes.push(process);
    }
    Ok(())
}

/// Parse /proc/[pid]/stat content to extract PID
///
/// Format: PID (COMMAND) STATE PPID ...
/// The command name can contain spaces and parentheses
fn parse_process_stat(stat: &str) -> Option<u32> {
    // Find the first '(' - marks start of command name
    let lparen_pos = stat.find('(')?;

    // Extract PID (everything before the '(')
    let pid_str = stat[..lparen_pos].trim();
    let pid: u32 = pid_str.parse().ok()?;

    Some(pid)
}

#[cfg(not(target_os = "linux"))]
fn get_uid_from_entry(_entry: &DirEntry) -> Option<u32> {
    unimplemented!()
}

#[cfg(target_os = "linux")]
fn get_uid_from_entry(entry: &DirEntry) -> Option<u32> {
    let f = entry.metadata().ok()?;
    let uid = f.st_uid();
    Some(uid)
}

#[cfg(not(target_os = "linux"))]
fn get_pid_from_entry(_entry: &DirEntry) -> Option<u64> {
    unimplemented!()
}

/// Check if a directory entry in /proc represents a process.
/// If so, returns the PID, None otherwise
#[cfg(target_os = "linux")]
fn get_pid_from_entry(entry: &DirEntry) -> Option<u64> {
    let file_name = entry.file_name();
    let name = file_name.to_str()?;

    // Check if name starts with a digit and parse as PID
    // Process directories are numeric PIDs (e.g., "1234")
    name.chars()
        .next()?
        .is_ascii_digit()
        .then(|| name.parse::<u64>().ok())?
}

#[cfg(not(target_os = "linux"))]
fn get_ns_ino(_pid: u32, _nsname: &str) -> Option<u64> {
    unimplemented!()
}

/// Get namespace inode number for a process
///
/// Reads /proc/[pid]/ns/[nsname] and returns the namespace's inode
#[cfg(target_os = "linux")]
fn get_ns_ino(pid: u32, nsname: &str) -> Option<u64> {
    let ns_path = format!("/proc/{}/ns/{}", pid, nsname);

    // Get the namespace inode by stat'ing the namespace file
    let metadata = fs::metadata(&ns_path).ok()?;
    let ino = metadata.st_ino();

    Some(ino)
}

/// Get the command name for a process
///
/// Tries to read from /proc/[pid]/cmdline first (full command line),
/// falls back to /proc/[pid]/comm (just the command name)
fn get_process_command(pid: u32) -> String {
    // Try cmdline first (full command with arguments)
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    if let Ok(content) = fs::read(&cmdline_path) {
        // cmdline uses null bytes as separators
        if !content.is_empty() {
            // Find the first null byte or use entire content
            let end = content
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(content.len());
            if end > 0
                && let Ok(cmd) = String::from_utf8(content[..end].to_vec())
            {
                return cmd;
            }
        }
    }

    // Fall back to comm (just the command name, max 16 chars)
    let comm_path = format!("/proc/{}/comm", pid);
    if let Ok(content) = fs::read_to_string(&comm_path) {
        return content.trim().to_string();
    }

    // If both fail, return placeholder
    String::from("?")
}

/// Read process information from /proc/[pid] for a single process
fn read_process(entry: &DirEntry, pid: i32) -> Option<Process> {
    let mut process = Process::new();
    process.pid = pid as u32;

    process.uid = get_uid_from_entry(entry)?;

    // Read and parse /proc/[pid]/stat to validate the process
    let stat_path = format!("/proc/{}/stat", pid);

    let stat_content = match fs::read_to_string(&stat_path) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let pid = parse_process_stat(&stat_content)?;
    process.pid = pid;

    // Get namespace inodes for all namespace types
    for (i, nsname) in NSNAMES.iter().enumerate() {
        if let Some(ino) = get_ns_ino(pid, nsname) {
            process.ns_ids[i] = ino;
        }
    }

    // Read command name from /proc/[pid]/cmdline (preferred) or /proc/[pid]/comm (fallback)
    process.command = get_process_command(pid);

    Some(process)
}

fn read_namespaces(lsns: &mut Lsns) -> Result<(), LsnsError> {
    read_assigned_namespaces(lsns);

    read_persistent_namespaces(lsns)?;

    lsns.namespaces.sort_by_key(|ns| ns.id);

    Ok(())
}

/// Read and organize namespaces from the processes we've collected
fn read_assigned_namespaces(lsns: &mut Lsns) {
    // Key: namespace inode, Value: index in lsns.namespaces vector
    let mut namespace_map: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();

    // Iterate through all processes we collected
    for proc_id in 0..lsns.processes.len() {
        let process = &lsns.processes[proc_id];

        // For each of the 8 namespace types (mnt, net, pid, uts, ipc, user, cgroup, time)
        for ns_type_id in 0..8 {
            // Get the namespace inode for this process and namespace type
            let ns_inode = process.ns_ids[ns_type_id];

            // Skip if this process doesn't have this namespace type
            // (inode = 0 means not present)
            if ns_inode == 0 {
                continue;
            }

            // Check if we've already created a Namespace struct for this inode
            let ns_idx = if let Some(&idx) = namespace_map.get(&ns_inode) {
                // Namespace already exists - use existing index
                idx
            } else {
                // This is a new namespace - create it

                // Create the new namespace
                let namespace = Namespace {
                    id: ns_inode as u32, // Cast to match your Namespace.id type
                    ns_type: NamespaceType::from_index(ns_type_id),
                    nprocs: 0, // Will increment as we add processes
                    representative_pid: Some(process.pid), // Set initial representative
                    uid_fallback: process.uid, // Fallback UID if no process later
                };

                // Add to our namespace list
                let idx = lsns.namespaces.len();
                lsns.namespaces.push(namespace);

                // Remember this namespace's index for future lookups
                namespace_map.insert(ns_inode, idx);

                idx
            };

            // Now increment the process count for this namespace
            lsns.namespaces[ns_idx].nprocs += 1;

            // Update representative process (keep the lowest PID)
            // This matches the C code: if (!ns->proc || ns->proc->pid > proc->pid)
            let should_update = match lsns.namespaces[ns_idx].representative_pid {
                None => true,                                   // No representative yet
                Some(current_pid) => process.pid < current_pid, // New process has lower PID
            };

            if should_update {
                lsns.namespaces[ns_idx].representative_pid = Some(process.pid);
            }
        }
    }
}

/// Read namespaces that are bind-mounted to the filesystem (persistent namespaces)
fn read_persistent_namespaces(lsns: &mut Lsns) -> Result<(), LsnsError> {
    // Read the mount table from /proc/self/mountinfo
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;

    // Parse each line of the mount table
    for line in mountinfo.lines() {
        // Mount table format (simplified):
        // 24 0 0:21 net:[4026531992] /var/run/netns/test rw - nsfs nsfs rw
        //                ^^^^^^^^^^^^^                                ^^^^^
        //                mount root                              filesystem type

        // Avoid collecting into Vec - iterate directly over split
        let mut parts = line.split_whitespace();

        // Skip to field 3 (mount root) - fields 0, 1, 2
        let mount_root = parts.nth(3);
        if mount_root.is_none() {
            continue;
        }
        let mount_root = mount_root.unwrap();

        // Check if this is an nsfs mount
        // The filesystem type is after the "-" separator
        // We need to find the "-" separator and check the next field
        let mut found_separator = false;
        for part in parts.by_ref() {
            if part == "-" {
                found_separator = true;
                break;
            }
        }

        if !found_separator {
            continue;
        }

        // Next field after "-" should be "nsfs"
        if parts.next() != Some("nsfs") {
            continue;
        }

        // Parse the namespace inode from the root
        // Format: "type:[inode]"
        let ns_inode = match parse_namespace_inode(mount_root) {
            Some(ino) => ino,
            None => continue, // Invalid format, skip
        };

        // Check if we already know about this namespace
        if namespace_exists(lsns, ns_inode) {
            continue;
        }

        // Extract namespace type from mount_root (format: "type:[inode]")
        // e.g., "net:[4026531992]" -> "net"
        let ns_type_str = mount_root.split(':').next().unwrap_or("");

        // Find the namespace type index
        let ns_type_idx = match NSNAMES.iter().position(|&name| name == ns_type_str) {
            Some(idx) => idx,
            None => continue, // Unknown namespace type
        };

        // Create a minimal namespace entry for persistent namespaces
        // These namespaces have no processes (nprocs = 0) and no representative
        let namespace = Namespace {
            id: ns_inode as u32,
            ns_type: NamespaceType::from_index(ns_type_idx),
            nprocs: 0,                // Persistent namespace - no processes
            representative_pid: None, // No representative process
            uid_fallback: 0,          // Default to root (UID 0) for persistent namespaces
        };

        lsns.namespaces.push(namespace);
    }

    Ok(())
}

/// Parse namespace inode from mount root string
///
/// Input format: "net:[4026531992]"
/// Returns: Some(4026531992) or None if invalid
fn parse_namespace_inode(mount_root: &str) -> Option<u64> {
    // Find the opening bracket
    let start = mount_root.find('[')?;
    // Find the closing bracket
    let end = mount_root.find(']')?;

    // Extract the number between brackets
    let inode_str = &mount_root[start + 1..end];

    // Parse to u64
    inode_str.parse::<u64>().ok()
}

/// Check if a namespace with this inode already exists
fn namespace_exists(lsns: &Lsns, ns_inode: u64) -> bool {
    lsns.namespaces.iter().any(|ns| ns.id as u64 == ns_inode)
}

/// Helper to convert namespace type index to enum
impl NamespaceType {
    fn from_index(idx: usize) -> Self {
        match idx {
            0 => NamespaceType::Cgroup,
            1 => NamespaceType::Ipc,
            2 => NamespaceType::Mnt,
            3 => NamespaceType::Net,
            4 => NamespaceType::Pid,
            5 => NamespaceType::User,
            6 => NamespaceType::Uts,
            7 => NamespaceType::Time,
            _ => panic!("Invalid namespace type index: {}", idx),
        }
    }
}

/// Display namespaces in default format using smartcols
#[cfg(target_os = "linux")]
fn display_namespaces(lsns: &Lsns) -> Result<(), LsnsError> {
    use smartcols_sys::{SCOLS_FL_RIGHT, SCOLS_FL_TRUNC};

    // Initialize smartcols
    smartcols::initialize();

    // Create table
    let mut table = Table::new()?;

    // Define columns matching the C implementation
    // NS: width_hint=10, right-aligned
    table.new_column(c"NS", 10.0, SCOLS_FL_RIGHT)?;
    // TYPE: width_hint=5, left-aligned
    table.new_column(c"TYPE", 5.0, 0)?;
    // NPROCS: width_hint=5, right-aligned
    table.new_column(c"NPROCS", 5.0, SCOLS_FL_RIGHT)?;
    // PID: width_hint=5, right-aligned
    table.new_column(c"PID", 5.0, SCOLS_FL_RIGHT)?;
    // USER: width_hint=0 (auto-size), left-aligned
    table.new_column(c"USER", 0.0, 0)?;
    // COMMAND: width_hint=0 (auto-size), truncate if too long
    table.new_column(c"COMMAND", 0.0, SCOLS_FL_TRUNC)?;

    // Build username cache once before displaying
    let mut username_cache = std::collections::HashMap::new();

    // Build process lookup map for O(1) access by PID
    let process_map: std::collections::HashMap<u32, &Process> =
        lsns.processes.iter().map(|p| (p.pid, p)).collect();

    // Add each namespace as a row
    for ns in &lsns.namespaces {
        let mut line = table.new_line(None)?;

        // Get namespace type name
        let ns_type = NSNAMES[ns.ns_type as usize];

        // Find representative process using O(1) HashMap lookup
        let rep_pid = ns.representative_pid.unwrap_or(0);
        let rep_proc = process_map.get(&rep_pid).copied();

        // Get user name
        let uid = if let Some(proc) = rep_proc {
            proc.uid
        } else {
            ns.uid_fallback
        };
        let user = get_username_from_cache(&mut username_cache, uid);

        // Get command (empty for namespaces without processes)
        let command = rep_proc.map(|p| p.command.as_str()).unwrap_or("");

        // Set cell data
        let ns_str = std::ffi::CString::new(ns.id.to_string()).unwrap();
        let type_str = std::ffi::CString::new(ns_type).unwrap();
        let nprocs_str = std::ffi::CString::new(ns.nprocs.to_string()).unwrap();
        let pid_str = if rep_pid > 0 {
            std::ffi::CString::new(rep_pid.to_string()).unwrap()
        } else {
            std::ffi::CString::new("").unwrap()
        };
        let user_str = std::ffi::CString::new(user).unwrap();
        let command_str = std::ffi::CString::new(command).unwrap();

        line.set_data(0, &ns_str)?;
        line.set_data(1, &type_str)?;
        line.set_data(2, &nprocs_str)?;
        line.set_data(3, &pid_str)?;
        line.set_data(4, &user_str)?;
        line.set_data(5, &command_str)?;
    }

    // Print the table
    table.print()?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn display_namespaces(_lsns: &Lsns) -> Result<(), LsnsError> {
    unimplemented!()
}

/// Get username from cache, querying the system if not cached
#[cfg(target_os = "linux")]
fn get_username_from_cache(cache: &mut std::collections::HashMap<u32, String>, uid: u32) -> String {
    cache
        .entry(uid)
        .or_insert_with(|| {
            // Not cached - query the system using uucore's passwd utilities (getpwuid wrapper)
            entries::uid2usr(uid).unwrap_or_else(|_| uid.to_string())
        })
        .clone()
}

#[cfg(not(target_os = "linux"))]
fn get_username_from_cache(
    _cache: &mut std::collections::HashMap<u32, String>,
    _uid: u32,
) -> String {
    unimplemented!()
}
