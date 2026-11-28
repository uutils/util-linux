// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) getopt strtol

//! From the parsed options to the one thing chrt was asked to do. The checks
//! run in chrt(1)'s order, because the first failure is the one reported.

use std::ffi::{OsStr, OsString};

use clap::parser::ValueSource;
use clap::ArgMatches;

use crate::errors::{ChrtError, NumericArg};
use crate::options;
use crate::parse;
use crate::policy::Policy;

#[derive(Debug)]
pub(crate) struct Invocation {
    pub(crate) policy: Option<Policy>,
    /// A -f or -r anywhere on the line demands a priority even when a later
    /// policy option wins, as does the default SCHED_RR.
    pub(crate) priority_required: bool,
    pub(crate) reset_on_fork: bool,
    pub(crate) all_tasks: bool,
    pub(crate) pid_mode: bool,
    pub(crate) verbose: bool,
    pub(crate) parameters: Parameters,
    pub(crate) positionals: Vec<OsString>,
}

const POLICY_OPTIONS: [(&str, Policy); 7] = [
    (options::BATCH, Policy::Batch),
    (options::DEADLINE, Policy::Deadline),
    (options::EXT, Policy::Ext),
    (options::FIFO, Policy::Fifo),
    (options::IDLE, Policy::Idle),
    (options::OTHER, Policy::Other),
    (options::RR, Policy::Rr),
];

impl Invocation {
    pub(crate) fn from_matches(matches: &ArgMatches) -> Self {
        // The last policy option on the line wins. A flag not given still has
        // its default, with an index, so the source is checked first.
        let given: Vec<(usize, Policy)> = POLICY_OPTIONS
            .iter()
            .filter(|(id, _)| matches.value_source(id) == Some(ValueSource::CommandLine))
            .filter_map(|(id, policy)| {
                let last = matches.indices_of(id).and_then(Iterator::max)?;
                Some((last, *policy))
            })
            .collect();
        let policy = given
            .iter()
            .max_by_key(|(index, _)| *index)
            .map(|&(_, policy)| policy);
        let priority_required =
            policy.is_none() || given.iter().any(|(_, policy)| policy.needs_priority());
        let nanoseconds = |id: &str| {
            let value = matches.get_one::<u64>(id).copied();
            value.filter(|&value| value != 0)
        };

        Self {
            policy,
            priority_required,
            reset_on_fork: matches.get_flag(options::RESET_ON_FORK),
            all_tasks: matches.get_flag(options::ALL_TASKS),
            pid_mode: matches.get_flag(options::PID),
            verbose: matches.get_flag(options::VERBOSE),
            parameters: Parameters {
                runtime: nanoseconds(options::RUNTIME),
                period: nanoseconds(options::PERIOD),
                deadline: nanoseconds(options::DEADLINE_PARAMETER),
            },
            positionals: matches
                .get_many::<OsString>(options::ARGS)
                .map(|values| values.cloned().collect())
                .unwrap_or_default(),
        }
    }
}

/// In nanoseconds. A value of 0 is the same as none: chrt(1) neither holds it
/// to a policy nor passes it on over a default.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Parameters {
    pub(crate) runtime: Option<u64>,
    pub(crate) period: Option<u64>,
    pub(crate) deadline: Option<u64>,
}

#[derive(Debug)]
pub(crate) enum Target {
    Pid(i32),
    SelfThenExec(Vec<OsString>),
}

#[derive(Debug)]
pub(crate) struct Set {
    pub(crate) target: Target,
    pub(crate) policy: Policy,
    pub(crate) priority: i32,
    pub(crate) reset_on_fork: bool,
    pub(crate) parameters: Parameters,
    pub(crate) all_tasks: bool,
    pub(crate) verbose: bool,
}

#[derive(Debug)]
pub(crate) enum Request {
    Read { pid: i32, all_tasks: bool },
    Set(Set),
}

