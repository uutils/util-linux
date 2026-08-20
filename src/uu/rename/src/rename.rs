// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::path::Path;

use clap::builder::ValueParser;
use clap::{crate_version, Arg, ArgAction, ArgMatches, Command};
use uucore::error::{set_exit_code, UResult};
use uucore::{format_usage, help_about, help_usage};

mod argv;
mod encoding;
mod errors;
mod output;
mod prompt;
mod subst;

use argv::collect_getopt_argv;
use encoding::{os_string, units, Unit, SEP};
use errors::RenameError;
use output::Output;
use prompt::Answering;
use subst::{rewrite, Mode};

const ABOUT: &str = help_about!("rename.md");
const USAGE: &str = help_usage!("rename.md");

mod options {
    pub const VERBOSE: &str = "verbose";
    pub const SYMLINK: &str = "symlink";
    pub const NO_ACT: &str = "no-act";
    pub const ALL: &str = "all";
    pub const LAST: &str = "last";
    pub const NO_OVERWRITE: &str = "no-overwrite";
    pub const INTERACTIVE: &str = "interactive";
    pub const SUBSTRING: &str = "substring";
    pub const REPLACEMENT: &str = "replacement";
    pub const FILES: &str = "files";
}

/// What the run did, counted per operand.
///
/// The status is selected from two counters rather than or'd together, so a
/// no-match, a name that did not change, an -o skip and an -i decline - which
/// touch neither counter - can neither degrade a success nor promote a
/// failure. The documented 64, for an unanticipated error, is never produced.
#[derive(Debug, Default)]
struct Tally {
    /// Operands whose computed name differed from the current one and whose
    /// operation succeeded. Note that this is not the same as "the tree
    /// changed": two links to one inode rename successfully and change nothing.
    renamed: usize,
    failed: usize,
}

impl Tally {
    fn code(&self) -> i32 {
        match (self.renamed > 0, self.failed > 0) {
            (true, true) => 2,
            (true, false) => 0,
            (false, true) => 1,
            (false, false) => 4,
        }
    }
}

/// What one operand did. A skip, a no-match and an unchanged name are all
/// `Neither`; C util-linux cannot tell them apart either.
enum Outcome {
    Renamed,
    Neither,
}

struct Options {
    verbose: bool,
    symlink: bool,
    no_act: bool,
    no_overwrite: bool,
    interactive: bool,
    mode: Mode,
    needle: Vec<Unit>,
    replacement: Vec<Unit>,
}

impl Options {
    fn from_matches(matches: &ArgMatches) -> Self {
        let mode = if matches.get_flag(options::ALL) {
            Mode::All
        } else if matches.get_flag(options::LAST) {
            Mode::Last
        } else {
            Mode::First
        };

        Self {
            verbose: matches.get_flag(options::VERBOSE),
            symlink: matches.get_flag(options::SYMLINK),
            no_act: matches.get_flag(options::NO_ACT),
            no_overwrite: matches.get_flag(options::NO_OVERWRITE),
            interactive: matches.get_flag(options::INTERACTIVE),
            mode,
            needle: argument(matches, options::SUBSTRING),
            replacement: argument(matches, options::REPLACEMENT),
        }
    }
}

/// Both are `required(true)`, so clap has already refused an invocation that
/// omits them; the default stands in for an `unwrap` the review guidelines
/// forbid.
fn argument(matches: &ArgMatches, id: &str) -> Vec<Unit> {
    matches
        .get_one::<OsString>(id)
        .map(|value| units(value.as_os_str()))
        .unwrap_or_default()
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match uu_app().try_get_matches_from(collect_getopt_argv(args)) {
        Ok(matches) => matches,
        Err(error) => {
            report_parse_failure(&error);
            return Ok(());
        }
    };
    let options = Options::from_matches(&matches);

    // An identical substring and replacement short-circuit the whole run, not
    // one operand: C util-linux issues no filesystem syscall at all, even for
    // operands that do not exist.
    if options.needle == options.replacement {
        set_exit_code(4);
        return Ok(());
    }

    let mut tally = Tally::default();
    let stdout = io::stdout();
    // C util-linux's stdout is stdio's: line buffered on a terminal and fully
    // buffered anywhere else. Rust's is line buffered everywhere, and the
    // difference is not only a matter of syscall counts - under a write limit
    // it stops our loop part way through a run C util-linux finishes.
    let sink: Box<dyn Write> = if stdout.is_terminal() {
        Box::new(stdout.lock())
    } else {
        Box::new(BufWriter::new(stdout.lock()))
    };
    let mut out = Output::new(sink);
    let mut err = Output::new(io::stderr().lock());

    // Asked once for the whole run, before any operand is looked at, which is
    // where C util-linux asks it too.
    let answering = Answering::from_stdin();

    for operand in matches
        .get_many::<OsString>(options::FILES)
        .unwrap_or_default()
    {
        match rename_one(&options, answering, operand, &mut out) {
            Ok(Outcome::Renamed) => tally.renamed += 1,
            Ok(Outcome::Neither) => {}
            Err(error) => {
                error.report(&mut err);
                tally.failed += 1;
            }
        }
    }

    out.flush();
    // A failed write discards the tally on either stream, and C util-linux
    // finishes the run first either way. Only the report stream's failure has
    // anywhere left to report itself, and only it forgives a reader that left.
    let report_failed = if let Some(source) = out.into_reported_error() {
        RenameError::WriteFailed { source }.report(&mut err);
        true
    } else {
        false
    };
    err.flush();
    let diagnostics_failed = err.into_error().is_some();

    set_exit_code(if report_failed || diagnostics_failed {
        1
    } else {
        tally.code()
    });

    Ok(())
}

