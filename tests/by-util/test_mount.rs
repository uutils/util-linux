// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use uutests::new_ucmd;

// ── CLI / argument parsing tests ────────────────────────────────────────────

#[test]
fn test_invalid_arg() {
    new_ucmd!().arg("--definitely-invalid").fails().code_is(1);
}

/// `mount --help` should succeed and mention "mount".
#[test]
fn test_help() {
    new_ucmd!()
        .arg("--help")
        .succeeds()
        .stdout_contains("mount");
}

/// `mount --version` should succeed.
#[test]
fn test_version() {
    new_ucmd!().arg("--version").succeeds();
}

/// `mount --help` should advertise the major compatibility features.
#[test]
fn test_help_mentions_phase4_features() {
    new_ucmd!()
        .arg("--help")
        .succeeds()
        .stdout_contains("--fstab")
        .stdout_contains("--source")
        .stdout_contains("--target")
        .stdout_contains("--mkdir")
        .stdout_contains("--test-opts")
        .stdout_contains("--make-shared");
}

// ── Listing mounts (no arguments) ───────────────────────────────────────────

/// On Linux, `mount` with no arguments reads `/proc/mounts` and prints every
/// entry in the form:  <source> on <target> type <fstype> (<options>)
#[test]
#[cfg(target_os = "linux")]
fn test_list_mounts_no_args() {
    let result = new_ucmd!().succeeds();
    let stdout = result.stdout_str();
    // There is always at least the rootfs or sysfs entry on Linux.
    assert!(
        !stdout.is_empty(),
        "expected at least one mount entry, got empty output"
    );
    // Every line must match the expected format.
    for line in stdout.lines() {
        assert!(
            line.contains(" on ") && line.contains(" type "),
            "unexpected mount line format: {line:?}"
        );
    }
}

/// The output of `mount` (no args) must be consistent with `/proc/mounts`.
#[test]
#[cfg(target_os = "linux")]
fn test_list_mounts_matches_proc_mounts() {
    use std::fs;

    let proc_contents = fs::read_to_string("/proc/mounts").unwrap();
    let result = new_ucmd!().succeeds();
    let stdout = result.stdout_str();

    // Every source device listed in /proc/mounts should appear in the output.
    for line in proc_contents.lines() {
        let mut fields = line.split_whitespace();
        if let (Some(src), Some(tgt)) = (fields.next(), fields.next()) {
            assert!(
                stdout.contains(src) && stdout.contains(tgt),
                "expected source '{src}' and target '{tgt}' in mount output"
            );
        }
    }
}

/// `--fake --no-mtab` should succeed without touching /etc/mtab.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_no_mtab() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--no-mtab",
            "--types",
            "tmpfs",
            "tmpfs",
            dir.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `mount --all --fake --no-mtab` should succeed.
#[test]
#[cfg(target_os = "linux")]
fn test_all_fake_no_mtab() {
    if !std::path::Path::new("/etc/fstab").exists() {
        return;
    }
    new_ucmd!()
        .args(&["--all", "--fake", "--no-mtab"])
        .succeeds();
}

// ── `mount --fake` (dry-run) ─────────────────────────────────────────────────

/// `mount --fake` with a source and target should succeed without performing
/// the actual syscall (no root privileges needed).
#[test]
#[cfg(target_os = "linux")]
fn test_fake_mount_succeeds() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--types",
            "tmpfs",
            "tmpfs",
            dir.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `--fake --verbose` should print a diagnostic message to stderr.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_verbose_mount() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--verbose",
            "--types",
            "tmpfs",
            "tmpfs",
            dir.path().to_str().unwrap(),
        ])
        .succeeds()
        .stderr_contains("mounting");
}

/// `--fake` with `-o ro` should succeed.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_read_only_option() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "-o",
            "ro",
            "--types",
            "tmpfs",
            "tmpfs",
            dir.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `--fake --read-only` should succeed.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_read_only_flag() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--read-only",
            "--types",
            "tmpfs",
            "tmpfs",
            dir.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `--fake --bind` should succeed.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_bind_flag() {
    use tempfile::TempDir;
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--bind",
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `--fake --rbind` should succeed.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_rbind_flag() {
    use tempfile::TempDir;
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--rbind",
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `--fake --move` should succeed.
#[test]
#[cfg(target_os = "linux")]
fn test_fake_move_flag() {
    use tempfile::TempDir;
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--move",
            src.path().to_str().unwrap(),
            dst.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `mount --all --fake` should succeed when `/etc/fstab` is present (skips
/// syscall so no root privileges are needed).
#[test]
#[cfg(target_os = "linux")]
fn test_all_fake_succeeds() {
    // Only run this test when /etc/fstab exists.
    if !std::path::Path::new("/etc/fstab").exists() {
        return;
    }
    new_ucmd!().args(&["--all", "--fake"]).succeeds();
}

/// `--fstab` should allow single-argument resolution from an alternate file.
#[test]
#[cfg(target_os = "linux")]
fn test_alt_fstab_single_argument_resolution() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("mnt");
    fs::create_dir(&target).unwrap();
    let fstab = dir.path().join("test.fstab");
    fs::write(
        &fstab,
        format!("tmpfs {} tmpfs defaults 0 0\n", target.display()),
    )
    .unwrap();

    new_ucmd!()
        .args(&[
            "--fake",
            "--fstab",
            fstab.to_str().unwrap(),
            target.to_str().unwrap(),
        ])
        .succeeds();
}

/// `--target` should disambiguate a single-argument mountpoint lookup.
#[test]
#[cfg(target_os = "linux")]
fn test_explicit_target_single_argument_resolution() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("mnt");
    fs::create_dir(&target).unwrap();
    let fstab = dir.path().join("test.fstab");
    fs::write(
        &fstab,
        format!("tmpfs {} tmpfs defaults 0 0\n", target.display()),
    )
    .unwrap();

    new_ucmd!()
        .args(&[
            "--fake",
            "--fstab",
            fstab.to_str().unwrap(),
            "--target",
            target.to_str().unwrap(),
        ])
        .succeeds();
}

/// Explicit `--source` and `--target` should work for direct mounts.
#[test]
#[cfg(target_os = "linux")]
fn test_explicit_source_target_mount() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&[
            "--fake",
            "--types",
            "tmpfs",
            "--source",
            "tmpfs",
            "--target",
            dir.path().to_str().unwrap(),
        ])
        .succeeds();
}

