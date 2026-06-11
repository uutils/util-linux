// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.

use clap::{Arg, ArgAction, Command, crate_version, value_parser};
use uucore::libc;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("kill.md");
const USAGE: &str = help_usage!("kill.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    if let Some(pids) = matches.get_many::<i32>("pid") {
        for pid in pids {
            unsafe { libc::kill(*pid, libc::SIGTERM) };

            let err = std::io::Error::last_os_error().raw_os_error();
            if let Some(err_no) = err {
                match err_no {
                    libc::EPERM => {
                        eprintln!("bash: kill: ({pid}) - Operation not permitted");
                    }
                    libc::ESRCH => {
                        eprintln!("bash: kill: ({pid}) - No such process");
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("pid")
                .help("PID of the process to kill")
                .required(false)
                .action(ArgAction::Append)
                .value_name("PID")
                .value_parser(value_parser!(i32)),
        )
}
