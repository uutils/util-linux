// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) ioprio pgrp

use std::ffi::{c_int, c_long};
use std::fmt;
use std::io;

pub(crate) const IOPRIO_CLASS_NONE: i32 = 0;
pub(crate) const IOPRIO_CLASS_RT: i32 = 1;
pub(crate) const IOPRIO_CLASS_BE: i32 = 2;
pub(crate) const IOPRIO_CLASS_IDLE: i32 = 3;

/// The scheduling class names, accepted by -c and printed back; the numbers
/// are the kernel's.
pub(crate) const CLASSES: [(&str, i32); 4] = [
    ("none", IOPRIO_CLASS_NONE),
    ("realtime", IOPRIO_CLASS_RT),
    ("best-effort", IOPRIO_CLASS_BE),
    ("idle", IOPRIO_CLASS_IDLE),
];

/// The kernel packs a scheduling class above this many bits of priority data.
const IOPRIO_CLASS_SHIFT: u32 = 13;
const IOPRIO_PRIO_MASK: i32 = (1 << IOPRIO_CLASS_SHIFT) - 1;
const IOPRIO_CLASS_MASK: i32 = 0x7;

/// Whom an ioprio call applies to. libc exposes no IOPRIO_WHO_* constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum Who {
    Process = 1,
    Pgrp = 2,
    User = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IoPrio(i32);

impl IoPrio {
    /// Pack a class and a priority level. Neither is masked, and neither
    /// needs to be: the only classes and levels that reach here are the ones
    /// ionice(1) documents, which occupy their fields exactly.
    pub(crate) fn encode(class: i32, data: i32) -> Self {
        Self((class << IOPRIO_CLASS_SHIFT) | data)
    }

    pub(crate) fn class(self) -> i32 {
        (self.0 >> IOPRIO_CLASS_SHIFT) & IOPRIO_CLASS_MASK
    }

    pub(crate) fn data(self) -> i32 {
        self.0 & IOPRIO_PRIO_MASK
    }
}

impl fmt::Display for IoPrio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = self.class();

        if class == IOPRIO_CLASS_IDLE {
            // An idle priority prints as the bare class name; the level it
            // carries is not shown.
            f.write_str(class_name(class))
        } else {
            write!(f, "{}: prio {}", class_name(class), self.data())
        }
    }
}

/// The fallback is unreachable - the kernel constrains a get to classes 0
/// through 3, and the one warning that names a class is raised only for none
/// or idle - but it keeps this from having to panic.
pub(crate) fn class_name(class: i32) -> &'static str {
    CLASSES
        .iter()
        .find(|&&(_, value)| value == class)
        .map_or("unknown", |&(name, _)| name)
}

pub(crate) fn get(who: Who, who_id: i32) -> io::Result<IoPrio> {
    // SAFETY: ioprio_get takes two integers by value and touches no memory.
    // c_long::from avoids a cast, because the syscall number is c_int on a few
    // targets and c_long on the rest.
    let result = unsafe {
        libc::syscall(
            c_long::from(libc::SYS_ioprio_get),
            c_long::from(who as i32),
            c_long::from(who_id),
        )
    };

    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(IoPrio(result as c_int))
    }
}

pub(crate) fn set(who: Who, who_id: i32, priority: IoPrio) -> io::Result<()> {
    // SAFETY: ioprio_set takes three integers by value and touches no memory.
    let result = unsafe {
        libc::syscall(
            c_long::from(libc::SYS_ioprio_set),
            c_long::from(who as i32),
            c_long::from(who_id),
            c_long::from(priority.0),
        )
    };

    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