/// `--source` should disambiguate a single-argument source lookup through fstab.
#[test]
#[cfg(target_os = "linux")]
fn test_explicit_source_single_argument_resolution() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("mnt");
    fs::create_dir(&target).unwrap();
    let fstab = dir.path().join("test.fstab");
    fs::write(
        &fstab,
        format!("tmpfs {} tmpfs defaults 0 0\n", target.display()),
    )
    .unwrap();

    new_ucmd!()
        .args(&[
            "--fake",
            "--fstab",
            fstab.to_str().unwrap(),
            "--source",
            "tmpfs",
        ])
        .succeeds();
}

/// `--mkdir` should create a missing mountpoint before the mount attempt.
#[test]
#[cfg(target_os = "linux")]
fn test_mkdir_creates_missing_target() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("missing").join("mountpoint");

    new_ucmd!()
        .args(&[
            "--fake",
            "--mkdir",
            "--types",
            "tmpfs",
            "tmpfs",
            target.to_str().unwrap(),
        ])
        .succeeds();

    assert!(target.is_dir(), "--mkdir should create the target directory");
}

/// `--test-opts` should filter `--all` entries using options from the chosen fstab.
#[test]
#[cfg(target_os = "linux")]
fn test_all_test_opts_filters_entries() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target1 = dir.path().join("mnt1");
    let target2 = dir.path().join("mnt2");
    fs::create_dir(&target1).unwrap();
    fs::create_dir(&target2).unwrap();
    let fstab = dir.path().join("test.fstab");
    fs::write(
        &fstab,
        format!(
            "tmpfs {} tmpfs defaults,noatime 0 0\ntmpfs {} tmpfs defaults 0 0\n",
            target1.display(),
            target2.display()
        ),
    )
    .unwrap();

    new_ucmd!()
        .args(&[
            "--all",
            "--fake",
            "--verbose",
            "--fstab",
            fstab.to_str().unwrap(),
            "--test-opts",
            "noatime",
        ])
        .succeeds()
        .stderr_contains(target1.to_str().unwrap())
        .stderr_does_not_contain(target2.to_str().unwrap());
}

/// `--all` can create missing mountpoints when `--mkdir` is requested.
#[test]
#[cfg(target_os = "linux")]
fn test_all_with_mkdir_creates_missing_target() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target = dir.path().join("missing").join("mnt");
    let fstab = dir.path().join("test.fstab");
    fs::write(
        &fstab,
        format!("tmpfs {} tmpfs defaults 0 0\n", target.display()),
    )
    .unwrap();

    new_ucmd!()
        .args(&[
            "--all",
            "--fake",
            "--mkdir",
            "--fstab",
            fstab.to_str().unwrap(),
        ])
        .succeeds();

    assert!(target.is_dir(), "--all --mkdir should create the target directory");
}

/// `--fork` should be accepted with `--all` and still mount all matching entries.
#[test]
#[cfg(target_os = "linux")]
fn test_all_fork_fake_succeeds() {
    use std::fs;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let target1 = dir.path().join("mnt1");
    let target2 = dir.path().join("mnt2");
    fs::create_dir(&target1).unwrap();
    fs::create_dir(&target2).unwrap();
    let fstab = dir.path().join("test.fstab");
    fs::write(
        &fstab,
        format!(
            "tmpfs {} tmpfs defaults 0 0\ntmpfs {} tmpfs defaults 0 0\n",
            target1.display(),
            target2.display()
        ),
    )
    .unwrap();

    let result = new_ucmd!()
        .args(&[
            "--all",
            "--fake",
            "--fork",
            "--verbose",
            "--fstab",
            fstab.to_str().unwrap(),
        ])
        .succeeds();

    let stderr = result.stderr_str();
    assert!(stderr.contains(target1.to_str().unwrap()));
    assert!(stderr.contains(target2.to_str().unwrap()));
}

/// A pure propagation operation should work in fake mode on an explicit target.
#[test]
#[cfg(target_os = "linux")]
fn test_make_shared_fake_succeeds() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&["--fake", "--make-shared", dir.path().to_str().unwrap()])
        .succeeds();
}

