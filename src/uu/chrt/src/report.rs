// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) sched

//! The bytes chrt(1) prints: one task's attributes, one row of the -m table.

use std::fmt::Write;

use crate::policy::Policy;
use crate::sched::SCHED_FLAG_RESET_ON_FORK;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Current,
    New,
}

impl Phase {
    fn word(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::New => "new",
        }
    }
}

pub(crate) fn block(out: &mut impl Write, pid: i32, phase: Phase, attr: &libc::sched_attr) {
    let phase = phase.word();
    let policy = Policy::from_raw(attr.sched_policy);
    // A policy this build does not know is not reported as an error: the
    // task has one, whatever it is called.
    let name = policy.map_or("unknown", Policy::name);
    let reset = if attr.sched_flags & SCHED_FLAG_RESET_ON_FORK != 0 {
        "|SCHED_RESET_ON_FORK"
    } else {
        ""
    };
    // Neither a String nor an Output fails a write; the results are dropped
    // on purpose.
    let _ = writeln!(out, "pid {pid}'s {phase} scheduling policy: {name}{reset}");
    let _ = writeln!(
        out,
        "pid {pid}'s {phase} scheduling priority: {}",
        attr.sched_priority
    );
    match policy {
        Some(Policy::Other | Policy::Batch) => {
            let _ = writeln!(
                out,
                "pid {pid}'s {phase} runtime parameter: {}",
                attr.sched_runtime
            );
        }
        Some(Policy::Deadline) => {
            let _ = writeln!(
                out,
                "pid {pid}'s {phase} runtime/deadline/period parameters: {}/{}/{}",
                attr.sched_runtime, attr.sched_deadline, attr.sched_period
            );
        }
        _ => {}
    }
}

pub(crate) fn max_row(policy: Policy, (min, max): (i32, i32)) -> String {
    let name = policy.name();
    if min < 0 || max < 0 {
        format!("{name} not supported?\n")
    } else {
        format!("{name} min/max priority\t: {min}/{max}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attr(policy: Policy, runtime: u64) -> libc::sched_attr {
        libc::sched_attr {
            sched_policy: policy.raw() as u32,
            sched_runtime: runtime,
            ..crate::sched::zeroed_attr()
        }
    }

    fn render(pid: i32, phase: Phase, attr: &libc::sched_attr) -> String {
        let mut out = String::new();
        block(&mut out, pid, phase, attr);
        out
    }

    #[test]
    fn deadline_prints_its_three_parameters() {
        let mut deadline = attr(Policy::Deadline, 1_000_000);
        deadline.sched_deadline = 1_000_000;
        deadline.sched_period = 1_000_000;
        assert_eq!(
            render(978_562, Phase::Current, &deadline),
            "pid 978562's current scheduling policy: SCHED_DEADLINE\n\
             pid 978562's current scheduling priority: 0\n\
             pid 978562's current runtime/deadline/period parameters: 1000000/1000000/1000000\n"
        );
    }

    #[test]
    fn an_unknown_policy_number_is_still_reported() {
        let mut odd = attr(Policy::Other, 0);
        odd.sched_policy = 42;
        assert_eq!(
            render(1, Phase::Current, &odd),
            "pid 1's current scheduling policy: unknown\n\
             pid 1's current scheduling priority: 0\n"
        );
    }
}
