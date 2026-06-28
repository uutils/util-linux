// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use crate::errors::MountError;
use crate::escape::unescape_octal;
#[cfg(target_os = "linux")]
use std::path::Path;

/// Case-insensitive prefix strip that preserves the original case of the
/// remaining slice (the prefix must be ASCII-only for correctness).
fn strip_ci_prefix<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// A parsed entry from `/etc/fstab`.
#[derive(Debug, PartialEq, Clone)]
pub struct FsTabEntry {
    /// Device or remote filesystem (e.g. `/dev/sda1`, `UUID=...`, `LABEL=...`).
    pub fs_spec: String,
    /// Mount point (e.g. `/`, `/boot`).
    pub fs_file: String,
    /// Filesystem type (e.g. `ext4`, `tmpfs`).
    pub fs_vfstype: String,
    /// Mount options (e.g. `defaults`, `ro,noatime`).
    pub fs_mntops: String,
    /// Dump frequency (0 = never backed up).
    pub fs_freq: i32,
    /// `fsck` pass order (0 = skip).
    pub fs_passno: i32,
}

/// The parameters needed to perform a mount, as resolved from a label or UUID.
#[derive(Debug, PartialEq)]
pub struct ResolvedMount {
    /// Actual block device path (e.g. `/dev/sda1`).
    pub source: String,
    /// Mount point (e.g. `/boot`).
    pub target: String,
    /// Filesystem type, if known.
    pub fs_type: Option<String>,
    /// Mount options string.
    pub options: String,
}

/// Parse the contents of an `/etc/fstab`-formatted string into a list of
/// entries.
///
/// - Lines beginning with `#` and blank lines are ignored.
/// - Fields are separated by any amount of whitespace (spaces or tabs).
/// - Octal escape sequences (`\040`, `\011`, …) in field values are expanded.
/// - The `fs_freq` and `fs_passno` fields are optional and default to `0`.
/// - Lines with fewer than four fields are silently skipped.
pub fn parse_fstab_contents(contents: &str) -> Vec<FsTabEntry> {
    let mut entries = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 {
            continue;
        }
        let fs_freq = fields.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
        let fs_passno = fields.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
        entries.push(FsTabEntry {
            fs_spec: unescape_octal(fields[0]),
            fs_file: unescape_octal(fields[1]),
            fs_vfstype: fields[2].to_string(),
            fs_mntops: fields[3].to_string(),
            fs_freq,
            fs_passno,
        });
    }
    entries
}

/// Read and parse an fstab-formatted file.
#[cfg(target_os = "linux")]
pub fn parse_fstab_path(path: &Path) -> Result<Vec<FsTabEntry>, MountError> {
    let contents = std::fs::read_to_string(path)
        .map_err(|err| MountError::FstabRead(path.display().to_string(), err))?;
    Ok(parse_fstab_contents(&contents))
}

// ── Label / UUID resolution ──────────────────────────────────────────────────

/// Build a [`ResolvedMount`] from a set of fstab `entries` where `fs_spec`
/// matches `spec` (compared case-insensitively).
///
/// `display` is used in error messages (e.g. `"LABEL=boot"`).
fn resolve_from_entries(
    spec: &str,
    display: &str,
    cli_target: Option<&str>,
    entries: &[FsTabEntry],
    not_found_err: MountError,
) -> Result<ResolvedMount, MountError> {
    let entry = entries
        .iter()
        .find(|e| e.fs_spec.eq_ignore_ascii_case(spec))
        .cloned()
        .ok_or(not_found_err)?;

    let target = match cli_target {
        Some(t) => t.to_string(),
        None => entry.fs_file.clone(),
    };

    // Guard against accidentally trying to "mount" a swap entry with no
    // meaningful mount point (fs_file is "none").
    if target == "none" && cli_target.is_none() {
        return Err(MountError::NoMountPoint(display.to_string()));
    }

    Ok(ResolvedMount {
        source: entry.fs_spec.clone(),
        target,
        fs_type: Some(entry.fs_vfstype.clone()),
        options: entry.fs_mntops.clone(),
    })
}

