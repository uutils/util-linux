// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Remove this if the tool is ported to Non-UNIX platforms.

use clap::{Command, crate_version};
#[cfg(target_os = "linux")]
#[cfg(target_os = "linux")]
use uucore::{error::UResult, format_usage, help_about, help_usage};

const ABOUT: &str = help_about!("kill.md");
const USAGE: &str = help_usage!("kill.md");

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches = uu_app().try_get_matches_from(args)?;

    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
}