/// clap has already decided what to say and which stream to say it on; all
/// that is left is to say it without unwrapping the write.
///
/// The `?` this replaces hands the error to uucore, which prints it through a
/// `Display` impl that cannot report a failed write and panics on one. Calling
/// `print` here returns that result instead. Note the status comes from
/// `use_stderr` and not from clap's own `exit_code`, which is 2 for a usage
/// error where both C util-linux and this tree use 1.
fn report_parse_failure(error: &clap::Error) {
    let written = error.print();
    let mut err = Output::new(io::stderr().lock());

    match written {
        // A reader that has gone away is not reported, here or at exit.
        Err(source) if source.kind() != io::ErrorKind::BrokenPipe => {
            RenameError::WriteFailed { source }.report(&mut err);
            err.flush();
            set_exit_code(1);
        }
        _ => set_exit_code(i32::from(error.use_stderr())),
    }
}

/// One operand, start to finish. The only place that touches the filesystem.
fn rename_one<W: Write>(
    options: &Options,
    answering: Answering,
    operand: &OsStr,
    out: &mut Output<W>,
) -> Result<Outcome, RenameError> {
    // The existence check keeps the operand exactly as it was typed, trailing
    // separators and all, which is why `d1/s1/` reports ENOTDIR instead of
    // renaming `d1/s1`. Everything after this point uses the stripped form.
    let metadata = fs::symlink_metadata(operand).map_err(|source| RenameError::NotAccessible {
        path: operand.to_os_string(),
        source,
    })?;

    // This precedes the match test, so a non-symlink fails even when the
    // substring appears nowhere in its name.
    if options.symlink && !metadata.file_type().is_symlink() {
        return Err(RenameError::NotASymlink {
            path: operand.to_os_string(),
        });
    }

    // Symlink mode rewrites the link's target text and never the link's own
    // name; the chain is not resolved, so a link to a link sees only the next
    // hop's text.
    let source = if options.symlink {
        fs::read_link(operand)
            .map_err(|source| RenameError::NotAccessible {
                path: operand.to_os_string(),
                source,
            })?
            .into_os_string()
    } else {
        operand.to_os_string()
    };

    let source_units = units(&source);
    let change = rewrite(
        &source_units,
        &options.needle,
        &options.replacement,
        options.mode,
        SEP,
    );
    if change.is_unchanged() {
        return Ok(Outcome::Neither);
    }

    let old = os_string(change.old.to_vec());
    let new = os_string(change.new);

    // Only the two safeguards ask, and so C util-linux only asks for them:
    // without -o or -i it never looks at the destination at all, and neither
    // does this.
    let taken = (options.no_overwrite || options.interactive)
        && if options.symlink {
            link_target_entry_exists(&new)
        } else {
            destination_exists_following_links(&new)
        };

    let skipped = if !taken {
        false
    } else if options.interactive && !options.no_act {
        !answering.accepts(out, &new)
    } else {
        // -o, or -i under -n: -n never prompts and never reads stdin. `taken`
        // is only ever set under one of the two safeguards, so reaching here
        // means the other one is in force.
        true
    };
    if skipped {
        report_skip(out, options, operand, &old, &new);
    }

    if options.no_act {
        // The verbose line prints even for an operand the guard skipped, so a
        // skipped operand under -n prints two lines and counts as neither.
        report(out, options, operand, &old, &new);
        return Ok(if skipped {
            Outcome::Neither
        } else {
            Outcome::Renamed
        });
    }

    if skipped {
        return Ok(Outcome::Neither);
    }

    if options.symlink {
        // Not atomic, and deliberately so: C util-linux unlinks then creates
        // too, and a failure to create the replacement leaves the original
        // link gone.
        fs::remove_file(operand).map_err(|source| RenameError::UnlinkFailed {
            path: operand.to_os_string(),
            source,
        })?;
        encoding::symlink(&new, Path::new(operand)).map_err(|source| {
            RenameError::SymlinkFailed {
                path: operand.to_os_string(),
                new: new.clone(),
                source,
            }
        })?;
    } else {
        fs::rename(&old, &new).map_err(|source| RenameError::RenameFailed {
            old: old.clone(),
            new: new.clone(),
            source,
        })?;
    }

    report(out, options, operand, &old, &new);
    Ok(Outcome::Renamed)
}

