// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgGroup, ArgMatches, Command};
use std::collections::{HashMap, HashSet};
use uucore::{
    error::{UError, UResult},
    format_usage, help_about, help_usage,
};

mod errors;
mod escape;
mod fstab;
mod mtab;
use errors::MountError;
use escape::unescape_octal;
pub use fstab::{
    parse_fstab_contents, resolve_label_from, resolve_partlabel_from, resolve_partuuid_from,
    resolve_single_argument_from, resolve_spec, resolve_uuid_from, FsTabEntry, ResolvedMount,
};
pub use mtab::write_mtab_to;

const ABOUT: &str = help_about!("mount.md");
const USAGE: &str = help_usage!("mount.md");

mod options {
    pub const ALL: &str = "all";
    pub const BIND: &str = "bind";
    pub const FAKE: &str = "fake";
    pub const FORK: &str = "fork";
    pub const ALT_FSTAB: &str = "fstab";
    pub const MKDIR: &str = "mkdir";
    pub const NO_MTAB: &str = "no-mtab";
    pub const VERBOSE: &str = "verbose";
    pub const READ_ONLY: &str = "read-only";
    pub const READ_WRITE: &str = "read-write";
    pub const FSTYPE: &str = "types";
    pub const OPTIONS: &str = "options";
    pub const TEST_OPTS: &str = "test-opts";
    pub const LABEL: &str = "label";
    pub const UUID: &str = "uuid";
    pub const PARTLABEL: &str = "partlabel";
    pub const PARTUUID: &str = "partuuid";
    pub const POSITIONAL_SOURCE: &str = "positional-source";
    pub const POSITIONAL_TARGET: &str = "positional-target";
    pub const SOURCE: &str = "source";
    pub const TARGET: &str = "target";
    pub const RBIND: &str = "rbind";
    pub const MOVE: &str = "move";
    pub const MAKE_SHARED: &str = "make-shared";
    pub const MAKE_SLAVE: &str = "make-slave";
    pub const MAKE_PRIVATE: &str = "make-private";
    pub const MAKE_UNBINDABLE: &str = "make-unbindable";
    pub const MAKE_RSHARED: &str = "make-rshared";
    pub const MAKE_RSLAVE: &str = "make-rslave";
    pub const MAKE_RPRIVATE: &str = "make-rprivate";
    pub const MAKE_RUNBINDABLE: &str = "make-runbindable";
    pub const SHOW_LABELS: &str = "show-labels";
}

/// A parsed entry from `/proc/mounts` (Linux) or equivalent.
#[derive(Debug, PartialEq)]
pub struct MountEntry {
    pub source: String,
    pub target: String,
    pub fs_type: String,
    pub options: String,
}

impl MountEntry {
    pub fn new(source: &str, target: &str, fs_type: &str, options: &str) -> Self {
        Self {
            source: source.to_string(),
            target: target.to_string(),
            fs_type: fs_type.to_string(),
            options: options.to_string(),
        }
    }
}

impl std::fmt::Display for MountEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format_mount_listing(self, None))
    }
}

pub fn format_mount_listing(entry: &MountEntry, label: Option<&str>) -> String {
    let mut rendered = format!(
        "{} on {} type {} ({})",
        entry.source, entry.target, entry.fs_type, entry.options
    );
    if let Some(label) = label.filter(|label| !label.is_empty()) {
        rendered.push_str(" [");
        rendered.push_str(label);
        rendered.push(']');
    }
    rendered
}

/// Parse mount entries from the contents of `/proc/mounts`.
pub fn parse_mount_entries(contents: &str) -> Vec<MountEntry> {
    contents
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let source = fields.next()?;
            let target = fields.next()?;
            let fs_type = fields.next()?;
            let options = fields.next()?;

            // The next two fields are optional (dump and fsck).
            let _dump = fields.next();
            let _passno = fields.next();

            // If there are more fields, the line is malformed.
            if fields.next().is_some() {
                return None;
            }

            Some(MountEntry::new(
                &unescape_octal(source),
                &unescape_octal(target),
                &unescape_octal(fs_type),
                &unescape_octal(options),
            ))
        })
        .collect()
}

