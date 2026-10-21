// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.
// spell-checker:ignore (words) classdata ioprio pgid pgrp

use clap::builder::ValueParser;
use clap::{crate_version, Arg, ArgAction, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

#[cfg(target_os = "linux")]
mod errors;
#[cfg(target_os = "linux")]
mod ioprio;
#[cfg(target_os = "linux")]
mod parse;

const ABOUT: &str = help_about!("ionice.md");
const USAGE: &str = help_usage!("ionice.md");

mod options {
    pub const CLASS: &str = "class";
    pub const CLASSDATA: &str = "classdata";
    pub const PID: &str = "pid";
    pub const PGID: &str = "pgid";
    pub const UID: &str = "uid";
    pub const IGNORE: &str = "ignore";
    pub const ARGS: &str = "args";
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::OsString;
    use std::io;
    use std::os::unix::process::CommandExt;
    use std::process;

    use clap::ArgMatches;
    use uucore::error::UResult;

    use crate::errors::{IoniceError, NumericArg};
    use crate::ioprio::{self, IoPrio, Who};
    use crate::options;
    use crate::parse::{self, ClassError};

    /// Which of -p, -P and -u was given. Only one of them may appear.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum IdKind {
        Pid,
        Pgid,
        Uid,
    }

    impl IdKind {
        fn who(self) -> Who {
            match self {
                Self::Pid => Who::Process,
                Self::Pgid => Who::Pgrp,
                Self::Uid => Who::User,
            }
        }

        fn numeric_arg(self) -> NumericArg {
            match self {
                Self::Pid => NumericArg::Pid,
                Self::Pgid => NumericArg::Pgid,
                Self::Uid => NumericArg::Uid,
            }
        }
    }

    /// A value-taking option, tagged for the ordered pass below.
    #[derive(Debug, Clone, Copy)]
    enum Slot {
        Class,
        ClassData,
        Id(IdKind),
    }

    /// What the command line asked for once the options have been resolved.
    #[derive(Debug, Default)]
    struct Request {
        class: Option<i32>,
        data: Option<i32>,
        id_kind: Option<IdKind>,
        first_id: i32,
        ignore: bool,
    }

    /// Rebuild the left-to-right order in which the value-taking options
    /// appeared. clap collects every value before returning, but its value
    /// indices are monotone in argv order, so sorting by them recovers the
    /// sequence needed to reject the first bad option rather than an arbitrary
    /// one.
    fn ordered_options(matches: &ArgMatches) -> Vec<(usize, Slot, String)> {
        let slots = [
            (options::CLASS, Slot::Class),
            (options::CLASSDATA, Slot::ClassData),
            (options::PID, Slot::Id(IdKind::Pid)),
            (options::PGID, Slot::Id(IdKind::Pgid)),
            (options::UID, Slot::Id(IdKind::Uid)),
        ];

        let mut events = Vec::new();

        for (id, slot) in slots {
            let (Some(values), Some(indices)) =
                (matches.get_many::<OsString>(id), matches.indices_of(id))
            else {
                continue;
            };

            events.extend(
                indices
                    .zip(values)
                    .map(|(index, value)| (index, slot, value.to_string_lossy().into_owned())),
            );
        }

        events.sort_by_key(|(index, _, _)| *index);
        events
    }

    /// Apply the options in argv order, stopping at the first bad one.
    fn resolve_options(matches: &ArgMatches) -> Result<Request, IoniceError> {
        let mut request = Request {
            ignore: matches.get_count(options::IGNORE) > 0,
            ..Request::default()
        };

        for (_, slot, value) in ordered_options(matches) {
            match slot {
                Slot::Class => {
                    let class = parse::class(&value).map_err(|error| match error {
                        ClassError::UnknownName => IoniceError::UnknownClass(value.clone()),
                        ClassError::Number(detail) => {
                            IoniceError::invalid_number(NumericArg::Class, &value, detail)
                        }
                    })?;
                    request.class = Some(class);
                }
                Slot::ClassData => {
                    let data = parse::level(&value).map_err(|detail| {
                        IoniceError::invalid_number(NumericArg::ClassData, &value, detail)
                    })?;
                    request.data = Some(data);
                }
                Slot::Id(kind) => {
                    // A second id option is rejected before its value is even
                    // looked at, so `-P 1 -p bogus` reports the conflict.
                    if request.id_kind.is_some() {
                        return Err(IoniceError::ConflictingIdKinds);
                    }
                    request.id_kind = Some(kind);
                    request.first_id = parse_id(kind, &value)?;
                }
            }
        }

        Ok(request)
    }

    fn parse_id(kind: IdKind, value: &str) -> Result<i32, IoniceError> {
        parse::blank_tolerant_i32(value)
            .map_err(|detail| IoniceError::invalid_number(kind.numeric_arg(), value, detail))
    }

    /// What a set uses when -c or -n is absent. These are ionice's defaults,
    /// not the kernel's; the kernel's own fallback is derived from CPU nice,
    /// not from these numbers.
    const DEFAULT_CLASS: i32 = ioprio::IOPRIO_CLASS_BE;
    const DEFAULT_DATA: i32 = 4;

    /// The priority level a class carries when it has no level of its own;
    /// a level given with -n is discarded.
    ///
    /// The two arms have different authorities behind them. None must carry
    /// zero because that is the only value the kernel accepts for it. Idle
    /// carrying 7 is the reference implementation's convention: the kernel
    /// takes any level with idle and hands it back unchanged, so nothing
    /// forces that choice.
    fn fixed_data(class: i32) -> Option<i32> {
        match class {
            ioprio::IOPRIO_CLASS_NONE => Some(0),
            ioprio::IOPRIO_CLASS_IDLE => Some(7),
            _ => None,
        }
    }

    /// Settle the class and priority level a set will use. The warning is
    /// silenced by -t, but the clearing of a meaningless priority level is not.
    fn resolve_priority(request: &Request) -> IoPrio {
        let class = request.class.unwrap_or(DEFAULT_CLASS);

        if let Some(data) = fixed_data(class) {
            // The level is replaced whether or not -n asked for one; only
            // saying so is conditional.
            if request.data.is_some() && !request.ignore {
                uucore::show_error!(
                    "ignoring given class data for {} class",
                    ioprio::class_name(class)
                );
            }
            return IoPrio::encode(class, data);
        }

        IoPrio::encode(class, request.data.unwrap_or(DEFAULT_DATA))
    }

    pub(crate) fn run(matches: &ArgMatches) -> UResult<()> {
        let request = resolve_options(matches)?;

        let arguments: Vec<OsString> = matches
            .get_many::<OsString>(options::ARGS)
            .map(|values| values.cloned().collect())
            .unwrap_or_default();

        // Trailing arguments are further ids whenever an id option was given,
        // and the command to run otherwise.
        let command = if request.id_kind.is_none() && !arguments.is_empty() {
            Some(arguments.as_slice())
        } else {
            None
        };

        // Running a command always sets a priority, defaulting to best-effort
        // level 4. The warning belongs here rather than in the dispatch below,
        // because `ionice -c 3 -n 5` reports the discarded level before the
        // bad usage.
        let priority = (request.class.is_some() || request.data.is_some() || command.is_some())
            .then(|| resolve_priority(&request));

        match (request.id_kind, command, priority) {
            // A get whose only id is 0 reports the calling process rather than
            // the process group or user numbered 0. Setting has no such
            // fallback: `-c 3 -u 0` really does target uid 0.
            (Some(_), _, None) if request.first_id == 0 && arguments.is_empty() => {
                report(Who::Process, 0)
            }
            (Some(kind), _, _) => act_on_ids(&request, kind, priority, &arguments),
            (None, Some(command), Some(priority)) => {
                apply(&request, Who::Process, 0, priority)?;
                run_command(command)
            }
            (None, None, Some(_)) => Err(IoniceError::BadUsage.into()),
            (None, _, None) => report(Who::Process, 0),
        }
    }

    /// Act on the id given to the option, then on every trailing argument,
    /// parsing each one only when its turn comes so that a bad id later in the
    /// list does not hide the results of the ids before it.
    fn act_on_ids(
        request: &Request,
        kind: IdKind,
        priority: Option<IoPrio>,
        arguments: &[OsString],
    ) -> UResult<()> {
        let who = kind.who();
        act_on_id(request, who, request.first_id, priority)?;

        for argument in arguments {
            let id = parse_id(kind, &argument.to_string_lossy())?;
            act_on_id(request, who, id, priority)?;
        }

        Ok(())
    }

    fn act_on_id(request: &Request, who: Who, id: i32, priority: Option<IoPrio>) -> UResult<()> {
        match priority {
            Some(priority) => apply(request, who, id, priority),
            None => report(who, id),
        }
    }

    fn report(who: Who, id: i32) -> UResult<()> {
        let priority = ioprio::get(who, id).map_err(IoniceError::GetFailed)?;
        println!("{priority}");
        Ok(())
    }

    /// Set a priority, honoring -t: a failure to set is then silent and leaves
    /// the exit status alone.
    fn apply(request: &Request, who: Who, id: i32, priority: IoPrio) -> UResult<()> {
        match ioprio::set(who, id, priority) {
            Ok(()) => Ok(()),
            Err(_) if request.ignore => Ok(()),
            Err(error) => Err(IoniceError::SetFailed(error).into()),
        }
    }

    /// Replace this process with the command. ionice does not fork, so the
    /// command inherits the priority just set and its exit status is naturally
    /// this process's own.
    fn run_command(command: &[OsString]) -> UResult<()> {
        let Some((program, arguments)) = command.split_first() else {
            return Ok(());
        };

        // exec() returns only when it failed.
        let error = process::Command::new(program).args(arguments).exec();

        uucore::show_error!(
            "failed to execute {}: {}",
            program.to_string_lossy(),
            uucore::error::strip_errno(&error)
        );
        uucore::error::set_exit_code(if error.kind() == io::ErrorKind::NotFound {
            127
        } else {
            126
        });

        Ok(())
    }
}

