// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) getattr setattr setscheduler sched getpriority

//! Every call goes through `syscall(2)`: the libc crate binds no `sched_*attr`
//! wrapper, and musl's `sched_setscheduler` fails without making the call.

use std::fs;
use std::io;
use std::mem::size_of;
use std::path::PathBuf;

/// The reset-on-fork request typed for `sched_flags`; `sched_setscheduler(2)`
/// takes it as `SCHED_RESET_ON_FORK` or-ed into the policy.
pub(crate) const SCHED_FLAG_RESET_ON_FORK: u64 = libc::SCHED_FLAG_RESET_ON_FORK as u64;

/// The struct's first version, which every kernel with these calls accepts.
const SCHED_ATTR_SIZE: u32 = size_of::<libc::sched_attr>() as u32;

pub(crate) fn zeroed_attr() -> libc::sched_attr {
    libc::sched_attr {
        size: 0,
        sched_policy: 0,
        sched_flags: 0,
        sched_nice: 0,
        sched_priority: 0,
        sched_runtime: 0,
        sched_deadline: 0,
        sched_period: 0,
    }
}

pub(crate) fn get_attr(pid: i32) -> io::Result<libc::sched_attr> {
    let mut attr = zeroed_attr();
    // SAFETY: the kernel writes at most `SCHED_ATTR_SIZE` bytes into `attr`,
    // which is exactly that large, and reads nothing else.
    let result = unsafe {
        libc::syscall(
            libc::SYS_sched_getattr,
            pid,
            std::ptr::from_mut(&mut attr),
            SCHED_ATTR_SIZE,
            0u32,
        )
    };
    checked(result)?;
    Ok(attr)
}

pub(crate) fn set_attr(pid: i32, mut attr: libc::sched_attr) -> io::Result<()> {
    attr.size = SCHED_ATTR_SIZE;
    // SAFETY: the kernel reads `attr.size` bytes, exactly the struct, and on
    // E2BIG writes the size it supports back into `size`.
    let result = unsafe {
        libc::syscall(
            libc::SYS_sched_setattr,
            pid,
            std::ptr::from_mut(&mut attr),
            0u32,
        )
    };
    checked(result)?;
    Ok(())
}

/// The raw `getpriority(2)` answers `20 - nice`, never negative, so its -1 is
/// always an error; the libc wrapper's -1 is also a valid nice value.
pub(crate) fn nice(pid: i32) -> io::Result<i32> {
    // SAFETY: takes two integers and touches no memory.
    let result = unsafe { libc::syscall(libc::SYS_getpriority, libc::PRIO_PROCESS, pid) };
    Ok(20 - checked(result)? as i32)
}

/// `struct sched_param` as the kernel reads it: the priority and nothing
/// else. libc's type carries more fields on musl.
#[repr(C)]
struct SchedParam {
    sched_priority: libc::c_int,
}

/// Leaves the nice value alone.
pub(crate) fn set_scheduler(pid: i32, policy: libc::c_int, priority: i32) -> io::Result<()> {
    let param = SchedParam {
        sched_priority: priority,
    };
    // SAFETY: `param` outlives the call and the kernel only reads it.
    let result = unsafe {
        libc::syscall(
            libc::SYS_sched_setscheduler,
            pid,
            policy,
            std::ptr::from_ref(&param),
        )
    };
    checked(result)?;
    Ok(())
}

fn checked(result: libc::c_long) -> io::Result<libc::c_long> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

/// Every thread of the process a pid or tid is in, the leader first, in
/// /proc order (not tid order) and read as walked: a new thread may turn up.
pub(crate) fn tasks(pid: i32) -> io::Result<impl Iterator<Item = io::Result<i32>>> {
    let directory: PathBuf = ["/proc", &pid.to_string(), "task"].iter().collect();
    // Anything that is not a number is not a task; the directory holds
    // nothing else, so this only guards against a surprise.
    Ok(fs::read_dir(directory)?.filter_map(|entry| match entry {
        Ok(entry) => entry.file_name().to_str()?.parse().ok().map(Ok),
        Err(error) => Some(Err(error)),
    }))
}
