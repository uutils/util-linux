// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, Command};
use std::fs;
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("nologin.md");
const USAGE: &str = help_usage!("nologin.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;

    // Try to read custom message from /etc/nologin.txt
    let message = match fs::read_to_string("/etc/nologin.txt") {
        Ok(content) => content.trim().to_string(),
        Err(_) => "This account is currently not available.".to_string(),
    };

    println!("{}", message);
    std::process::exit(1);
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new("command")
                .short('c')
                .long("command")
                .value_name("command")
                .help("does nothing (for compatibility)"),
        )
        .arg(
            Arg::new("init-file")
                .long("init-file")
                .value_name("file")
                .help("does nothing (for compatibility)"),
        )
        .arg(
            Arg::new("interactive")
                .short('i')
                .long("interactive")
                .help("does nothing (for compatibility)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("login")
                .short('l')
                .long("login")
                .help("does nothing (for compatibility)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("noprofile")
                .long("noprofile")
                .help("does nothing (for compatibility)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("norc")
                .long("norc")
                .help("does nothing (for compatibility)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("posix")
                .long("posix")
                .help("does nothing (for compatibility)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("rcfile")
                .long("rcfile")
                .value_name("file")
                .help("does nothing (for compatibility)"),
        )
        .arg(
            Arg::new("restricted")
                .short('r')
                .long("restricted")
                .help("does nothing (for compatibility)")
                .action(ArgAction::SetTrue),
        )
}