/// Under --pid with -v, chrt(1) shows the target's current attributes as soon
/// as the pid is parsed, so `show_current` is called with the pid right there.
pub(crate) fn build(
    invocation: &Invocation,
    mut show_current: impl FnMut(i32) -> Result<(), ChrtError>,
) -> Result<Request, ChrtError> {
    let policy = invocation.policy.unwrap_or(Policy::Rr);
    if invocation.pid_mode {
        let Some((first, rest)) = invocation.positionals.split_first() else {
            return Err(ChrtError::TooFewArguments);
        };
        // The pid is the last operand as written (`-p 0 --` names pid `--`)
        // and is parsed before anything else: `-p -f abc` reports the pid.
        let pid = number(rest.last().unwrap_or(first), NumericArg::Pid)?;
        if rest.is_empty() && invocation.policy.is_none() {
            return Ok(Request::Read {
                pid,
                all_tasks: invocation.all_tasks,
            });
        }
        // -v shows no current attributes for a negative pid, -1 included, as
        // chrt(1) does.
        if invocation.verbose && pid >= 0 {
            show_current(pid)?;
        }
        let (priority, command) = if pid < 0 {
            // Every operand, the pid included, is read as exec mode reads
            // them; for pid -1 the command is what that leaves.
            priority_and_command(invocation, policy)?
        } else if rest.is_empty() {
            if invocation.priority_required {
                return Err(ChrtError::PriorityRequired(policy));
            }
            (0, std::slice::from_ref(first))
        } else {
            // The first operand is the priority, read as the pid is; the
            // operands after it, the pid included, are the command for pid 0.
            (
                number(first, NumericArg::Priority)?,
                without_terminator(rest),
            )
        };
        applicable(policy, &invocation.parameters)?;
        let target = target(pid, command);
        return Ok(Request::Set(set(invocation, target, policy, priority)));
    }

    // The operands are judged before the timing options are matched to the
    // policy: `chrt -f -T 1 cmd` reports the missing priority.
    let (priority, command) = priority_and_command(invocation, policy)?;
    applicable(policy, &invocation.parameters)?;

    let target = Target::SelfThenExec(command.to_vec());
    Ok(Request::Set(set(invocation, target, policy, priority)))
}

/// The first operand is the priority only when it is all digits and something
/// follows it, if only a `--`; otherwise the command starts there.
fn priority_and_command(
    invocation: &Invocation,
    policy: Policy,
) -> Result<(i32, &[OsString]), ChrtError> {
    let operands = invocation.positionals.as_slice();
    let Some((first, rest)) = operands.split_first() else {
        return Err(ChrtError::TooFewArguments);
    };
    match parse::digits_i32(first) {
        Some(_) if rest.is_empty() && invocation.priority_required => {
            Err(ChrtError::NoCommandOrPriority)
        }
        Some(_) if rest.is_empty() => Ok((0, operands)),
        Some(Err(detail)) => Err(ChrtError::invalid_number(
            NumericArg::Priority,
            first,
            detail,
        )),
        Some(Ok(priority)) => Ok((priority, without_terminator(rest))),
        None if invocation.priority_required => Err(ChrtError::PriorityRequired(policy)),
        None => Ok((0, without_terminator(operands))),
    }
}

fn set(invocation: &Invocation, target: Target, policy: Policy, priority: i32) -> Set {
    Set {
        target,
        policy,
        priority,
        reset_on_fork: invocation.reset_on_fork,
        parameters: invocation.parameters,
        all_tasks: invocation.all_tasks,
        verbose: invocation.verbose,
    }
}

/// getopt stops at the first operand, so a `--` where the command starts never
/// reached it: chrt drops that one, so `chrt -o 0 -- cmd` runs `cmd`.
fn without_terminator(rest: &[OsString]) -> &[OsString] {
    match rest.split_first() {
        Some((first, remaining)) if first == "--" => remaining,
        _ => rest,
    }
}

/// Pid -1 is chrt itself too, as in chrt(1).
fn target(pid: i32, command: &[OsString]) -> Target {
    if pid == 0 || pid == -1 {
        Target::SelfThenExec(command.to_vec())
    } else {
        Target::Pid(pid)
    }
}

/// Checked once the operands are parsed and before the priority is held to its
/// range, as in chrt(1).
fn applicable(policy: Policy, parameters: &Parameters) -> Result<(), ChrtError> {
    if parameters.runtime.is_some() && !policy.carries_timing() {
        return Err(ChrtError::RuntimeNotSupported);
    }
    if (parameters.period.is_some() || parameters.deadline.is_some()) && policy != Policy::Deadline
    {
        return Err(ChrtError::DeadlineParametersNotSupported);
    }
    Ok(())
}

fn number(text: &OsStr, arg: NumericArg) -> Result<i32, ChrtError> {
    parse::strtol_i32(text).map_err(|detail| ChrtError::invalid_number(arg, text, detail))
}