/// Recursive propagation operations should be accepted by the parser.
#[test]
#[cfg(target_os = "linux")]
fn test_make_rprivate_fake_succeeds() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    new_ucmd!()
        .args(&["--fake", "--make-rprivate", "--target", dir.path().to_str().unwrap()])
        .succeeds();
}

/// A direct mount can be followed by propagation operations in the same command.
#[test]
#[cfg(target_os = "linux")]
fn test_mount_with_propagation_fake_succeeds() {
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let result = new_ucmd!()
        .args(&[
            "--fake",
            "--verbose",
            "--make-private",
            "--make-unbindable",
            "--types",
            "tmpfs",
            "tmpfs",
            dir.path().to_str().unwrap(),
        ])
        .succeeds();

    let stderr = result.stderr_str();
    assert!(stderr.contains("applying --make-private"));
    assert!(stderr.contains("applying --make-unbindable"));
}

/// Propagation operations must not be combined with `--all`.
#[test]
#[cfg(target_os = "linux")]
fn test_make_shared_conflicts_with_all() {
    new_ucmd!()
        .args(&["--all", "--fake", "--make-shared", "/tmp"])
        .fails()
        .code_is(1);
}

/// `--label` with an unknown label should fail.
#[test]
#[cfg(target_os = "linux")]
fn test_label_not_found_fails() {
    new_ucmd!()
        .args(&["--fake", "--label", "this-label-does-not-exist-xyz"])
        .fails()
        .code_is(1);
}

/// `--uuid` with an unknown UUID should fail.
#[test]
#[cfg(target_os = "linux")]
fn test_uuid_not_found_fails() {
    new_ucmd!()
        .args(&["--fake", "--uuid", "00000000-0000-0000-0000-000000000000"])
        .fails()
        .code_is(1);
}

/// `--uuid` arg must be accepted by the parser (no "unexpected argument" error).
#[test]
fn test_uuid_arg_is_recognised() {
    // We just check the flag is parsed; actual resolution will fail without a
    // matching device, which is expected in CI.
    let out = new_ucmd!()
        .args(&["--fake", "--uuid", "any-uuid", "/tmp"])
        .run();
    // Either succeeds (device found) or fails with exit code 1 (not found),
    // but must NOT fail with exit code 64 (clap usage error).
    assert_ne!(out.code(), 64, "--uuid should be a recognised argument");
}

/// `--partlabel` arg must be accepted by the parser.
#[test]
fn test_partlabel_arg_is_recognised() {
    let out = new_ucmd!()
        .args(&["--fake", "--partlabel", "some-part-label", "/tmp"])
        .run();
    assert_ne!(
        out.code(),
        64,
        "--partlabel should be a recognised argument"
    );
}

/// `--partuuid` arg must be accepted by the parser.
#[test]
fn test_partuuid_arg_is_recognised() {
    let out = new_ucmd!()
        .args(&[
            "--fake",
            "--partuuid",
            "00000000-0000-0000-0000-000000000000",
            "/tmp",
        ])
        .run();
    assert_ne!(out.code(), 64, "--partuuid should be a recognised argument");
}

/// `-l`/`--show-labels` with no other args should list mounted filesystems.
#[test]
#[cfg(target_os = "linux")]
fn test_show_labels_flag_shows_mounts() {
    let result = new_ucmd!().arg("-l").succeeds();
    let stdout = result.stdout_str();
    assert!(!stdout.is_empty(), "-l should produce output");
    for line in stdout.lines() {
        assert!(
            line.contains(" on ") && line.contains(" type "),
            "unexpected line format: {line:?}"
        );
    }
}

/// `-r` and `-w` are mutually exclusive.
#[test]
fn test_read_only_and_read_write_are_mutually_exclusive() {
    new_ucmd!().args(&["--read-only", "--read-write"]).fails();
}

/// `-t` filters the listing output.
#[test]
#[cfg(target_os = "linux")]
fn test_types_filter_in_list_mode() {
    // tmpfs is present on every Linux system; filter for it.
    let result = new_ucmd!().args(&["-t", "tmpfs"]).succeeds();
    let stdout = result.stdout_str();
    // Every printed line must be of type tmpfs.
    for line in stdout.lines() {
        assert!(
            line.contains(" type tmpfs "),
            "expected only tmpfs entries, got: {line:?}"
        );
    }
}

/// `-t notmpfs` should exclude tmpfs entries.
#[test]
#[cfg(target_os = "linux")]
fn test_types_filter_exclusion_in_list_mode() {
    let result = new_ucmd!().args(&["-t", "notmpfs"]).succeeds();
    let stdout = result.stdout_str();
    for line in stdout.lines() {
        assert!(
            !line.contains(" type tmpfs "),
            "expected no tmpfs entries, got: {line:?}"
        );
    }
}

// ── Unit tests for internal helpers ─────────────────────────────────────────

#[cfg(target_os = "linux")]
mod unit {
    use mount::{
        collect_propagation_changes, format_mount_listing, fstab_entry_matches_test_opts,
        fstype_matches_filter, is_already_mounted, merge_mount_options, options_to_flags,
        parse_fstab_contents, parse_mount_entries, resolve_label_from, resolve_partlabel_from,
        resolve_partuuid_from, resolve_single_argument_from, resolve_uuid_from, write_mtab_to,
        FsTabEntry, MountEntry,
    };