/// The default path asks whether anything is reachable at the destination, so
/// it follows symlinks: a dangling symlink sitting there does not count as
/// existing, and -o lets the rename clobber it. A probe that cannot be
/// performed at all - a parent directory denying search - reads as absent,
/// which is measured behavior and not a defensive default.
///
/// Both guards ask whether the NAME is taken, never whether it names the same
/// file as the source, as C util-linux does. That shows on a filesystem which
/// folds case, where a rename differing only in case finds its own source at
/// the destination: -o skips it and -i prompts for it.
fn destination_exists_following_links(path: &OsStr) -> bool {
    Path::new(path).try_exists().unwrap_or(false)
}

/// Symlink mode asks a different question: is there an ENTRY at the new target
/// path? This is an lstat where the one above is a stat, so a dangling entry
/// counts here and blocks the rewrite. Do not merge the two.
fn link_target_entry_exists(path: &OsStr) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn report<W: Write>(
    out: &mut Output<W>,
    options: &Options,
    operand: &OsStr,
    old: &OsStr,
    new: &OsStr,
) {
    if !options.verbose {
        return;
    }

    if options.symlink {
        out.write_os(operand);
        out.write_bytes(b": ");
    }
    out.write_quoted(old);
    out.write_bytes(b" -> ");
    out.write_quoted(new);
    out.write_bytes(b"\n");
}

/// The skip report, which unlike the prompt is printed only under -v.
fn report_skip<W: Write>(
    out: &mut Output<W>,
    options: &Options,
    operand: &OsStr,
    old: &OsStr,
    new: &OsStr,
) {
    if !options.verbose {
        return;
    }

    if options.symlink {
        // Symlink mode names the link and the target it still has.
        out.write_bytes(b"Skipping existing link: ");
        out.write_quoted(operand);
        out.write_bytes(b" -> ");
        out.write_quoted(old);
    } else {
        out.write_bytes(b"Skipping existing file: ");
        out.write_quoted(new);
    }
    out.write_bytes(b"\n");
}

fn flag(id: &'static str, short: char, help: &'static str) -> Arg {
    Arg::new(id)
        .short(short)
        .long(id)
        .help(help)
        .action(ArgAction::SetTrue)
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        // getopt lets a flag repeat, and so does C util-linux: `-v -v` renames.
        // A SetTrue argument rejects its second occurrence without this, and
        // it does not weaken `conflicts_with`, which clap validates separately.
        .args_override_self(true)
        .arg(flag(options::VERBOSE, 'v', "explain what is being done"))
        .arg(flag(options::SYMLINK, 's', "act on the target of symlinks"))
        .arg(flag(options::NO_ACT, 'n', "do not make any changes"))
        .arg(flag(options::ALL, 'a', "replace all occurrences").conflicts_with(options::LAST))
        .arg(flag(options::LAST, 'l', "replace only the last occurrence"))
        .arg(
            flag(options::NO_OVERWRITE, 'o', "don't overwrite existing files")
                .conflicts_with(options::INTERACTIVE),
        )
        .arg(flag(options::INTERACTIVE, 'i', "prompt before overwrite"))
        // The usage line already names all three, and C util-linux's help has
        // no operand section at all, so listing them again would print three
        // names with nothing beside them.
        .arg(
            Arg::new(options::SUBSTRING)
                .value_name("substring")
                .required(true)
                .hide(true)
                .value_parser(ValueParser::os_string()),
        )
        .arg(
            Arg::new(options::REPLACEMENT)
                .value_name("replacement")
                .required(true)
                .hide(true)
                .value_parser(ValueParser::os_string()),
        )
        .arg(
            Arg::new(options::FILES)
                .value_name("file")
                .required(true)
                .num_args(1..)
                .hide(true)
                .action(ArgAction::Append)
                .value_parser(ValueParser::os_string()),
        )
}
