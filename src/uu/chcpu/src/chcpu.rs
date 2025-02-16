// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Command};
use uucore::error::UResult;

#[uucore::main]
pub fn uumain(_args: impl uucore::Args) -> UResult<()> {
    println!("chcpu: Hello world");
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .infer_long_args(true)
}
