// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) sched EINVAL

/// SCHED_EXT, new in Linux 6.12; the libc crate does not name it.
const SCHED_EXT: libc::c_int = 7;

/// A scheduling policy, in the order chrt(1) lists them under --max.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Policy {
    Other,
    Fifo,
    Rr,
    Batch,
    Idle,
    Deadline,
    Ext,
}

impl Policy {
    pub(crate) const ALL: [Policy; 7] = [
        Policy::Other,
        Policy::Fifo,
        Policy::Rr,
        Policy::Batch,
        Policy::Idle,
        Policy::Deadline,
        Policy::Ext,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Other => "SCHED_OTHER",
            Self::Fifo => "SCHED_FIFO",
            Self::Rr => "SCHED_RR",
            Self::Batch => "SCHED_BATCH",
            Self::Idle => "SCHED_IDLE",
            Self::Deadline => "SCHED_DEADLINE",
            Self::Ext => "SCHED_EXT",
        }
    }

    pub(crate) fn raw(self) -> libc::c_int {
        match self {
            Self::Other => libc::SCHED_OTHER,
            Self::Fifo => libc::SCHED_FIFO,
            Self::Rr => libc::SCHED_RR,
            Self::Batch => libc::SCHED_BATCH,
            Self::Idle => libc::SCHED_IDLE,
            Self::Deadline => libc::SCHED_DEADLINE,
            Self::Ext => SCHED_EXT,
        }
    }

    pub(crate) fn from_raw(raw: u32) -> Option<Policy> {
        Self::ALL
            .into_iter()
            .find(|policy| u32::try_from(policy.raw()) == Ok(raw))
    }

    /// The policies -T applies to; only `sched_setattr(2)` can set their timing.
    pub(crate) fn carries_timing(self) -> bool {
        matches!(self, Self::Other | Self::Batch | Self::Deadline)
    }

    /// The realtime policies take a priority; every other policy carries 0 and
    /// may omit it.
    pub(crate) fn needs_priority(self) -> bool {
        matches!(self, Self::Fifo | Self::Rr)
    }

    /// The priority bounds the running kernel reports, -1 for either it refuses
    /// to state, as a kernel built without the policy does (EINVAL).
    pub(crate) fn kernel_bounds(self) -> (libc::c_int, libc::c_int) {
        // SAFETY: both take a plain integer and touch no memory.
        let min = unsafe { libc::sched_get_priority_min(self.raw()) };
        let max = unsafe { libc::sched_get_priority_max(self.raw()) };
        (min, max)
    }
}
