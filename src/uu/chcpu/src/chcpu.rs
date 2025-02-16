// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use clap::{crate_version, Arg, ArgAction, ArgGroup, Command};
use uucore::{error::UResult, format_usage, help_about, help_usage};

fn enable_cpus(cpu_list: &str) {
    println!("Enabling CPUs: {}", cpu_list)
}

fn disable_cpus(cpu_list: &str) {
    println!("Disabling CPUs: {}", cpu_list)
}

fn configure_cpus(cpu_list: &str) {
    println!("Configuring CPUs: {}", cpu_list)
}

fn deconfigure_cpus(cpu_list: &str) {
    println!("Deconfiguring CPUs: {}", cpu_list)
}

fn set_dispatch_mode(mode: &str) {
    println!("Setting dispatch mode to: {}", mode)
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app().try_get_matches_from(args)?;

    if let Some(cpu_list) = matches.get_one::<String>(options::ENABLE) {
        enable_cpus(cpu_list);
    }

    if let Some(cpu_list) = matches.get_one::<String>(options::DISABLE) {
        disable_cpus(cpu_list);
    }

    if let Some(cpu_list) = matches.get_one::<String>(options::CONFIGURE) {
        configure_cpus(cpu_list);
    }

    if let Some(cpu_list) = matches.get_one::<String>(options::DECONFIGURE) {
        deconfigure_cpus(cpu_list);
    }

    if let Some(mode) = matches.get_one::<String>(options::DISPATCH) {
        set_dispatch_mode(mode);
    }

    Ok(())
}

mod options {
    pub const ENABLE: &str = "enable";
    pub const DISABLE: &str = "disable";
    pub const CONFIGURE: &str = "configure";
    pub const DECONFIGURE: &str = "deconfigure";
    pub const DISPATCH: &str = "dispatch";
}

const ABOUT: &str = help_about!("chcpu.md");
const USAGE: &str = help_usage!("chcpu.md");

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::ENABLE)
                .short('e')
                .long("enable")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::DISABLE)
                .short('d')
                .long("disable")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::CONFIGURE)
                .short('c')
                .long("configure")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::DECONFIGURE)
                .short('g')
                .long("deconfigure")
                .value_name("cpu-list")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::DISPATCH)
                .short('p')
                .long("dispatch")
                .value_name("mode")
                .action(ArgAction::Set),
        )
        .group(
            ArgGroup::new("action")
                .args(vec![
                    options::ENABLE,
                    options::DISABLE,
                    options::CONFIGURE,
                    options::DECONFIGURE,
                    options::DISPATCH,
                ])
                .multiple(false) // These 5 are mutually exclusive
                .required(true),
        )
}
