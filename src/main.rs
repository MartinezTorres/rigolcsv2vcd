use csv;
use bitvec::prelude::*;
use serde::{self, Deserialize, Serialize};
use std::{error::Error, fs::File, io::BufWriter, path::PathBuf};

use vcd::{ self, Value, TimescaleUnit };
use std::io;

#[derive(Debug, Deserialize)]
struct RigolCSV {
    #[serde(rename = "Time(s)")]
    timestamp: String, // TODO: This field can be missing :/
    #[serde(rename = "D7-D0")]
    d7_d0: String, // TODO: Unfortunately those fields are "user-flippable" in order from the scope, i.e: d0_d7 vs d7_d0
    #[serde(rename = "D15-D8")]
    d15_d8: String,
}

#[derive(Debug, Serialize)]
struct RigolDataSeries {
    timestamp: f32,
    signals: u16,
}
struct Values {
    inner: BitVec<Value>
}

impl From<u16> for Values {
    fn from(v: u16) -> Values {
        let bitslice = bitvec![u16, Lsb0; 0; 16];
        for bit in bitslice {
            if bit {
                bitslice.push(Value::V1);
            } else {
                bitslice.push(Value::V0);
            }
        }
        Values { inner: bitslice }
    }
}

fn read_rigol_csv() -> Result<Vec<RigolDataSeries>, Box<dyn Error>> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true) // ignore broken header
        .from_reader(io::stdin());

    // TODO: Handle CSV when timestamps are in each row (enum/option)
    // Initial timestamp...
    let header = rdr.headers()?.clone();
    dbg!(&header);
    let t0_header: Vec<&str> = header[3].split('=').collect();
    let t0 = t0_header[1].trim_start().replace('s', "").parse::<f32>()?;
    // ...and increments
    let tinc_header: Vec<&str> = header[4].split('=').collect();
    let tinc = tinc_header[1].trim_start().parse::<f32>()?;
    println!("Initial timestamp {t0} with increments of {tinc} seconds");

    let mut _t_now: f32;
    let mut t_csv: f32;

    let mut signals: Vec<RigolDataSeries> = vec![];

    for row in rdr.deserialize().skip(1) {
        let record: RigolCSV = row?;
        // Compare t0+tinc vs timestamp divergence
        _t_now = t0 + tinc;
        t_csv = record.timestamp.parse::<f32>()?;
        // Parse digital signal groups
        let d_group_low = record.d7_d0.parse::<f32>()?;
        let d_group_high = record.d15_d8.parse::<f32>()?;

        // https://stackoverflow.com/questions/19507730/how-do-i-parse-a-string-to-a-list-of-floats-using-functional-style
        // https://stackoverflow.com/a/50244328/457116
        let d_all = ((d_group_high as u16) << 8) | d_group_low as u16;
        signals.push(RigolDataSeries { timestamp:t_csv, signals: d_all });
        //assert_eq!(t_now, t_csv);
        //println!("{:b}", d_all);
    }
    Ok(signals)
}

fn write_vcd(f: PathBuf, sigs: Vec<RigolDataSeries>) -> Result<(), Box<dyn Error>> {
    let buf = BufWriter::new(File::create(f)?);
    let mut writer = vcd::Writer::new(buf);

    // Write the header
    writer.timescale(1, TimescaleUnit::US)?;
    writer.add_module("top")?;
    let data = writer.add_wire(16, "data")?;
    writer.upscope()?;
    writer.enddefinitions()?;
  
    // // Write the initial values
    // writer.begin(SimulationCommand::Dumpvars)?;
    // writer.change_vector(data, &[sigs])?;
    // writer.end()?;
  
    // Write the data values
    for s in sigs {
      writer.timestamp(s.timestamp as u64)?;
      writer.change_vector(data, Values::from(s.signals))?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let sigs = read_rigol_csv()?;
    write_vcd(PathBuf::from("data/test.vcd"), sigs)?;
    Ok(())
}