/// Resolve a filesystem **label** to mount parameters using the supplied
/// pre-loaded fstab `entries`.
///
/// Resolution order:
/// 1. `/dev/disk/by-label/<label>` symlink (udev-maintained, most reliable).
/// 2. A fstab entry whose `fs_spec` is `LABEL=<label>` (case-insensitive).
///
/// `cli_target`, when provided, overrides the mount point found in fstab.
pub fn resolve_label_from(
    label: &str,
    cli_target: Option<&str>,
    entries: &[FsTabEntry],
) -> Result<ResolvedMount, MountError> {
    let by_label = std::path::Path::new("/dev/disk/by-label").join(label);

    if by_label.exists() {
        // Canonicalize the symlink to get the real block device path.
        let source = by_label
            .canonicalize()
            .map_err(MountError::Fstab)?
            .to_string_lossy()
            .into_owned();

        // Also check fstab for the mount point and options.
        let spec = format!("LABEL={label}");
        let entry = entries
            .iter()
            .find(|e| e.fs_spec.eq_ignore_ascii_case(spec.as_str()))
            .cloned();

        let target = match cli_target {
            Some(t) => t.to_string(),
            None => entry
                .as_ref()
                .map(|e| e.fs_file.clone())
                .ok_or_else(|| MountError::NoMountPoint(spec.clone()))?,
        };

        return Ok(ResolvedMount {
            source,
            target,
            fs_type: entry.as_ref().map(|e| e.fs_vfstype.clone()),
            options: entry
                .as_ref()
                .map(|e| e.fs_mntops.clone())
                .unwrap_or_else(|| "defaults".to_string()),
        });
    }

    // Fall back to a fstab LABEL= entry.
    let spec = format!("LABEL={label}");
    resolve_from_entries(
        &spec,
        &spec,
        cli_target,
        entries,
        MountError::LabelNotFound(label.to_string()),
    )
}

/// Resolve a **UUID** to mount parameters using the supplied pre-loaded fstab
/// `entries`.
///
/// Resolution order:
/// 1. `/dev/disk/by-uuid/<uuid>` symlink.
/// 2. A fstab entry whose `fs_spec` is `UUID=<uuid>` (case-insensitive).
///
/// `cli_target`, when provided, overrides the mount point found in fstab.
pub fn resolve_uuid_from(
    uuid: &str,
    cli_target: Option<&str>,
    entries: &[FsTabEntry],
) -> Result<ResolvedMount, MountError> {
    let by_uuid = std::path::Path::new("/dev/disk/by-uuid").join(uuid);

    if by_uuid.exists() {
        let source = by_uuid
            .canonicalize()
            .map_err(MountError::Fstab)?
            .to_string_lossy()
            .into_owned();

        let spec = format!("UUID={uuid}");
        let entry = entries
            .iter()
            .find(|e| e.fs_spec.eq_ignore_ascii_case(spec.as_str()))
            .cloned();

        let target = match cli_target {
            Some(t) => t.to_string(),
            None => entry
                .as_ref()
                .map(|e| e.fs_file.clone())
                .ok_or_else(|| MountError::NoMountPoint(spec.clone()))?,
        };

        return Ok(ResolvedMount {
            source,
            target,
            fs_type: entry.as_ref().map(|e| e.fs_vfstype.clone()),
            options: entry
                .as_ref()
                .map(|e| e.fs_mntops.clone())
                .unwrap_or_else(|| "defaults".to_string()),
        });
    }

    // Fall back to a fstab UUID= entry.
    let spec = format!("UUID={uuid}");
    resolve_from_entries(
        &spec,
        &spec,
        cli_target,
        entries,
        MountError::UuidNotFound(uuid.to_string()),
    )
}

// ── PARTLABEL / PARTUUID resolution ─────────────────────────────────────────

/// Resolve a **partition label** (`PARTLABEL=`) to mount parameters using
/// `/dev/disk/by-partlabel/<label>` and/or a matching fstab entry.
pub fn resolve_partlabel_from(
    label: &str,
    cli_target: Option<&str>,
    entries: &[FsTabEntry],
) -> Result<ResolvedMount, MountError> {
    let by_partlabel = std::path::Path::new("/dev/disk/by-partlabel").join(label);

    if by_partlabel.exists() {
        let source = by_partlabel
            .canonicalize()
            .map_err(MountError::Fstab)?
            .to_string_lossy()
            .into_owned();

        let spec = format!("PARTLABEL={label}");
        let entry = entries
            .iter()
            .find(|e| e.fs_spec.eq_ignore_ascii_case(spec.as_str()))
            .cloned();

        let target = match cli_target {
            Some(t) => t.to_string(),
            None => entry
                .as_ref()
                .map(|e| e.fs_file.clone())
                .ok_or_else(|| MountError::NoMountPoint(spec.clone()))?,
        };

        return Ok(ResolvedMount {
            source,
            target,
            fs_type: entry.as_ref().map(|e| e.fs_vfstype.clone()),
            options: entry
                .as_ref()
                .map(|e| e.fs_mntops.clone())
                .unwrap_or_else(|| "defaults".to_string()),
        });
    }

    let spec = format!("PARTLABEL={label}");
    resolve_from_entries(
        &spec,
        &spec,
        cli_target,
        entries,
        MountError::LabelNotFound(label.to_string()),
    )
}

