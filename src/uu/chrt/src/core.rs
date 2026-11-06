// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) setattr sched ENOENT getpriority niced

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::unix::process::CommandExt;
use std::process;

use uucore::error::UResult;

use crate::errors::{ChrtError, TaskKind};
use crate::output::Output;
use crate::policy::Policy;
use crate::report::{block, Phase};
use crate::request::{self, Invocation, Request, Set, Target};
use crate::sched::{self, SCHED_FLAG_RESET_ON_FORK};

pub(crate) fn run(invocation: &Invocation) -> UResult<()> {
    let mut out = Output::stdout();
    let request = request::build(invocation, |pid| {
        show(&mut out, pid, invocation.all_tasks, Phase::Current)
    });
    let outcome = request.and_then(|request| match request {
        Request::Read { pid, all_tasks } => show(&mut out, pid, all_tasks, Phase::Current),
        Request::Set(set) => match &set.target {
            Target::Pid(pid) => change(&mut out, &set, *pid),
            Target::SelfThenExec(command) => change_self_then_exec(&mut out, &set, command),
        },
    });
    // chrt(1) writes an error at once and its buffered output at exit, so
    // off a terminal the error comes first.
    if let Err(error) = outcome {
        uucore::show!(error);
    }
    out.finish().map_err(Into::into)
}

/// A failure partway leaves the earlier tasks changed, as in chrt(1). Under
/// -v the new blocks come from a second read of the task list.
fn change(out: &mut impl fmt::Write, set: &Set, pid: i32) -> Result<(), ChrtError> {
    within_range(set.policy.kernel_bounds(), set.priority)?;
    let kind = if set.all_tasks {
        TaskKind::Tid
    } else {
        TaskKind::Pid
    };
    walk(
        pid,
        set.all_tasks,
        |tid| apply(kind, tid, set),
        |error| Err(ChrtError::TaskListUnavailable(error)),
    )?;
    if set.verbose {
        show(out, pid, set.all_tasks, Phase::New)?;
    }
    Ok(())
}

/// Under -v the new block is shown before the exec; off a terminal it waits in
/// the buffer the exec discards, so it shows only if chrt reaches exit.
fn change_self_then_exec(
    out: &mut impl fmt::Write,
    set: &Set,
    command: &[OsString],
) -> Result<(), ChrtError> {
    within_range(set.policy.kernel_bounds(), set.priority)?;
    // There is no task list to walk for a command that has not started.
    if set.all_tasks {
        return Err(ChrtError::TaskListUnavailable(
            io::Error::from_raw_os_error(libc::ENOENT),
        ));
    }
    apply(TaskKind::Pid, 0, set)?;
    if set.verbose {
        show(out, 0, false, Phase::New)?;
    }
    // `chrt -o 0 --` has a priority and nothing to run: the set has already
    // happened by the time this is found out, as in chrt(1).
    let Some((program, arguments)) = command.split_first() else {
        return Err(ChrtError::NoCommand);
    };
    let error = process::Command::new(program).args(arguments).exec();
    Err(ChrtError::ExecFailed {
        program: program.clone(),
        error,
    })
}

fn walk(
    pid: i32,
    all_tasks: bool,
    mut each: impl FnMut(i32) -> Result<(), ChrtError>,
    unlisted: impl Fn(io::Error) -> Result<(), ChrtError>,
) -> Result<(), ChrtError> {
    if !all_tasks {
        return each(pid);
    }
    let tasks = match sched::tasks(pid) {
        Ok(tasks) => tasks,
        Err(error) => return unlisted(error),
    };
    for tid in tasks {
        match tid {
            Ok(tid) => each(tid)?,
            Err(error) => return unlisted(error),
        }
    }
    Ok(())
}

/// A task list that cannot be taken shows none, which chrt(1) does not report:
/// `chrt -a -p` of a pid that names nothing prints nothing and succeeds.
fn show(
    out: &mut impl fmt::Write,
    pid: i32,
    all_tasks: bool,
    phase: Phase,
) -> Result<(), ChrtError> {
    walk(
        pid,
        all_tasks,
        |tid| {
            // Pid 0 asks the kernel about chrt itself, and chrt(1) names it
            // by its real pid.
            let named = if tid == 0 { process::id() as i32 } else { tid };
            block(out, named, phase, &attributes(tid)?);
            Ok(())
        },
        |_| Ok(()),
    )
}

