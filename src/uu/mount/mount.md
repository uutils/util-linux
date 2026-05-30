# mount

```
mount [options]
mount [options] <source> <directory>
mount [options] <source> | <directory>
mount [options] --source <source> [--target <directory>]
mount [options] [--source <source>] --target <directory>
mount [options] --make-{shared,slave,private,unbindable} <mountpoint>
mount [options] --make-{rshared,rslave,rprivate,runbindable} <mountpoint>
```

Mount a filesystem, or show all currently mounted filesystems.

When called without arguments, or with `-t` but no source and target, mount
prints all currently mounted filesystems (read from `/proc/mounts` on Linux).
The `-t` option filters the listing to entries of the given type. `-l` requests
labels in listing output when they can be discovered from `/dev/disk/by-label`.

When called with a source device and a target directory, mount attaches the
filesystem found on the source to the target directory.

When called with a single positional argument, mount resolves that argument via
`/etc/fstab`. The argument may be either a source specifier or a mount point.
`--source` and `--target` can be used to disambiguate that lookup.

When called with `--make-*`, mount applies mount-propagation changes to the
specified mountpoint. If a normal mount is also requested in the same command,
the propagation changes are applied after the mount succeeds.

## Supported contract

This implementation currently targets **Linux** and is intended to provide a
strong baseline `mount` with the core operational semantics in place:

- no-argument listing from `/proc/mounts`
- direct mounts plus `--bind`, `--rbind`, and `--move`
- `--all` with already-mounted skipping, `-t` filtering, `-O` filtering, and
  optional `--fork`
- `/etc/fstab`-driven single-argument resolution, including alternate files via
  `-T`
- label/UUID/PARTLABEL/PARTUUID resolution
- merged `fstab` + CLI `-o` options for fstab-derived mounts
- optional mountpoint creation via `-m`
- propagation changes via `--make-*`

This command is **not yet a full upstream-compatible replacement** for every
advanced `mount(8)` feature. The supported behavior is intentionally explicit so
reviewers can evaluate the current contract clearly.

## Options

- `-a`, `--all` — mount all filesystems listed in `/etc/fstab` (respects `noauto`
  and `-t` / `-O` filters)
- `-B`, `--bind` — bind-mount a subtree at another location (`MS_BIND`)
- `-R`, `--rbind` — recursively bind-mount a subtree (`MS_BIND | MS_REC`)
- `-M`, `--move` — atomically move a mounted subtree to a new location
  (`MS_MOVE`)
- `--make-shared` — mark a subtree as shared
- `--make-slave` — mark a subtree as slave
- `--make-private` — mark a subtree as private
- `--make-unbindable` — mark a subtree as unbindable
- `--make-rshared` — recursively mark a whole subtree as shared
- `--make-rslave` — recursively mark a whole subtree as slave
- `--make-rprivate` — recursively mark a whole subtree as private
- `--make-runbindable` — recursively mark a whole subtree as unbindable
- `-f`, `--fake` — dry run; parse arguments and resolve devices but skip the
  actual `mount(2)` syscall
- `-F`, `--fork` — with `--all`, mount matching filesystems in separate worker
  processes
- `-T`, `--fstab PATH` — use an alternate fstab file instead of `/etc/fstab`
- `-l`, `--show-labels` — show filesystem labels in listing output when
  available
- `-m`, `--mkdir` — create the target mountpoint if it does not already exist
- `-n`, `--no-mtab` — do not write an entry to `/etc/mtab`
- `-o`, `--options LIST` — comma-separated list of mount options (e.g.
  `ro,noatime,uid=1000`); for `/etc/fstab`-resolved mounts, CLI options are
  appended after `fstab` options so later values win
- `-O`, `--test-opts LIST` — with `--all`, limit mounts to fstab entries whose
  option field matches `LIST`
- `-r`, `--read-only` — mount read-only (same as `-o ro`)
- `-w`, `--read-write` — mount read-write, overriding a `ro` option from fstab
- `-t`, `--types LIST` — filesystem type filter; prefix a type with `no` to
  exclude it (e.g. `-t noext4`)
- `-v`, `--verbose` — print a diagnostic line for each mount operation
- `-L`, `--label LABEL` — mount the device with the given filesystem label
- `-U`, `--uuid UUID` — mount the device with the given filesystem UUID
- `--partlabel LABEL` — mount the partition with the given partition label
  (`PARTLABEL=`)
- `--partuuid UUID` — mount the partition with the given partition UUID
  (`PARTUUID=`)
- `--source SOURCE` — explicitly specify the source side of the mount or the
  single-argument fstab lookup key
- `--target DIRECTORY` — explicitly specify the target side of the mount or the
  single-argument fstab lookup key

## Notes

- `--make-*` propagation operations are not combined with `--all`.
- Propagation changes do not read `/etc/fstab`; provide the target mountpoint
  explicitly when using them directly.

## Deferred features

Notable items that remain outside the current supported contract include:

- alternate `--options-mode` handling beyond the current append-style merge
- helper-specific behaviors outside this in-process Linux implementation
- additional advanced `mount(8)` compatibility options not yet implemented