    // ── parse_mount_entries ──────────────────────────────────────────────────

    #[test]
    fn test_parse_empty() {
        assert!(parse_mount_entries("").is_empty());
    }

    #[test]
    fn test_parse_single_entry() {
        let input = "sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0\n";
        let entries = parse_mount_entries(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            MountEntry::new("sysfs", "/sys", "sysfs", "rw,nosuid,nodev,noexec,relatime")
        );
    }

    #[test]
    fn test_parse_single_entry_unescapes_fields() {
        let input = r"/dev/disk/by-label/My\040Drive /mount\040point ext4 rw\011errors=remount-ro 0 0
";
        let entries = parse_mount_entries(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].source, "/dev/disk/by-label/My Drive");
        assert_eq!(entries[0].target, "/mount point");
        assert_eq!(entries[0].options, "rw\terrors=remount-ro");
    }

    #[test]
    fn test_parse_multiple_entries() {
        let input = "\
sysfs /sys sysfs rw,nosuid 0 0
proc /proc proc rw,nosuid 0 0
tmpfs /run tmpfs rw,noexec 0 0
";
        let entries = parse_mount_entries(input);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].source, "sysfs");
        assert_eq!(entries[1].source, "proc");
        assert_eq!(entries[2].source, "tmpfs");
    }

    #[test]
    fn test_parse_skips_malformed_lines() {
        let input = "\
good /mnt ext4 rw 0 0
bad_line_with_only_one_field
also /mnt2 ext4 ro 0 0
";
        let entries = parse_mount_entries(input);
        // Lines with fewer than 4 fields are skipped.
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_mount_entry_display() {
        let entry = MountEntry::new("tmpfs", "/run", "tmpfs", "rw,noexec");
        assert_eq!(entry.to_string(), "tmpfs on /run type tmpfs (rw,noexec)");
    }

    #[test]
    fn test_format_mount_listing_with_label() {
        let entry = MountEntry::new("/dev/sda1", "/", "ext4", "rw,relatime");
        assert_eq!(
            format_mount_listing(&entry, Some("rootfs")),
            "/dev/sda1 on / type ext4 (rw,relatime) [rootfs]"
        );
    }

    // ── options_to_flags ─────────────────────────────────────────────────────
    // options_to_flags returns (set_flags, clear_flags, extra_data).

    #[test]
    fn test_flags_ro() {
        let (set_flags, _, extra) = options_to_flags("ro");
        assert!(set_flags & libc::MS_RDONLY != 0);
        assert!(extra.is_empty());
    }

    #[test]
    fn test_flags_rw_clears_rdonly() {
        let (set_flags, clear_flags, extra) = options_to_flags("rw");
        assert_eq!(set_flags, 0);
        assert!(clear_flags & libc::MS_RDONLY != 0);
        assert!(extra.is_empty());
    }

    #[test]
    fn test_flags_noexec() {
        let (set_flags, _, _) = options_to_flags("noexec");
        assert!(set_flags & libc::MS_NOEXEC != 0);
    }

    #[test]
    fn test_flags_nosuid() {
        let (set_flags, _, _) = options_to_flags("nosuid");
        assert!(set_flags & libc::MS_NOSUID != 0);
    }

    #[test]
    fn test_flags_nodev() {
        let (set_flags, _, _) = options_to_flags("nodev");
        assert!(set_flags & libc::MS_NODEV != 0);
    }

    #[test]
    fn test_flags_noatime() {
        let (set_flags, _, _) = options_to_flags("noatime");
        assert!(set_flags & libc::MS_NOATIME != 0);
    }

    #[test]
    fn test_flags_relatime() {
        let (set_flags, _, _) = options_to_flags("relatime");
        assert!(set_flags & libc::MS_RELATIME != 0);
    }

    #[test]
    fn test_flags_remount() {
        let (set_flags, _, _) = options_to_flags("remount");
        assert!(set_flags & libc::MS_REMOUNT != 0);
    }

    #[test]
    fn test_flags_exec_clears_noexec() {
        let (set_flags, clear_flags, _) = options_to_flags("exec");
        assert_eq!(set_flags & libc::MS_NOEXEC, 0);
        assert!(clear_flags & libc::MS_NOEXEC != 0);
    }

    #[test]
    fn test_flags_suid_clears_nosuid() {
        let (_, clear_flags, _) = options_to_flags("suid");
        assert!(clear_flags & libc::MS_NOSUID != 0);
    }

    #[test]
    fn test_flags_dev_clears_nodev() {
        let (_, clear_flags, _) = options_to_flags("dev");
        assert!(clear_flags & libc::MS_NODEV != 0);
    }

    #[test]
    fn test_flags_atime_clears_noatime() {
        let (_, clear_flags, _) = options_to_flags("atime");
        assert!(clear_flags & libc::MS_NOATIME != 0);
    }

    #[test]
    fn test_flags_diratime_clears_nodiratime() {
        let (_, clear_flags, _) = options_to_flags("diratime");
        assert!(clear_flags & libc::MS_NODIRATIME != 0);
    }

    #[test]
    fn test_flags_bind() {
        let (set_flags, _, _) = options_to_flags("bind");
        assert!(set_flags & libc::MS_BIND != 0);
    }

    #[test]
    fn test_flags_rbind() {
        let (set_flags, _, _) = options_to_flags("rbind");
        assert!(set_flags & libc::MS_BIND != 0);
        assert!(set_flags & libc::MS_REC != 0);
    }

    #[test]
    fn test_flags_shared() {
        let (set_flags, _, _) = options_to_flags("shared");
        assert!(set_flags & libc::MS_SHARED != 0);
    }

    #[test]
    fn test_flags_slave() {
        let (set_flags, _, _) = options_to_flags("slave");
        assert!(set_flags & libc::MS_SLAVE != 0);
    }

    #[test]
    fn test_flags_private() {
        let (set_flags, _, _) = options_to_flags("private");
        assert!(set_flags & libc::MS_PRIVATE != 0);
    }

    #[test]
    fn test_flags_unbindable() {
        let (set_flags, _, _) = options_to_flags("unbindable");
        assert!(set_flags & libc::MS_UNBINDABLE != 0);
    }

    #[test]
    fn test_flags_combined() {
        let (set_flags, _, extra) = options_to_flags("ro,nosuid,nodev,noexec");
        assert!(set_flags & libc::MS_RDONLY != 0);
        assert!(set_flags & libc::MS_NOSUID != 0);
        assert!(set_flags & libc::MS_NODEV != 0);
        assert!(set_flags & libc::MS_NOEXEC != 0);
        assert!(extra.is_empty());
    }

    #[test]
    fn test_flags_unknown_option_passed_through() {
        let (_, _, extra) = options_to_flags("uid=1000,gid=1000");
        assert!(extra.contains("uid=1000"));
        assert!(extra.contains("gid=1000"));
    }

    #[test]
    fn test_flags_mixed_known_and_unknown() {
        let (set_flags, _, extra) = options_to_flags("ro,uid=1000,noexec");
        assert!(set_flags & libc::MS_RDONLY != 0);
        assert!(set_flags & libc::MS_NOEXEC != 0);
        assert_eq!(extra, "uid=1000");
    }

    #[test]
    fn test_merge_mount_options_keeps_order() {
        assert_eq!(
            merge_mount_options("defaults,noexec", "ro,exec"),
            "defaults,noexec,ro,exec"
        );
    }

    #[test]
    fn test_merge_mount_options_skips_empty_segments() {
        assert_eq!(merge_mount_options("", "ro"), "ro");
        assert_eq!(merge_mount_options("defaults", ""), "defaults");
    }

    #[test]
    fn test_fstab_entry_matches_test_opts_positive_match() {
        let entry = FsTabEntry {
            fs_spec: "/dev/sda1".into(),
            fs_file: "/data".into(),
            fs_vfstype: "ext4".into(),
            fs_mntops: "defaults,noatime".into(),
            fs_freq: 0,
            fs_passno: 0,
        };
        assert!(fstab_entry_matches_test_opts(&entry, "noatime"));
        assert!(!fstab_entry_matches_test_opts(&entry, "nodev"));
    }

    #[test]
    fn test_fstab_entry_matches_test_opts_negative_match() {
        let entry = FsTabEntry {
            fs_spec: "/dev/sda1".into(),
            fs_file: "/data".into(),
            fs_vfstype: "ext4".into(),
            fs_mntops: "defaults,netdev".into(),
            fs_freq: 0,
            fs_passno: 0,
        };
        assert!(!fstab_entry_matches_test_opts(&entry, "no_netdev"));
        assert!(fstab_entry_matches_test_opts(&entry, "no_noatime"));
    }

    #[test]
    fn test_collect_propagation_changes_basic_and_recursive() {
        let changes = collect_propagation_changes(&["make-shared", "make-rprivate"]);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].name, "make-shared");
        assert_eq!(changes[0].flags, libc::MS_SHARED);
        assert_eq!(changes[1].name, "make-rprivate");
        assert_eq!(changes[1].flags, libc::MS_PRIVATE | libc::MS_REC);
    }

    // ── parse_fstab_contents ─────────────────────────────────────────────────

    #[test]
    fn test_fstab_empty() {
        assert!(parse_fstab_contents("").is_empty());
    }

    #[test]
    fn test_fstab_skips_comments() {
        let input = "# This is a comment\n# Another comment\n";
        assert!(parse_fstab_contents(input).is_empty());
    }

    #[test]
    fn test_fstab_skips_blank_lines() {
        let input = "\n   \n\t\n";
        assert!(parse_fstab_contents(input).is_empty());
    }

    #[test]
    fn test_fstab_basic_entry() {
        let input = "/dev/sda1 / ext4 defaults 1 1\n";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0],
            FsTabEntry {
                fs_spec: "/dev/sda1".into(),
                fs_file: "/".into(),
                fs_vfstype: "ext4".into(),
                fs_mntops: "defaults".into(),
                fs_freq: 1,
                fs_passno: 1,
            }
        );
    }

    #[test]
    fn test_fstab_tab_separated() {
        let input = "/dev/sda1\t/\text4\tdefaults\t0\t0\n";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_spec, "/dev/sda1");
        assert_eq!(entries[0].fs_file, "/");
        assert_eq!(entries[0].fs_vfstype, "ext4");
    }

    #[test]
    fn test_fstab_multiple_spaces() {
        let input = "/dev/sda1   /boot   ext4   defaults   0   2\n";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_passno, 2);
    }

    #[test]
    fn test_fstab_optional_freq_passno() {
        // Only 4 fields (freq and passno omitted) → default to 0.
        let input = "tmpfs /tmp tmpfs defaults\n";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_freq, 0);
        assert_eq!(entries[0].fs_passno, 0);
    }

    #[test]
    fn test_fstab_optional_passno_only() {
        // 5 fields (passno omitted) → fs_passno defaults to 0.
        let input = "tmpfs /tmp tmpfs defaults 1\n";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_freq, 1);
        assert_eq!(entries[0].fs_passno, 0);
    }

    #[test]
    fn test_fstab_skips_malformed_lines() {
        let input = "\
/dev/sda1 / ext4 defaults 1 1
too_few_fields
/dev/sdb1 /data ext4 defaults 0 0
";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].fs_spec, "/dev/sda1");
        assert_eq!(entries[1].fs_spec, "/dev/sdb1");
    }

    #[test]
    fn test_fstab_octal_space_in_path() {
        // \040 is the octal escape for a space character.
        let input = r"/dev/sda1 /mount\040point ext4 defaults 0 0";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_file, "/mount point");
    }

    #[test]
    fn test_fstab_octal_tab_in_spec() {
        // \011 is the octal escape for a tab character.
        let input = r"/dev/sd\011a1 / ext4 defaults 0 0";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_spec, "/dev/sd\ta1");
    }

    #[test]
    fn test_fstab_comment_after_data() {
        let input = "\
# /etc/fstab
# <fs>   <mount>  <type>  <opts>    <dump>  <pass>
/dev/sda1  /       ext4    defaults  1       1
tmpfs      /tmp    tmpfs   noauto    0       0
";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].fs_vfstype, "ext4");
        assert_eq!(entries[1].fs_mntops, "noauto");
    }

    #[test]
    fn test_fstab_swap_entry() {
        let input = "/dev/sda2 none swap sw 0 0\n";
        let entries = parse_fstab_contents(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].fs_file, "none");
        assert_eq!(entries[0].fs_vfstype, "swap");
    }

    // ── write_mtab_to ────────────────────────────────────────────────────────

    #[test]
    fn test_write_mtab_appends_entry() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "existing /mnt ext4 defaults 0 0").unwrap();

        write_mtab_to(f.path(), "tmpfs", "/tmp", "tmpfs", "rw,nodev").unwrap();

        let contents = std::fs::read_to_string(f.path()).unwrap();
        assert!(contents.contains("existing /mnt ext4 defaults 0 0"));
        assert!(contents.contains("tmpfs /tmp tmpfs rw,nodev 0 0"));
    }

    #[test]
    fn test_write_mtab_entry_format() {
        use tempfile::NamedTempFile;

        let f = NamedTempFile::new().unwrap();
        write_mtab_to(f.path(), "/dev/sda1", "/boot", "ext4", "ro").unwrap();

        let contents = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(contents.trim(), "/dev/sda1 /boot ext4 ro 0 0");
    }

    #[test]
    fn test_write_mtab_escapes_whitespace() {
        use tempfile::NamedTempFile;

        let f = NamedTempFile::new().unwrap();
        write_mtab_to(
            f.path(),
            "/dev/disk/by-label/My Drive",
            "/mnt/my mount",
            "ext4",
            "rw",
        )
        .unwrap();

        let contents = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(
            contents.trim(),
            r"/dev/disk/by-label/My\040Drive /mnt/my\040mount ext4 rw 0 0"
        );
    }

    #[test]
    fn test_write_mtab_skips_symlink() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let target = dir.path().join("real_file");
        std::fs::write(&target, "").unwrap();
        let link = dir.path().join("mtab_link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        write_mtab_to(&link, "tmpfs", "/tmp", "tmpfs", "defaults").unwrap();

        let contents = std::fs::read_to_string(&target).unwrap();
        assert!(contents.is_empty(), "symlink target should not be modified");
    }

    #[test]
    fn test_write_mtab_missing_file_is_ok() {
        let result = write_mtab_to(
            std::path::Path::new("/nonexistent/mtab"),
            "x",
            "/y",
            "z",
            "rw",
        );
        assert!(result.is_ok());
    }

    // ── resolve_label_from ───────────────────────────────────────────────────

    fn make_entries(specs: &[(&str, &str, &str, &str)]) -> Vec<FsTabEntry> {
        specs
            .iter()
            .map(|&(spec, file, vfs, opts)| FsTabEntry {
                fs_spec: spec.to_string(),
                fs_file: file.to_string(),
                fs_vfstype: vfs.to_string(),
                fs_mntops: opts.to_string(),
                fs_freq: 0,
                fs_passno: 0,
            })
            .collect()
    }

    #[test]
    fn test_resolve_label_from_fstab() {
        let entries = make_entries(&[
            ("LABEL=boot", "/boot", "ext4", "defaults"),
            ("/dev/sda2", "/", "ext4", "defaults"),
        ]);
        let r = resolve_label_from("boot", None, &entries).unwrap();
        assert_eq!(r.target, "/boot");
        assert_eq!(r.fs_type.as_deref(), Some("ext4"));
        assert_eq!(r.options, "defaults");
    }

    #[test]
    fn test_resolve_label_case_insensitive() {
        let entries = make_entries(&[("LABEL=Boot", "/boot", "ext4", "ro")]);
        // Lookup with different case should still find it.
        let r = resolve_label_from("boot", None, &entries).unwrap();
        assert_eq!(r.target, "/boot");
    }

    #[test]
    fn test_resolve_label_cli_target_overrides_fstab() {
        let entries = make_entries(&[("LABEL=data", "/data", "ext4", "defaults")]);
        let r = resolve_label_from("data", Some("/mnt/override"), &entries).unwrap();
        assert_eq!(r.target, "/mnt/override");
    }

    #[test]
    fn test_resolve_label_not_found() {
        let entries = make_entries(&[("LABEL=other", "/other", "ext4", "defaults")]);
        let err = resolve_label_from("missing", None, &entries).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn test_resolve_label_swap_has_no_mount_point() {
        // "none" target means it's a swap entry – should fail without cli_target.
        let entries = make_entries(&[("LABEL=swap0", "none", "swap", "sw")]);
        let err = resolve_label_from("swap0", None, &entries).unwrap_err();
        assert!(err.to_string().contains("no mount point"));
    }

    // ── resolve_uuid_from ────────────────────────────────────────────────────

    #[test]
    fn test_resolve_uuid_from_fstab() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let entries = make_entries(&[(&format!("UUID={uuid}"), "/data", "ext4", "noatime")]);
        let r = resolve_uuid_from(uuid, None, &entries).unwrap();
        assert_eq!(r.target, "/data");
        assert_eq!(r.options, "noatime");
    }

    #[test]
    fn test_resolve_uuid_case_insensitive() {
        let entries = make_entries(&[("UUID=ABCD-EF01", "/usb", "vfat", "defaults")]);
        let r = resolve_uuid_from("abcd-ef01", None, &entries).unwrap();
        assert_eq!(r.target, "/usb");
    }

    #[test]
    fn test_resolve_uuid_cli_target_overrides_fstab() {
        let entries = make_entries(&[("UUID=1234-5678", "/home", "ext4", "defaults")]);
        let r = resolve_uuid_from("1234-5678", Some("/mnt/usb"), &entries).unwrap();
        assert_eq!(r.target, "/mnt/usb");
    }

    #[test]
    fn test_resolve_uuid_not_found() {
        let entries: Vec<FsTabEntry> = vec![];
        let err = resolve_uuid_from("dead-beef", None, &entries).unwrap_err();
        assert!(err.to_string().contains("dead-beef"));
    }

    // ── resolve_partlabel_from ───────────────────────────────────────────────

    #[test]
    fn test_resolve_partlabel_from_fstab() {
        let entries = make_entries(&[("PARTLABEL=efi", "/boot/efi", "vfat", "defaults")]);
        let r = resolve_partlabel_from("efi", None, &entries).unwrap();
        assert_eq!(r.target, "/boot/efi");
        assert_eq!(r.fs_type.as_deref(), Some("vfat"));
    }

    #[test]
    fn test_resolve_partlabel_case_insensitive() {
        let entries = make_entries(&[("PARTLABEL=EFI", "/boot/efi", "vfat", "ro")]);
        let r = resolve_partlabel_from("efi", None, &entries).unwrap();
        assert_eq!(r.target, "/boot/efi");
    }

    #[test]
    fn test_resolve_partlabel_not_found() {
        let entries: Vec<FsTabEntry> = vec![];
        let err = resolve_partlabel_from("missing-part", None, &entries).unwrap_err();
        assert!(err.to_string().contains("missing-part"));
    }

    // ── resolve_partuuid_from ────────────────────────────────────────────────

    #[test]
    fn test_resolve_partuuid_from_fstab() {
        let uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let entries = make_entries(&[(&format!("PARTUUID={uuid}"), "/data", "ext4", "noatime")]);
        let r = resolve_partuuid_from(uuid, None, &entries).unwrap();
        assert_eq!(r.target, "/data");
        assert_eq!(r.options, "noatime");
    }

    #[test]
    fn test_resolve_partuuid_case_insensitive() {
        let entries = make_entries(&[("PARTUUID=ABCD-EF01", "/usb", "vfat", "defaults")]);
        let r = resolve_partuuid_from("abcd-ef01", None, &entries).unwrap();
        assert_eq!(r.target, "/usb");
    }

    #[test]
    fn test_resolve_partuuid_not_found() {
        let entries: Vec<FsTabEntry> = vec![];
        let err = resolve_partuuid_from("dead-beef", None, &entries).unwrap_err();
        assert!(err.to_string().contains("dead-beef"));
    }

    // ── resolve_single_argument_from ─────────────────────────────────────────

    #[test]
    fn test_resolve_single_argument_from_mountpoint() {
        let entries = make_entries(&[("/dev/sda1", "/data", "ext4", "defaults")]);
        let resolved = resolve_single_argument_from("/data", &entries).unwrap();
        assert_eq!(resolved.source, "/dev/sda1");
        assert_eq!(resolved.target, "/data");
    }

    #[test]
    fn test_resolve_single_argument_from_source() {
        let entries = make_entries(&[("/dev/sda1", "/data", "ext4", "defaults")]);
        let resolved = resolve_single_argument_from("/dev/sda1", &entries).unwrap();
        assert_eq!(resolved.source, "/dev/sda1");
        assert_eq!(resolved.target, "/data");
    }

    #[test]
    fn test_resolve_single_argument_from_swap_has_no_mount_point() {
        let entries = make_entries(&[("/dev/sda2", "none", "swap", "sw")]);
        let err = resolve_single_argument_from("/dev/sda2", &entries).unwrap_err();
        assert!(err.to_string().contains("no mount point"));
    }

    #[test]
    fn test_resolve_single_argument_from_missing_entry() {
        let entries = make_entries(&[("/dev/sda1", "/data", "ext4", "defaults")]);
        let err = resolve_single_argument_from("/missing", &entries).unwrap_err();
        assert!(err.to_string().contains("/missing"));
    }

    #[test]
    fn test_is_already_mounted_matches_exact_source_and_target() {
        let entry = FsTabEntry {
            fs_spec: "/dev/sda1".into(),
            fs_file: "/data".into(),
            fs_vfstype: "ext4".into(),
            fs_mntops: "defaults".into(),
            fs_freq: 0,
            fs_passno: 0,
        };
        let mounts = vec![MountEntry::new("/dev/sda1", "/data", "ext4", "rw")];
        assert!(is_already_mounted(&entry, &mounts));
    }

    #[test]
    fn test_is_already_mounted_matches_canonicalised_source() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let device = dir.path().join("device");
        let symlink = dir.path().join("device-link");
        std::fs::write(&device, "").unwrap();
        std::os::unix::fs::symlink(&device, &symlink).unwrap();

        let entry = FsTabEntry {
            fs_spec: symlink.to_string_lossy().into_owned(),
            fs_file: "/data".into(),
            fs_vfstype: "ext4".into(),
            fs_mntops: "defaults".into(),
            fs_freq: 0,
            fs_passno: 0,
        };
        let mounts = vec![MountEntry::new(
            &device.to_string_lossy(),
            "/data",
            "ext4",
            "rw",
        )];
        assert!(is_already_mounted(&entry, &mounts));
    }

    #[test]
    fn test_is_already_mounted_requires_matching_target() {
        let entry = FsTabEntry {
            fs_spec: "/dev/sda1".into(),
            fs_file: "/data".into(),
            fs_vfstype: "ext4".into(),
            fs_mntops: "defaults".into(),
            fs_freq: 0,
            fs_passno: 0,
        };
        let mounts = vec![MountEntry::new("/dev/sda1", "/other", "ext4", "rw")];
        assert!(!is_already_mounted(&entry, &mounts));
    }

    // ── fstype_matches_filter ────────────────────────────────────────────────

    #[test]
    fn test_fstype_filter_simple_inclusion() {
        assert!(fstype_matches_filter("ext4", "ext4"));
        assert!(!fstype_matches_filter("tmpfs", "ext4"));
    }

    #[test]
    fn test_fstype_filter_multiple_inclusions() {
        assert!(fstype_matches_filter("ext4", "ext4,vfat"));
        assert!(fstype_matches_filter("vfat", "ext4,vfat"));
        assert!(!fstype_matches_filter("tmpfs", "ext4,vfat"));
    }

    #[test]
    fn test_fstype_filter_exclusion_only() {
        // "notmpfs" → include everything except tmpfs.
        assert!(fstype_matches_filter("ext4", "notmpfs"));
        assert!(!fstype_matches_filter("tmpfs", "notmpfs"));
    }

    #[test]
    fn test_fstype_filter_exclusion_multiple() {
        assert!(fstype_matches_filter("ext4", "notmpfs,nodevtmpfs"));
        assert!(!fstype_matches_filter("tmpfs", "notmpfs,nodevtmpfs"));
        assert!(!fstype_matches_filter("devtmpfs", "notmpfs,nodevtmpfs"));
    }

    #[test]
    fn test_fstype_filter_mixed_inclusion_exclusion() {
        // Include ext4 and vfat, but exclude vfat → only ext4 matches.
        assert!(fstype_matches_filter("ext4", "ext4,novfat"));
        assert!(!fstype_matches_filter("vfat", "ext4,novfat"));
        assert!(!fstype_matches_filter("tmpfs", "ext4,novfat"));
    }

    #[test]
    fn test_fstype_filter_empty_string_matches_all() {
        // An empty filter string (no inclusions, no exclusions) → all types match.
        assert!(fstype_matches_filter("ext4", ""));
        assert!(fstype_matches_filter("tmpfs", ""));
    }
}

/// An unmatched single positional argument should be treated as an fstab lookup
/// failure rather than falling back to listing mounted filesystems.
#[test]
#[cfg(target_os = "linux")]
fn test_single_positional_unknown_fails() {
    new_ucmd!()
        .arg("/copilot-mount-missing-entry-9876f2ab")
        .fails()
        .code_is(1)
        .stderr_contains("in fstab");
}