#[cfg(target_os = "linux")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    linux::run(&matches)
}

#[cfg(not(target_os = "linux"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`ionice` is available only on Linux.",
    ))
}

/// A value-taking option. Hyphen-leading values are accepted so that the
/// argument following the flag is taken verbatim, which is what makes
/// `ionice -p -c 3` report an invalid PID of "-c" instead of parsing -c.
fn value_option(
    id: &'static str,
    short: char,
    long: &'static str,
    value_name: &'static str,
) -> Arg {
    Arg::new(id)
        .short(short)
        .long(long)
        .value_name(value_name)
        .action(ArgAction::Append)
        .num_args(1)
        .allow_hyphen_values(true)
        .value_parser(ValueParser::os_string())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(value_option(options::CLASS, 'c', "class", "class").help(
            "name or number of the scheduling class: \
                 none (0), realtime (1), best-effort (2) or idle (3)",
        ))
        .arg(
            value_option(options::CLASSDATA, 'n', "classdata", "num").help(
                "priority (0..7) in the specified scheduling class, \
                 only for the realtime and best-effort classes",
            ),
        )
        .arg(
            value_option(options::PID, 'p', "pid", "pid")
                .help("act on these already running processes"),
        )
        .arg(
            value_option(options::PGID, 'P', "pgid", "pgrp")
                .help("act on already running processes in these groups"),
        )
        // Declared between -P and -u so that --help lists the options in the
        // same order the reference does; clap orders by declaration.
        .arg(
            Arg::new(options::IGNORE)
                .short('t')
                .long("ignore")
                // Count rather than SetTrue: clap rejects a repeated SetTrue
                // flag, and `ionice -t -t` is accepted.
                .action(ArgAction::Count)
                .help("ignore failures"),
        )
        .arg(
            value_option(options::UID, 'u', "uid", "uid")
                .help("act on already running processes owned by these users"),
        )
        .arg(
            Arg::new(options::ARGS)
                .value_name("command")
                .help("further pid, pgrp or uid arguments, or the command to run")
                .index(1)
                .action(ArgAction::Set)
                .trailing_var_arg(true)
                .value_parser(ValueParser::os_string())
                .num_args(1..),
        )
}
