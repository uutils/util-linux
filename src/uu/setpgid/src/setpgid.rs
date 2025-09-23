// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command as ClapCommand};
use uucore::{
    error::{UResult, USimpleError},
    format_usage, help_about, help_usage,
};

const ABOUT: &str = help_about!("setpgid.md");
const USAGE: &str = help_usage!("setpgid.md");

#[cfg(target_family = "unix")]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    use std::ffi::CString;
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    let matches = uu_app().try_get_matches_from(args)?;

    let remaining_args: Vec<String> = matches
        .get_many::<String>("args")
        .unwrap()
        .cloned()
        .collect();

    if unsafe { libc::setpgid(0, 0) } != 0 {
        return Err(USimpleError::new(
            1,
            format!(
                "failed to create new process group: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }

    if matches.get_flag("foreground") {
        if let Ok(tty_file) = File::open("/dev/tty") {
            unsafe {
                libc::tcsetpgrp(tty_file.as_raw_fd(), libc::getpgrp());
            }
        }
        // According to strace open("/dev/tty") failure is ignored.
    }

    let program = &remaining_args[0];
    let program_args = &remaining_args[1..];

    // Command line arguments can't contain NUL bytes, so unwrap() is safe here.
    let program_cstr = CString::new(program.as_str()).unwrap();
    let mut argv = vec![program_cstr.clone()];
    for arg in program_args {
        argv.push(CString::new(arg.as_str()).unwrap());
    }

    let Err(e) = nix::unistd::execvp(&program_cstr, &argv);
    Err(USimpleError::new(
        1,
        format!("failed to execute '{}': {}", program, e),
    ))
}

#[cfg(not(target_family = "unix"))]
#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    Err(USimpleError::new(
        1,
        "`setpgid` is unavailable on non-UNIX-like platforms.",
    ))
}

pub fn uu_app() -> ClapCommand {
    ClapCommand::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("foreground")
                .short('f')
                .long("foreground")
                .help("Make a foreground process group")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("args")
                .hide_short_help(true)
                .hide_long_help(true)
                .required(true)
                .action(ArgAction::Append)
                .num_args(1..)
                .trailing_var_arg(true),
        )
}
