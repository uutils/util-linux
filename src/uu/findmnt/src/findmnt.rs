use core::fmt;
use std::fs;

use clap::{crate_version, Command};
use tabled::{settings::Style, Table, Tabled};
use uucore::{error::UResult, help_about};

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let _matches: clap::ArgMatches = uu_app().try_get_matches_from(args)?;

    // By default findmnt reads /proc/self/mountinfo
    let mut res = Findmnt::new(MOUNTINFO_DIR);
    res.form_nodes();
    res.print_table();
    Ok(())
}

pub static MOUNTINFO_DIR: &str = "/proc/self/mountinfo";
pub static ABOUT: &str = help_about!("findmnt.md");

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
}

#[derive(Debug, Clone)]
pub struct Findmnt<'a> {
    pub nodes_vec: Vec<Node>,
    file_name: &'a str,
}

impl<'a> Findmnt<'a> {
    pub fn new(file_name: &str) -> Findmnt {
        Findmnt {
            file_name,
            nodes_vec: Vec::<Node>::new(),
        }
    }

    pub fn form_nodes(&mut self) {
        let res = fs::read_to_string(self.file_name).unwrap();
        let lines = res.lines();
        let mut unsorted_vec = Vec::<Node>::new();

        for line in lines {
            let res = Node::parse(line);
            unsorted_vec.push(res);
        }

        self.nodes_vec = unsorted_vec;
        // /, /proc, /sys, /dev, /run, /tmp, /boot
        // Sort the vec according to this
        self.sort_nodes();
    }

    pub fn print_table(&self) {
        let mut table = Table::new(self.nodes_vec.clone());
        table.with(Style::empty());
        print!("{}", table)
    }

    fn sort_nodes(&mut self) {
        let unsorted_vec = self.nodes_vec.clone();
        let mut sorted_vec = Vec::new();

        // "/"
        // This should always give one element
        let res = unsorted_vec
            .iter()
            .find(|node| node.target == Types::ROOT.to_string());
        sorted_vec.push(res.unwrap().clone());

        // "proc"
        sorted_vec.extend(self.filter(Types::PROC));

        // "/sys"
        sorted_vec.extend(self.filter(Types::SYS));

        // "/dev"
        sorted_vec.extend(self.filter(Types::DEV));

        // "/run"
        sorted_vec.extend(self.filter(Types::RUN));

        // "/tmp"
        sorted_vec.extend(self.filter(Types::TMP));

        // "/boot"
        sorted_vec.extend(self.filter(Types::BOOT));

        self.nodes_vec = sorted_vec;
    }

    fn filter(&self, pattern: Types) -> Vec<Node> {
        let mut temp_vec = Vec::<Node>::new();
        let _ = self.filter_with_pattern(pattern).iter().for_each(|node| {
            temp_vec.push(node.clone());
        });
        temp_vec
    }

    fn filter_with_pattern(&self, pattern: Types) -> Vec<Node> {
        self.nodes_vec
            .iter()
            .filter(|node| node.target.starts_with(&pattern.to_string()))
            .cloned()
            .collect()
    }
}

// Different types for a particular node
#[derive(Debug, Clone)]
pub enum Types {
    ROOT,
    PROC,
    SYS,
    DEV,
    RUN,
    TMP,
    BOOT,
}

impl fmt::Display for Types {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Types::ROOT => write!(f, "{}", "/"),
            Types::PROC => write!(f, "{}", "/proc"),
            Types::SYS => write!(f, "{}", "/sys"),
            Types::DEV => write!(f, "{}", "/dev"),
            Types::RUN => write!(f, "{}", "/run"),
            Types::TMP => write!(f, "{}", "/tmp"),
            Types::BOOT => write!(f, "{}", "/boot"),
        }
    }
}

// Represents each row for the table
#[derive(Debug, Clone, Tabled)]
pub struct Node {
    target: String,
    source: String,
    fstype: String,
    options: String,
}

impl Node {
    fn new(target: String, source: String, fstype: String, options: String) -> Node {
        Node {
            target,
            source,
            fstype,
            options,
        }
    }

    pub fn filter_with_pattern(node_vec: &Vec<Node>, pattern: Types) -> Vec<Node> {
        node_vec
            .iter()
            .filter(|node| node.target.starts_with(&pattern.to_string()))
            .cloned()
            .collect()
    }

    // This is the main function that parses the default /proc/self/mountinfo
    pub fn parse(line: &str) -> Self {
        let (_, rest) = line.split_once("/").unwrap();
        let (target, rest) = rest.trim().split_once(" ").unwrap();
        let (options, rest) = rest.trim().split_once(" ").unwrap();
        let (_, rest) = rest.trim().split_once("-").unwrap();
        let (fstype, rest) = rest.trim().split_once(" ").unwrap();
        let (source, rest) = rest.trim().split_once(" ").unwrap();
        let options_added = if let Some(_) = rest.split_once("rw") {
            rest.split_once("rw").unwrap().1
        } else {
            rest
        };

        let final_options = options.to_owned() + options_added;

        Self::new(
            target.to_string(),
            source.to_string(),
            fstype.to_string(),
            final_options,
        )
    }
}