/// Read currently mounted filesystems.
#[cfg(target_os = "linux")]
fn read_mounts() -> Result<Vec<MountEntry>, MountError> {
    let contents = std::fs::read_to_string("/proc/mounts").map_err(MountError::ProcMounts)?;
    Ok(parse_mount_entries(&contents))
}

/// Convert a comma-separated options string into mount flags and a remaining
/// options string for `libc::mount`.
///
/// Returns `(flags_to_set, flags_to_clear, extra_data)`.  The caller should
/// compute the effective flags as `(base | flags_to_set) & !flags_to_clear`.
/// For a fresh mount `base` is typically `0`; for a remount it should be the
/// flags already in effect so that positive options (e.g. `exec`) can undo
/// previously set negative ones (e.g. `noexec`).
#[cfg(target_os = "linux")]
pub fn options_to_flags(opts: &str) -> (libc::c_ulong, libc::c_ulong, String) {
    let mut set_flags: libc::c_ulong = 0;
    let mut clear_flags: libc::c_ulong = 0;
    let mut extra: Vec<&str> = Vec::new();

    for opt in opts.split(',') {
        match opt.trim() {
            // ── flags to set ─────────────────────────────────────────────────
            "ro" => set_flags |= libc::MS_RDONLY,
            "rw" => clear_flags |= libc::MS_RDONLY,
            "noexec" => set_flags |= libc::MS_NOEXEC,
            "nosuid" => set_flags |= libc::MS_NOSUID,
            "nodev" => set_flags |= libc::MS_NODEV,
            "sync" => set_flags |= libc::MS_SYNCHRONOUS,
            "remount" => set_flags |= libc::MS_REMOUNT,
            "mand" => set_flags |= libc::MS_MANDLOCK,
            "dirsync" => set_flags |= libc::MS_DIRSYNC,
            "noatime" => set_flags |= libc::MS_NOATIME,
            "nodiratime" => set_flags |= libc::MS_NODIRATIME,
            "relatime" => set_flags |= libc::MS_RELATIME,
            "strictatime" => set_flags |= libc::MS_STRICTATIME,
            "lazytime" => set_flags |= libc::MS_LAZYTIME,
            "bind" => set_flags |= libc::MS_BIND,
            "rbind" => set_flags |= libc::MS_BIND | libc::MS_REC,
            "shared" => set_flags |= libc::MS_SHARED,
            "slave" => set_flags |= libc::MS_SLAVE,
            "private" => set_flags |= libc::MS_PRIVATE,
            "unbindable" => set_flags |= libc::MS_UNBINDABLE,
            // ── flags to clear (positive counterparts) ────────────────────────
            "exec" => clear_flags |= libc::MS_NOEXEC,
            "suid" => clear_flags |= libc::MS_NOSUID,
            "dev" => clear_flags |= libc::MS_NODEV,
            "atime" => clear_flags |= libc::MS_NOATIME,
            "diratime" => clear_flags |= libc::MS_NODIRATIME,
            "" => {}
            o => extra.push(o),
        }
    }

    (set_flags, clear_flags, extra.join(","))
}

