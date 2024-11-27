// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use serde::Serialize;
use std::io;

pub fn serialize_records(records: &Vec<crate::Record>) -> String {
    let json = Dmesg::from(records);
    let formatter = DmesgFormatter::new();
    let mut buf = vec![];
    let mut serializer = serde_json::Serializer::with_formatter(&mut buf, formatter);
    json.serialize(&mut serializer).unwrap();
    String::from_utf8_lossy(&buf).to_string()
}

#[derive(serde::Serialize)]
struct Dmesg<'a> {
    dmesg: Vec<Record<'a>>,
}

#[derive(serde::Serialize)]
struct Record<'a> {
    pri: u32,
    time: i64,
    msg: &'a str,
}

impl<'a> From<&'a Vec<crate::Record>> for Dmesg<'a> {
    fn from(value: &'a Vec<crate::Record>) -> Self {
        let mut dmesg_json = Dmesg { dmesg: vec![] };
        for record in value {
            let record_json = Record {
                pri: record.priority_facility,
                time: record.timestamp_us,
                msg: &record.message,
            };
            dmesg_json.dmesg.push(record_json);
        }
        dmesg_json
    }
}

struct DmesgFormatter {
    nesting_depth: i32,
}

impl DmesgFormatter {
    const SINGLE_INDENTATION: &[u8] = b"   ";

    fn new() -> Self {
        DmesgFormatter { nesting_depth: 0 }
    }

    fn write_indentation<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        for _ in 0..self.nesting_depth {
            writer.write_all(Self::SINGLE_INDENTATION)?;
        }
        Ok(())
    }
}

impl serde_json::ser::Formatter for DmesgFormatter {
    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.nesting_depth += 1;
        writer.write_all(b"{\n")
    }

    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b"\n")?;
        self.nesting_depth -= 1;
        self.write_indentation(writer)?;
        writer.write_all(b"}")
    }

    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        self.nesting_depth += 1;
        writer.write_all(b"[\n")
    }

    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b"\n")?;
        self.nesting_depth -= 1;
        self.write_indentation(writer)?;
        writer.write_all(b"]")
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if !first {
            writer.write_all(b",\n")?;
        }
        self.write_indentation(writer)
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writer.write_all(b": ")
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        if first {
            self.write_indentation(writer)
        } else {
            writer.write_all(b",")
        }
    }

    fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        // The only i64 field in Dmesg is time, which requires a specific format
        let seconds = value / 1000000;
        let sub_seconds = value % 1000000;
        let repr = format!("{:>5}.{:0>6}", seconds, sub_seconds);
        writer.write_all(repr.as_bytes())
    }
}
