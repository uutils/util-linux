// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{builder::PossibleValuesParser, crate_version, Arg, ArgAction, ArgMatches, Command};
#[cfg(target_family = "unix")]
use uucore::error::{set_exit_code, UIoError};
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("mesg.md");
const USAGE: &str = help_usage!("mesg.md");

#[cfg(target_family = "unix")]
pub fn do_mesg(matches: &ArgMatches) -> UResult<()> {
    use nix::sys::stat::{fchmod, fstat, Mode};
    use std::{io, os::fd::AsRawFd};
    use std::{io::IsTerminal, os::fd::AsFd};

    for fd in &[
        std::io::stdin().as_fd(),
        std::io::stdout().as_fd(),
        std::io::stderr().as_fd(),
    ] {
        if fd.is_terminal() {
            let st = fstat(fd.as_raw_fd())?;
            if let Some(enable) = matches.get_one::<String>("enable") {
                // 'mesg y' on the GNU version seems to only modify the group write bit,
                // but 'mesg n' modifies both group and others write bits.
                let new_mode = if enable == "y" {
                    st.st_mode | 0o020
                } else {
                    st.st_mode & !0o022
                };
                fchmod(fd.as_raw_fd(), Mode::from_bits_retain(new_mode))?;
                if enable == "n" {
                    set_exit_code(1);
                }
                if matches.get_flag("verbose") {
                    println!(
                        "write access to your terminal is {}",
                        if enable == "y" { "allowed" } else { "denied" }
                    );
                }
            } else if st.st_mode & 0o022 != 0 {
                println!("is y");
            } else {
                set_exit_code(1);
                println!("is n");
            }
            return Ok(());
        }
    }
    Err(UIoError::new(
        io::ErrorKind::Other,
        "stdin/stdout/stderr is not a terminal",
    ))
}

#[cfg(target_family = "unix")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;
    if let Err(e) = do_mesg(&matches) {
        set_exit_code(2);
        uucore::show_error!("{}", e);
    };
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Explain what is being done")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("enable")
                .help("Whether to allow or disallow messages")
                .value_parser(PossibleValuesParser::new(["y", "n"]))
                .action(ArgAction::Set),
        )
}

#[cfg(not(target_family = "unix"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(uucore::error::USimpleError::new(
        1,
        "`mesg` is available only on Unix platforms.",
    ))
}