pub fn merge_mount_options(fstab_opts: &str, cli_opts: &str) -> String {
    [fstab_opts, cli_opts]
        .into_iter()
        .flat_map(|opts| opts.split(','))
        .map(str::trim)
        .filter(|opt| !opt.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn fstab_entry_matches_test_opts(entry: &FsTabEntry, filter: &str) -> bool {
    let entry_options: HashSet<&str> = entry
        .fs_mntops
        .split(',')
        .map(str::trim)
        .filter(|opt| !opt.is_empty())
        .collect();

    filter
        .split(',')
        .map(str::trim)
        .filter(|opt| !opt.is_empty())
        .all(|required| match required.strip_prefix("no_") {
            Some(excluded) => !entry_options.contains(excluded),
            None => entry_options.contains(required),
        })
}

/// Determine whether `fs_type` matches the `-t`/`--types` filter string.
///
/// The filter is a comma-separated list of filesystem types. A leading `no`
/// prefix excludes that type (e.g. `noext4`). Rules (matching GNU mount):
/// - If the list contains only exclusions, all types match *except* those
///   listed.
/// - If the list contains any inclusions, only those types match (and any
///   simultaneously excluded types are removed).
#[cfg(target_os = "linux")]
pub fn fstype_matches_filter(fs_type: &str, filter: &str) -> bool {
    let parts: Vec<&str> = filter
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let (included, excluded): (Vec<&str>, Vec<&str>) = parts
        .iter()
        .partition(|&&t| !t.starts_with("no") || t.len() <= 2);
    let excluded_types: Vec<&str> = excluded.iter().map(|t| &t[2..]).collect();

    if excluded_types.contains(&fs_type) {
        return false;
    }
    if included.is_empty() {
        return true;
    }
    included.contains(&fs_type)
}

fn canonical_device_path(path: &str) -> Option<String> {
    let path = std::path::Path::new(path);
    if !path.exists() {
        return None;
    }
    path.canonicalize()
        .ok()
        .map(|resolved| resolved.to_string_lossy().into_owned())
}

fn source_match_candidates(source: &str) -> HashSet<String> {
    let mut candidates = HashSet::from([source.to_string()]);
    if let Some(resolved) = resolve_spec(source) {
        candidates.insert(resolved.clone());
        if let Some(canonical) = canonical_device_path(&resolved) {
            candidates.insert(canonical);
        }
    }
    if let Some(canonical) = canonical_device_path(source) {
        candidates.insert(canonical);
    }
    candidates
}

pub fn is_already_mounted(entry: &FsTabEntry, mounts: &[MountEntry]) -> bool {
    let expected_sources = source_match_candidates(&entry.fs_spec);
    mounts.iter().any(|mount| {
        mount.target == entry.fs_file
            && !expected_sources.is_disjoint(&source_match_candidates(&mount.source))
    })
}

fn read_mount_labels() -> HashMap<String, String> {
    let mut labels = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/dev/disk/by-label") else {
        return labels;
    };

    for entry in entries.flatten() {
        let label = entry.file_name().to_string_lossy().into_owned();
        if let Ok(device) = entry.path().canonicalize() {
            labels.insert(device.to_string_lossy().into_owned(), label);
        }
    }

    labels
}

fn mount_label<'a>(entry: &MountEntry, labels: &'a HashMap<String, String>) -> Option<&'a str> {
    source_match_candidates(&entry.source)
        .into_iter()
        .find_map(|source| labels.get(&source).map(String::as_str))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropagationChange {
    pub name: &'static str,
    pub flags: libc::c_ulong,
}

pub fn collect_propagation_changes(enabled: &[&str]) -> Vec<PropagationChange> {
    enabled
        .iter()
        .filter_map(|name| match *name {
            options::MAKE_SHARED => Some(PropagationChange {
                name: options::MAKE_SHARED,
                flags: libc::MS_SHARED,
            }),
            options::MAKE_SLAVE => Some(PropagationChange {
                name: options::MAKE_SLAVE,
                flags: libc::MS_SLAVE,
            }),
            options::MAKE_PRIVATE => Some(PropagationChange {
                name: options::MAKE_PRIVATE,
                flags: libc::MS_PRIVATE,
            }),
            options::MAKE_UNBINDABLE => Some(PropagationChange {
                name: options::MAKE_UNBINDABLE,
                flags: libc::MS_UNBINDABLE,
            }),
            options::MAKE_RSHARED => Some(PropagationChange {
                name: options::MAKE_RSHARED,
                flags: libc::MS_SHARED | libc::MS_REC,
            }),
            options::MAKE_RSLAVE => Some(PropagationChange {
                name: options::MAKE_RSLAVE,
                flags: libc::MS_SLAVE | libc::MS_REC,
            }),
            options::MAKE_RPRIVATE => Some(PropagationChange {
                name: options::MAKE_RPRIVATE,
                flags: libc::MS_PRIVATE | libc::MS_REC,
            }),
            options::MAKE_RUNBINDABLE => Some(PropagationChange {
                name: options::MAKE_RUNBINDABLE,
                flags: libc::MS_UNBINDABLE | libc::MS_REC,
            }),
            _ => None,
        })
        .collect()
}

