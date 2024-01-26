# util-linux

This projects aims at doing the same as https://github.com/uutils/coreutils for util-linux.
We are rewriting these tools in Rust as dropped-in replacements.

Currently, we have skeletons for:
* /usr/bin/lscpu: Displays information about the CPU architecture.
* /bin/mountpoint: Checks if a directory is a mountpoint.


First, reimplement the most important tools from util-linux

* /bin/su: Switches to another user account.
* /sbin/blkid: Locates or prints block device attributes.
* /sbin/fsck: Checks and repairs a Linux file system.
* /sbin/mkfs: Builds a Linux file system on a device, usually used for formatting.
* /sbin/mkswap: Sets up a Linux swap area.
* /usr/bin/getopt: Parses command options, essential for scripting.
* /usr/bin/lsblk: Lists information about all available or specified block devices.
* /bin/dmesg: Displays or controls the kernel ring buffer, used for viewing system message logs.

Second, the long list:
* /bin/findmnt: Lists all mounted filesystems or searches for a filesystem.
* /bin/more: A file pager, displays text files one screen at a time.
* /sbin/agetty: Manages physical or virtual terminals and is essential for login prompts.
* /sbin/blkdiscard: Securely discards blocks on a device, useful for SSDs and thin-provisioned LUNs.
* /sbin/blkzone: Reports or modifies zoned block device parameters.
* /sbin/blockdev: Performs various block device operations like setting read-only, fetching the size, etc.
* /sbin/chcpu: Configures CPU devices in the Linux kernel.
* /sbin/ctrlaltdel: Sets the function of the Ctrl-Alt-Del combination on the console.
* /sbin/findfs: Finds a filesystem by label or UUID.
* /sbin/fsfreeze: Halts access to a filesystem for a snapshot.
* /sbin/fstrim: Discards unused blocks on a mounted filesystem, useful for SSDs.
* /sbin/isosize: Outputs the length of an iso9660 filesystem.
* /sbin/mkfs.bfs: Creates a BFS filesystem, used primarily in older Linux distributions.
* /sbin/mkfs.cramfs: Creates a compressed ROM file system (cramfs).
* /sbin/mkfs.minix: Creates a MINIX filesystem.
* /sbin/pivot_root: Changes the root file system, used in advanced boot or system recovery scenarios.
* /sbin/runuser: Runs a command with the privileges of a specified user account.
* /sbin/sulogin: Provides a login prompt to a single user shell, especially in system rescue mode.
* /sbin/swaplabel: Provides label and UUID for swap area.
* /sbin/switch_root: Switches to another filesystem as the root of the mount tree.
* /sbin/wipefs: Wipes a signature from a device to make it unrecognizable.
* /sbin/zramctl: Sets up and controls zram devices, which are compressed block devices in RAM.
* /usr/bin/addpart: Adds a partition to the system.
* /usr/bin/choom: Adjusts the OOM-killer score of processes.
* /usr/bin/chrt: Manipulates the real-time attributes of a process.
* /usr/bin/delpart: Deletes a partition from the system.
* /usr/bin/fallocate: Manipulates file space, allowing you to efficiently manage file storage space.
* /usr/bin/flock: Manages file locking which is crucial in scripting and avoiding race conditions.
* /usr/bin/hardlink: Creates a hard link to a file.
* /usr/bin/ionice: Sets or gets the I/O scheduling class and priority for a program.
* /usr/bin/ipcmk: Creates IPC (Inter-process communication) resources.
* /usr/bin/ipcrm: Removes IPC resources.
* /usr/bin/ipcs: Shows IPC resources status.
* /usr/bin/last: Shows a listing of last logged in users.
* /usr/bin/lsipc: Shows information on IPC facilities.
* /usr/bin/lslocks: Lists local system locks.
* /usr/bin/lslogins: Displays information about known users in the system.
* /usr/bin/lsmem: Shows the status of available memory.
* /usr/bin/lsns: Lists namespaces.
* /usr/bin/mcookie: Generates magic cookies for xauth.
* /usr/bin/mesg: Controls the access to your terminal by others.
* /usr/bin/namei: Follows a pathname until a terminal point is found.
* /usr/bin/nsenter: Runs programs in the context of other namespaces.
* /usr/bin/partx: Tells the kernel about the presence and numbering of on-disk partitions.
* /usr/bin/prlimit: Sets or reports process resource limits.
* /usr/bin/rename.ul: Renames files.
* /usr/bin/resizepart: Resizes a partition.
* /usr/bin/rev: Reverses lines of a file or files.
* /usr/bin/setarch: Sets architecture emulation for a new process.
* /usr/bin/setpriv: Runs a program with different Linux privilege settings.
* /usr/bin/setsid: Creates a new session and sets the process group ID.
* /usr/bin/setterm: Sets terminal attributes.
* /usr/bin/taskset: Assigns a process to a specific CPU core.
* /usr/bin/uclampset: Sets or queries the utilization clamping value.
* /usr/bin/unshare: Runs a program with some namespaces unshared from the parent.
* /usr/bin/utmpdump: Dumps the contents of utmp and wtmp files in a readable format.
* /usr/bin/whereis: Locates the binary, source, and manual pages for a command.
* /usr/sbin/chmem: Sets or reports the kernel memory policy.
* /usr/sbin/ldattach: Attaches a line discipline to a serial line.
* /usr/sbin/readprofile: Reads and displays kernel profiling information.
* /usr/sbin/rtcwake: Enters a system sleep state until specified wakeup time.
* /sbin/getty: Manages virtual consoles and serial terminals.
* /usr/bin/i386: Sets the process execution domain to i386, for running 32-bit applications on 64-bit systems.
* /usr/bin/lastb: Shows a listing of last failed login attempts.
* /usr/bin/linux32: Runs a program in a 32-bit environment on a 64-bit kernel.
* /usr/bin/linux64: Runs a program in a 64-bit environment on a 64-bit kernel.
* /usr/bin/x86_64: Sets the process execution domain to x86_64, for running 64-bit applications.

Note:
* /bin/more is already implemented in https://github.com/uutils/coreutils


Project:
http://www.kernel.org/pub/linux/utils/util-linux/