fn attributes(pid: i32) -> Result<libc::sched_attr, ChrtError> {
    sched::get_attr(pid).map_err(|error| ChrtError::GetFailed { pid, error })
}

/// The kernel's bounds, taken as they come: for a policy the kernel lacks both
/// are -1, which refuses every priority but -1.
fn within_range((min, max): (i32, i32), priority: i32) -> Result<(), ChrtError> {
    if (min..=max).contains(&priority) {
        Ok(())
    } else {
        Err(ChrtError::UnsupportedPriority(priority))
    }
}

/// `sched_setattr` sets the nice value too, so the task's own is passed back:
/// resetting a niced task to 0 needs privilege.
fn apply(kind: TaskKind, tid: i32, set: &Set) -> Result<(), ChrtError> {
    let result = if set.policy.carries_timing() {
        // A task getpriority cannot find, sched_setattr cannot find either,
        // and its error is the one chrt(1) reports.
        let nice = sched::nice(tid).unwrap_or(0);
        sched::set_attr(tid, attributes_for(set, nice))
    } else {
        let reset = if set.reset_on_fork {
            libc::SCHED_RESET_ON_FORK
        } else {
            0
        };
        sched::set_scheduler(tid, set.policy.raw() | reset, set.priority)
    };
    result.map_err(|error| ChrtError::SetFailed {
        kind,
        id: tid,
        error,
    })
}

/// The relations between the deadline parameters are the kernel's to judge;
/// for SCHED_OTHER and SCHED_BATCH a runtime of 0 asks for the default slice.
fn attributes_for(set: &Set, nice: i32) -> libc::sched_attr {
    let given = set.parameters;
    let (runtime, deadline, period) = if set.policy == Policy::Deadline {
        let deadline = given.deadline.or(given.period).unwrap_or(0);
        (
            given.runtime.unwrap_or(deadline),
            deadline,
            given.period.unwrap_or(0),
        )
    } else {
        (given.runtime.unwrap_or(0), 0, 0)
    };
    libc::sched_attr {
        sched_policy: set.policy.raw() as u32,
        sched_flags: if set.reset_on_fork {
            SCHED_FLAG_RESET_ON_FORK
        } else {
            0
        },
        sched_nice: nice,
        // Negative only past a lower bound the kernel refused to state, which
        // it does for a policy it lacks; the set then fails on the policy.
        sched_priority: u32::try_from(set.priority).unwrap_or(0),
        sched_runtime: runtime,
        sched_deadline: deadline,
        sched_period: period,
        ..sched::zeroed_attr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Parameters;

    fn deadline_set(runtime: Option<u64>, deadline: Option<u64>, period: Option<u64>) -> Set {
        Set {
            target: Target::Pid(1),
            policy: Policy::Deadline,
            priority: 0,
            reset_on_fork: false,
            parameters: Parameters {
                runtime,
                deadline,
                period,
            },
            all_tasks: false,
            verbose: false,
        }
    }

    fn timing(set: &Set) -> (u64, u64, u64) {
        let attr = attributes_for(set, 0);
        (attr.sched_runtime, attr.sched_deadline, attr.sched_period)
    }

    #[test]
    fn omitted_deadline_parameters_copy_from_the_ones_given() {
        assert_eq!(
            timing(&deadline_set(None, None, Some(1_000_000))),
            (1_000_000, 1_000_000, 1_000_000)
        );
        assert_eq!(
            timing(&deadline_set(None, Some(1_000_000), None)),
            (1_000_000, 1_000_000, 0)
        );
        assert_eq!(
            timing(&deadline_set(Some(500_000), None, None)),
            (500_000, 0, 0)
        );
        assert_eq!(
            timing(&deadline_set(
                Some(500_000),
                Some(1_000_000),
                Some(2_000_000)
            )),
            (500_000, 1_000_000, 2_000_000)
        );
        assert_eq!(timing(&deadline_set(None, None, None)), (0, 0, 0));
    }

    #[test]
    fn other_passes_the_slice_alone() {
        let mut set = deadline_set(Some(3_000_000), Some(9), Some(9));
        set.policy = Policy::Other;
        assert_eq!(timing(&set), (3_000_000, 0, 0));
    }
}