fn selected_propagation_changes(matches: &ArgMatches) -> Vec<PropagationChange> {
    let mut enabled = Vec::new();
    for option in [
        options::MAKE_SHARED,
        options::MAKE_SLAVE,
        options::MAKE_PRIVATE,
        options::MAKE_UNBINDABLE,
        options::MAKE_RSHARED,
        options::MAKE_RSLAVE,
        options::MAKE_RPRIVATE,
        options::MAKE_RUNBINDABLE,
    ] {
        if matches.get_flag(option) {
            enabled.push(option);
        }
    }
    collect_propagation_changes(&enabled)
}

#[derive(Clone, Copy)]
struct MountInvocation<'a> {
    source: Option<&'a str>,
    target: Option<&'a str>,
    lookup_arg: Option<&'a str>,
}

fn resolve_mount_invocation<'a>(matches: &'a ArgMatches) -> UResult<MountInvocation<'a>> {
    let source_opt = matches.get_one::<String>(options::SOURCE).map(String::as_str);
    let target_opt = matches.get_one::<String>(options::TARGET).map(String::as_str);
    let arg1 = matches
        .get_one::<String>(options::POSITIONAL_SOURCE)
        .map(String::as_str);
    let arg2 = matches
        .get_one::<String>(options::POSITIONAL_TARGET)
        .map(String::as_str);

    let invocation = match (source_opt, target_opt, arg1, arg2) {
        (Some(source), Some(target), None, None) => MountInvocation {
            source: Some(source),
            target: Some(target),
            lookup_arg: None,
        },
        (Some(source), None, Some(target), None) => MountInvocation {
            source: Some(source),
            target: Some(target),
            lookup_arg: None,
        },
        (None, Some(target), Some(source), None) => MountInvocation {
            source: Some(source),
            target: Some(target),
            lookup_arg: None,
        },
        (None, None, Some(source), Some(target)) => MountInvocation {
            source: Some(source),
            target: Some(target),
            lookup_arg: None,
        },
        (Some(source), None, None, None) => MountInvocation {
            source: None,
            target: None,
            lookup_arg: Some(source),
        },
        (None, Some(target), None, None) => MountInvocation {
            source: None,
            target: None,
            lookup_arg: Some(target),
        },
        (None, None, Some(arg), None) => MountInvocation {
            source: None,
            target: None,
            lookup_arg: Some(arg),
        },
        (None, None, None, None) => MountInvocation {
            source: None,
            target: None,
            lookup_arg: None,
        },
        _ => {
            return Err(uucore::error::USimpleError::new(
                1,
                "invalid combination of source/target arguments",
            ));
        }
    };

    Ok(invocation)
}

fn ensure_mount_point(target: &str) -> Result<(), MountError> {
    use std::os::unix::fs::DirBuilderExt;

    let path = std::path::Path::new(target);
    if path.is_dir() {
        return Ok(());
    }

    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o755)
        .create(path)
        .map_err(|err| MountError::CreateMountPoint(target.to_string(), err))
}

