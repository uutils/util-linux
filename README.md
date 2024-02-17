[![Crates.io](https://img.shields.io/crates/v/util-linux.svg)](https://crates.io/crates/util-linux)
[![Discord](https://img.shields.io/badge/discord-join-7289DA.svg?logo=discord&longCache=true&style=flat)](https://discord.gg/wQVJbvJ)
[![License](http://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/uutils/util-linux/blob/main/LICENSE)
[![dependency status](https://deps.rs/repo/github/uutils/util-linux/status.svg)](https://deps.rs/repo/github/uutils/util-linux)

[![CodeCov](https://codecov.io/gh/uutils/util-linux/branch/master/graph/badge.svg)](https://codecov.io/gh/uutils/util-linux)

# util-linux

This projects aims at doing the same as https://github.com/uutils/coreutils for util-linux.

We are rewriting <a href="http://www.kernel.org/pub/linux/utils/util-linux/">these tools</a> in Rust as dropped-in replacements.


First, reimplement the most important tools from util-linux
## System Information
- dmesg: Displays kernel messages.
- lscpu: Shows CPU architecture information.
  Started
- lsipc: Lists IPC facilities.
- lslocks: Lists system locks.
- lsmem: Lists memory ranges and status.
- lsns: Lists namespaces.

## Hardware Management
- chcpu: Manages CPU state.
- rtcwake: Manages system sleep states.
- zramctl: Manages zram devices.
- wdctl: Shows watchdog status.
- chmem: Manages kernel memory usage.

## Filesystem Tools
- findmnt: Lists mounted filesystems.
- mountpoint: Checks if a directory is a mountpoint.
  Started
- fsck: Checks and repairs filesystems.
- fsfreeze: Freezes/unfreezes filesystems.
- fstrim: Discards unused blocks on filesystems.
- wipefs: Wipes filesystem signatures.

## Partition Management
- blkdiscard: Discards sectors on a device.
- blkid: Identifies block device attributes.
- blkzone: Manages zoned block device parameters.
- blockdev: Performs block device operations.
- mkswap: Sets up swap space.
- swaplabel: Manages swap space labels.
- addpart: Adds a partition.
- delpart: Deletes a partition.
- partx: Manages partition entries.
- resizepart: Resizes a partition.

## Process and Resource Management
- runuser: Runs a shell with different user/group IDs.
- sulogin: Provides single-user mode login.
- chrt: Manages real-time process attributes.
- ionice: Sets process I/O scheduling class/priority.
- kill: Sends signals to processes.
- renice: Alters process priority.
- prlimit: Sets/gets process resource limits.
- taskset: Sets/gets process CPU affinity.
- uclampset: Manages process utilization clamping.

## User and Session Management
- su: Changes user ID or becomes superuser.
- agetty: Manages TTYs for login prompts.
- ctrlaltdel: Configures Ctrl-Alt-Del action.
- pivot_root: Changes the root filesystem.
- switch_root: Switches to a different root filesystem.
- last: Lists last logged-in users.
- lslogins: Displays user information.
- mesg: Controls write access to terminal.
- setsid: Runs a program in a new session.
- setterm: Sets terminal attributes.
- getty: Manages virtual console login prompts.

## Networking and IPC
- ipcmk: Creates IPC resources.
- ipcrm: Removes IPC resources.
- ipcs: Shows IPC facilities status.
- nsenter: Enters different namespaces.

## Utility Tools
- lsblk: Lists block devices.
- more: Pager for file viewing.
- fallocate: Preallocates file space.
- flock: Manages file locks.
- getopt: Parses command options.
- hardlink: Creates hard links.
- mcookie: Generates random numbers.
- namei: Follows a pathname to its endpoint.
- rename.ul: Renames files.
- rev: Reverses lines in a file.
- setarch: Sets architecture emulation.
- setpriv: Runs a program with different privileges.
- unshare: Runs a program with unshared namespaces.
- utmpdump: Dumps UTMP/WTMP files.
- whereis: Locates binaries, sources, and manuals.
- ldattach: Attaches line discipline to a serial line.
- readprofile: Reads kernel profiling info.
- i386, linux32, linux64, x86_64: Set personality flags for execution environment.

Note:
* /bin/more is already implemented in https://github.com/uutils/coreutils

Project:
http://www.kernel.org/pub/linux/utils/util-linux/

## Installation

Ensure you have Rust installed on your system. You can install Rust through [rustup](https://rustup.rs/).

Clone the repository and build the project using Cargo:

```bash
git clone https://github.com/uutils/util-linux.git
cd util-linux
cargo build --release
cargo run --release
```

## License

util-linux is licensed under the MIT License - see the `LICENSE` file for details
