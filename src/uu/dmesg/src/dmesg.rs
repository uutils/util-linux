use clap::{crate_version, Arg, ArgAction, Command};
use uucore::error::UResult;

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let mut dmesg = Dmesg::new();
    let matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;
    if let Some(kmsg_file) = matches.get_one::<String>(options::KMSG_FILE) {
        dmesg.kmsg_file = kmsg_file;
    }
    if matches.get_flag(options::JSON) {
        dmesg.output_format = OutputFormat::Json;
    }
    dmesg.parse().print();
    Ok(())
}

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .arg(
            Arg::new(options::KMSG_FILE)
                .short('K')
                .long("kmsg-file")
                .action(ArgAction::Set),
        )
        .arg(
            Arg::new(options::JSON)
                .short('J')
                .long("json")
                .action(ArgAction::SetTrue),
        )
}

mod options {
    pub const KMSG_FILE: &'static str = "kmsg-file";
    pub const JSON: &'static str = "json";
}

struct Dmesg<'a> {
    kmsg_file: &'a str,
    output_format: OutputFormat,
    _records: Option<Vec<Record>>,
}

impl Dmesg<'_> {
    fn new() -> Self {
        Dmesg {
            kmsg_file: "/dev/kmsg",
            output_format: OutputFormat::Normal,
            _records: None,
        }
    }

    fn parse(self) -> Self {
        self
    }

    fn print(&self) {

    }
}

enum OutputFormat {
    Normal,
    Json,
}

struct Record {
}