#[cfg(target_os = "linux")]
fn apply_propagation_change(
    target: &str,
    change: PropagationChange,
    verbose: bool,
    fake: bool,
) -> Result<(), MountError> {
    use std::ffi::CString;

    let c_target = CString::new(target).map_err(MountError::InvalidTarget)?;

    if verbose {
        eprintln!("mount: applying --{} to {}", change.name, target);
    }

    if fake {
        return Ok(());
    }

    let ret = unsafe {
        libc::mount(
            std::ptr::null(),
            c_target.as_ptr(),
            std::ptr::null(),
            change.flags,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        return Err(MountError::MountFailed(
            std::io::Error::last_os_error(),
            format!("--{}", change.name),
            target.to_string(),
        ));
    }

    Ok(())
}

/// Perform the actual mount syscall on Linux.
#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn do_mount(
    source: &str,
    target: &str,
    fs_type: Option<&str>,
    opts: &str,
    extra_flags: libc::c_ulong,
    extra_clear_flags: libc::c_ulong,
    verbose: bool,
    fake: bool,
    mkdir: bool,
    no_mtab: bool,
) -> Result<(), MountError> {
    use std::ffi::CString;

    macro_rules! to_cstring {
        ($s:expr, $e:expr) => {
            CString::new($s).map_err($e)
        };
    }

    let (set_flags, clear_flags, data) = options_to_flags(opts);
    let flags = (set_flags | extra_flags) & !(clear_flags | extra_clear_flags);

    let c_source = to_cstring!(source, MountError::InvalidSource)?;
    let c_target = to_cstring!(target, MountError::InvalidTarget)?;
    let c_fstype = fs_type
        .map(|t| to_cstring!(t, MountError::InvalidFSType))
        .transpose()?;
    let c_data = to_cstring!(data.as_str(), MountError::InvalidOptions)?;

    if mkdir {
        ensure_mount_point(target)?;
    }

    if verbose {
        eprintln!(
            "mount: mounting {} on {} with options \"{}\"",
            source, target, opts
        );
    }

    if fake {
        return Ok(());
    }

    let fstype_ptr = c_fstype
        .as_ref()
        .map(|s| s.as_ptr())
        .unwrap_or(std::ptr::null());

    let ret = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            fstype_ptr,
            flags,
            c_data.as_ptr() as *const libc::c_void,
        )
    };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        let hint = if err.kind() == std::io::ErrorKind::PermissionDenied {
            " (must be run as root)"
        } else {
            ""
        };
        return Err(MountError::MountFailed(
            err,
            format!("{source}{hint}"),
            target.to_string(),
        ));
    }

    if !no_mtab {
        let fstype_str = fs_type.unwrap_or("none");
        let effective_opts = if opts.is_empty() { "defaults" } else { opts };
        if let Err(e) = mtab::write_mtab(source, target, fstype_str, effective_opts) {
            eprintln!("mount: warning: failed to write to /etc/mtab: {e}");
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
#[allow(clippy::too_many_arguments)]
fn mount_fstab_entry(
    entry: &FsTabEntry,
    user_opts: &str,
    extra_flags: libc::c_ulong,
    extra_clear_flags: libc::c_ulong,
    verbose: bool,
    fake: bool,
    mkdir: bool,
    no_mtab: bool,
) -> Result<MountEntry, MountError> {
    let device = fstab::resolve_spec(&entry.fs_spec).unwrap_or_else(|| entry.fs_spec.clone());
    let effective_opts = merge_mount_options(&entry.fs_mntops, user_opts);
    do_mount(
        &device,
        &entry.fs_file,
        Some(&entry.fs_vfstype),
        &effective_opts,
        extra_flags,
        extra_clear_flags,
        verbose,
        fake,
        mkdir,
        no_mtab,
    )?;

    Ok(MountEntry::new(
        &device,
        &entry.fs_file,
        &entry.fs_vfstype,
        &effective_opts,
    ))
}

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    let verbose = matches.get_flag(options::VERBOSE);
    let fake = matches.get_flag(options::FAKE);
    let fork = matches.get_flag(options::FORK);
    let mkdir = matches.get_flag(options::MKDIR);
    let no_mtab = matches.get_flag(options::NO_MTAB);
    let read_only = matches.get_flag(options::READ_ONLY);
    let read_write = matches.get_flag(options::READ_WRITE);
    let bind = matches.get_flag(options::BIND);
    let rbind = matches.get_flag(options::RBIND);
    let move_mount = matches.get_flag(options::MOVE);
    let show_labels = matches.get_flag(options::SHOW_LABELS);
    let fstab_path = matches
        .get_one::<String>(options::ALT_FSTAB)
        .map(String::as_str)
        .unwrap_or("/etc/fstab");
    let fstype_filter = matches
        .get_one::<String>(options::FSTYPE)
        .map(String::as_str);
    let test_opts_filter = matches
        .get_one::<String>(options::TEST_OPTS)
        .map(String::as_str);
    let user_opts = matches
        .get_one::<String>(options::OPTIONS)
        .map(String::as_str)
        .unwrap_or("");
    let invocation = resolve_mount_invocation(&matches)?;
    let propagation_changes = selected_propagation_changes(&matches);

    // Compute extra kernel flags from flag arguments.
    let mut extra_flags: libc::c_ulong = 0;
    if read_only {
        extra_flags |= libc::MS_RDONLY;
    }
    if bind {
        extra_flags |= libc::MS_BIND;
    } else if rbind {
        extra_flags |= libc::MS_BIND | libc::MS_REC;
    } else if move_mount {
        extra_flags |= libc::MS_MOVE;
    }

    // -w/--read-write explicitly clears MS_RDONLY (useful when fstab says ro).
    let extra_clear_flags: libc::c_ulong = if read_write { libc::MS_RDONLY } else { 0 };

    if matches.get_flag(options::ALL) && !propagation_changes.is_empty() {
        return Err(uucore::error::USimpleError::new(
            1,
            "--make-* propagation operations cannot be combined with --all",
        ));
    }

    // ── mount --all ──────────────────────────────────────────────────────────
    if matches.get_flag(options::ALL) {
        let fstab_entries = fstab::parse_fstab_path(std::path::Path::new(fstab_path))?;
        let mut current_mounts = read_mounts()?;
        let mut had_error = false;
        let mut child_pids = Vec::new();
        for entry in fstab_entries {
            // Skip entries explicitly marked as not auto-mountable.
            if entry.fs_mntops.split(',').any(|o| o.trim() == "noauto") {
                continue;
            }
            // Apply -t filter when specified.
            if let Some(filter) = fstype_filter {
                if !fstype_matches_filter(&entry.fs_vfstype, filter) {
                    continue;
                }
            }
            if let Some(filter) = test_opts_filter {
                if !fstab_entry_matches_test_opts(&entry, filter) {
                    continue;
                }
            }
            if is_already_mounted(&entry, &current_mounts) {
                continue;
            }
            if fork {
                let pid = unsafe { libc::fork() };
                if pid < 0 {
                    return Err(MountError::Fork(std::io::Error::last_os_error()).into());
                }
                if pid == 0 {
                    let exit_code = match mount_fstab_entry(
                        &entry,
                        user_opts,
                        extra_flags,
                        extra_clear_flags,
                        verbose,
                        fake,
                        mkdir,
                        no_mtab,
                    ) {
                        Ok(_) => 0,
                        Err(e) => {
                            eprintln!("mount: {e}");
                            e.code()
                        }
                    };
                    std::process::exit(exit_code);
                }
                child_pids.push(pid);
            } else {
                match mount_fstab_entry(
                    &entry,
                    user_opts,
                    extra_flags,
                    extra_clear_flags,
                    verbose,
                    fake,
                    mkdir,
                    no_mtab,
                ) {
                    Ok(mount_entry) => current_mounts.push(mount_entry),
                    Err(e) => {
                        eprintln!("mount: {e}");
                        had_error = true;
                    }
                }
            }
        }
        for pid in child_pids {
            let mut status = 0;
            if unsafe { libc::waitpid(pid, &mut status, 0) } < 0 {
                return Err(MountError::Wait(std::io::Error::last_os_error()).into());
            }
            if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
                had_error = true;
            }
        }
        if had_error {
            return Err(uucore::error::USimpleError::new(
                errors::EXIT_MOUNT_FAILED,
                "some mounts failed",
            ));
        }
        return Ok(());
    }

    let source = invocation.source;
    let target = invocation.target;
    let lookup_arg = invocation.lookup_arg;

    if matches.contains_id(options::SOURCE)
        && (matches.contains_id(options::LABEL)
            || matches.contains_id(options::UUID)
            || matches.contains_id(options::PARTLABEL)
            || matches.contains_id(options::PARTUUID))
    {
        return Err(uucore::error::USimpleError::new(
            1,
            "cannot combine --source with --label/--uuid/--partlabel/--partuuid",
        ));
    }

    // ── mount --label / --uuid / --partlabel / --partuuid ────────────────────
    let label = matches
        .get_one::<String>(options::LABEL)
        .map(String::as_str);
    let uuid = matches.get_one::<String>(options::UUID).map(String::as_str);
    let partlabel = matches
        .get_one::<String>(options::PARTLABEL)
        .map(String::as_str);
    let partuuid = matches
        .get_one::<String>(options::PARTUUID)
        .map(String::as_str);

    if !propagation_changes.is_empty()
        && (label.is_some() || uuid.is_some() || partlabel.is_some() || partuuid.is_some())
    {
        return Err(uucore::error::USimpleError::new(
            1,
            "--make-* propagation operations require an explicit source and target mount",
        ));
    }

    if label.is_some() || uuid.is_some() || partlabel.is_some() || partuuid.is_some() {
        let fstab_entries = fstab::parse_fstab_path(std::path::Path::new(fstab_path))?;
        let cli_target = target.or(lookup_arg);

        let resolved = match (label, uuid, partlabel, partuuid) {
            (Some(lbl), None, None, None) => fstab::resolve_label_from(lbl, cli_target, &fstab_entries)?,
            (None, Some(id), None, None) => fstab::resolve_uuid_from(id, cli_target, &fstab_entries)?,
            (None, None, Some(lbl), None) => {
                fstab::resolve_partlabel_from(lbl, cli_target, &fstab_entries)?
            }
            (None, None, None, Some(id)) => {
                fstab::resolve_partuuid_from(id, cli_target, &fstab_entries)?
            }
            _ => unreachable!("clap only permits one device-identifier flag"),
        };

        let effective_opts = merge_mount_options(&resolved.options, user_opts);
        let effective_fstype = fstype_filter.or(resolved.fs_type.as_deref());

        do_mount(
            &resolved.source,
            &resolved.target,
            effective_fstype,
            &effective_opts,
            extra_flags,
            extra_clear_flags,
            verbose,
            fake,
            mkdir,
            no_mtab,
        )?;
        for change in &propagation_changes {
            apply_propagation_change(&resolved.target, *change, verbose, fake)?;
        }
        return Ok(());
    }

    // ── mount SOURCE TARGET (explicit) / list mounts ─────────────────────────
    match (source, target) {
        (Some(src), Some(tgt)) => {
            do_mount(
                src,
                tgt,
                fstype_filter,
                user_opts,
                extra_flags,
                extra_clear_flags,
                verbose,
                fake,
                mkdir,
                no_mtab,
            )?;
            for change in &propagation_changes {
                apply_propagation_change(tgt, *change, verbose, fake)?;
            }
        }
        _ if lookup_arg.is_some() => {
            if propagation_changes.is_empty() {
                let entries = fstab::parse_fstab_path(std::path::Path::new(fstab_path))?;
                let resolved = fstab::resolve_single_argument_from(lookup_arg.unwrap(), &entries)?;
                let effective_opts = merge_mount_options(&resolved.options, user_opts);
                let effective_fstype = fstype_filter.or(resolved.fs_type.as_deref());

                do_mount(
                    &resolved.source,
                    &resolved.target,
                    effective_fstype,
                    &effective_opts,
                    extra_flags,
                    extra_clear_flags,
                    verbose,
                    fake,
                    mkdir,
                    no_mtab,
                )?;
            } else {
                for change in &propagation_changes {
                    apply_propagation_change(lookup_arg.unwrap(), *change, verbose, fake)?;
                }
            }
        }
        _ => {
            if !propagation_changes.is_empty() {
                return Err(uucore::error::USimpleError::new(
                    1,
                    "--make-* propagation operations require a target mountpoint",
                ));
            }
            // No source/target: list mounts.
            let entries = read_mounts()?;
            let labels = if show_labels {
                Some(read_mount_labels())
            } else {
                None
            };
            for entry in entries {
                // Apply -t filter to listing output when specified.
                if let Some(filter) = fstype_filter {
                    if !fstype_matches_filter(&entry.fs_type, filter) {
                        continue;
                    }
                }
                let label = labels.as_ref().and_then(|labels| mount_label(&entry, labels));
                println!("{}", format_mount_listing(&entry, label));
            }
        }
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;
    Err(uucore::error::USimpleError::new(
        1,
        "`mount` is currently only supported on Linux.",
    ))
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
                .help("mount all filesystems mentioned in fstab")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::BIND)
                .short('B')
                .long("bind")
                .help("mount a subtree somewhere else (like bind)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::RBIND)
                .short('R')
                .long("rbind")
                .help("mount a subtree and all submounts somewhere else")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MOVE)
                .short('M')
                .long("move")
                .help("move a subtree to some other place")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_SHARED)
                .long("make-shared")
                .help("mark a subtree as shared")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_SLAVE)
                .long("make-slave")
                .help("mark a subtree as slave")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_PRIVATE)
                .long("make-private")
                .help("mark a subtree as private")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_UNBINDABLE)
                .long("make-unbindable")
                .help("mark a subtree as unbindable")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_RSHARED)
                .long("make-rshared")
                .help("recursively mark a whole subtree as shared")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_RSLAVE)
                .long("make-rslave")
                .help("recursively mark a whole subtree as slave")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_RPRIVATE)
                .long("make-rprivate")
                .help("recursively mark a whole subtree as private")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::MAKE_RUNBINDABLE)
                .long("make-runbindable")
                .help("recursively mark a whole subtree as unbindable")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::FAKE)
                .short('f')
                .long("fake")
                .help("dry run; skip the mount(2) syscall")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::FORK)
                .short('F')
                .long("fork")
                .help("fork off a new mount process for each device (use with --all)")
                .requires(options::ALL)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::ALT_FSTAB)
                .short('T')
                .long("fstab")
                .value_name("PATH")
                .help("use an alternative fstab file")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::MKDIR)
                .short('m')
                .long("mkdir")
                .help("create the target mountpoint if it does not exist")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::NO_MTAB)
                .short('n')
                .long("no-mtab")
                .help("don't write to /etc/mtab")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::READ_ONLY)
                .short('r')
                .long("read-only")
                .help("mount the filesystem read-only (same as -o ro)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::READ_WRITE)
                .short('w')
                .long("read-write")
                .help("mount the filesystem read-write (default)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::VERBOSE)
                .short('v')
                .long("verbose")
                .help("say what is being done")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::FSTYPE)
                .short('t')
                .long("types")
                .value_name("LIST")
                .help("filesystem type")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::OPTIONS)
                .short('o')
                .long("options")
                .value_name("LIST")
                .help("comma-separated list of mount options")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::TEST_OPTS)
                .short('O')
                .long("test-opts")
                .value_name("LIST")
                .help("limit --all to fstab entries whose option field matches LIST")
                .requires(options::ALL)
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::LABEL)
                .short('L')
                .long("label")
                .value_name("LABEL")
                .help("synonym for LABEL=<label>")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::UUID)
                .short('U')
                .long("uuid")
                .value_name("UUID")
                .help("synonym for UUID=<uuid>")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::PARTLABEL)
                .long("partlabel")
                .value_name("LABEL")
                .help("synonym for PARTLABEL=<label>")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::PARTUUID)
                .long("partuuid")
                .value_name("UUID")
                .help("synonym for PARTUUID=<uuid>")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::SHOW_LABELS)
                .short('l')
                .long("show-labels")
                .help("show filesystem labels in listing output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new(options::SOURCE)
                .long("source")
                .value_name("SOURCE")
                .help("explicitly specifies the mount source")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::TARGET)
                .long("target")
                .value_name("DIRECTORY")
                .help("explicitly specifies the mountpoint")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::POSITIONAL_SOURCE)
                .value_name("SOURCE")
                .help("special device or remote filesystem to mount")
                .index(1),
        )
        .arg(
            Arg::new(options::POSITIONAL_TARGET)
                .value_name("DIRECTORY")
                .help("mount point for the filesystem")
                .index(2),
        )
        .group(
            ArgGroup::new("mount-action")
                .args([options::BIND, options::RBIND, options::MOVE])
                .required(false),
        )
        .group(
            ArgGroup::new("rw-mode")
                .args([options::READ_ONLY, options::READ_WRITE])
                .required(false),
        )
}