/// Resolve a **partition UUID** (`PARTUUID=`) to mount parameters using
/// `/dev/disk/by-partuuid/<uuid>` and/or a matching fstab entry.
pub fn resolve_partuuid_from(
    uuid: &str,
    cli_target: Option<&str>,
    entries: &[FsTabEntry],
) -> Result<ResolvedMount, MountError> {
    let by_partuuid = std::path::Path::new("/dev/disk/by-partuuid").join(uuid);

    if by_partuuid.exists() {
        let source = by_partuuid
            .canonicalize()
            .map_err(MountError::Fstab)?
            .to_string_lossy()
            .into_owned();

        let spec = format!("PARTUUID={uuid}");
        let entry = entries
            .iter()
            .find(|e| e.fs_spec.eq_ignore_ascii_case(spec.as_str()))
            .cloned();

        let target = match cli_target {
            Some(t) => t.to_string(),
            None => entry
                .as_ref()
                .map(|e| e.fs_file.clone())
                .ok_or_else(|| MountError::NoMountPoint(spec.clone()))?,
        };

        return Ok(ResolvedMount {
            source,
            target,
            fs_type: entry.as_ref().map(|e| e.fs_vfstype.clone()),
            options: entry
                .as_ref()
                .map(|e| e.fs_mntops.clone())
                .unwrap_or_else(|| "defaults".to_string()),
        });
    }

    let spec = format!("PARTUUID={uuid}");
    resolve_from_entries(
        &spec,
        &spec,
        cli_target,
        entries,
        MountError::UuidNotFound(uuid.to_string()),
    )
}

fn resolved_mount_from_entry(
    entry: &FsTabEntry,
    display: &str,
) -> Result<ResolvedMount, MountError> {
    if entry.fs_file == "none" {
        return Err(MountError::NoMountPoint(display.to_string()));
    }

    Ok(ResolvedMount {
        source: resolve_spec(&entry.fs_spec).unwrap_or_else(|| entry.fs_spec.clone()),
        target: entry.fs_file.clone(),
        fs_type: Some(entry.fs_vfstype.clone()),
        options: entry.fs_mntops.clone(),
    })
}

/// Resolve a single positional mount argument through `/etc/fstab`.
///
/// The argument may name either a mount point or a source specifier. If the
/// fstab entry uses `LABEL=`, `UUID=`, `PARTLABEL=`, or `PARTUUID=`, the source
/// is resolved to its canonical device path when possible.
pub fn resolve_single_argument_from(
    arg: &str,
    entries: &[FsTabEntry],
) -> Result<ResolvedMount, MountError> {
    if let Some(entry) = entries.iter().find(|entry| entry.fs_file == arg) {
        return resolved_mount_from_entry(entry, arg);
    }

    if let Some(entry) = entries.iter().find(|entry| {
        entry.fs_spec.eq_ignore_ascii_case(arg)
            || resolve_spec(&entry.fs_spec).as_deref() == Some(arg)
    }) {
        return resolved_mount_from_entry(entry, &entry.fs_spec);
    }

    Err(MountError::FstabEntryNotFound(arg.to_string()))
}

/// Attempt to resolve a device specifier that may use a `LABEL=`, `UUID=`,
/// `PARTLABEL=`, or `PARTUUID=` prefix to a real block device path via the
/// corresponding `/dev/disk/by-*` symlink.
///
/// Returns the canonicalised device path on success, or `None` if the
/// specifier does not use a recognised prefix or the symlink does not exist.
/// Callers should fall back to using the original `spec` string unchanged.
pub fn resolve_spec(spec: &str) -> Option<String> {
    let (disk_dir, value) = if let Some(v) = strip_ci_prefix(spec, "LABEL=") {
        ("/dev/disk/by-label", v)
    } else if let Some(v) = strip_ci_prefix(spec, "UUID=") {
        ("/dev/disk/by-uuid", v)
    } else if let Some(v) = strip_ci_prefix(spec, "PARTLABEL=") {
        ("/dev/disk/by-partlabel", v)
    } else if let Some(v) = strip_ci_prefix(spec, "PARTUUID=") {
        ("/dev/disk/by-partuuid", v)
    } else {
        return None;
    };

    let path = std::path::Path::new(disk_dir).join(value);
    if path.exists() {
        path.canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    } else {
        None
    }
}
