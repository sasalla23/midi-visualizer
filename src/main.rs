use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::str;
use std::io;

#[derive(Debug, Clone,Copy,PartialEq, Eq)]
enum Format {
    SINGLE_TRACK,
    SIMUL_TRACK,
    SEQUENCE_TRACK
}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
enum Division {
    TicksPerQuarter(u32),
    TicksPerFrame(u32,u32)
}

#[derive(Debug)]
enum Chunk {
    Header { format: Format, ntrks: u32, division: Division },
    Track,
    Unknown,
    None
}

enum Event {
    MidiEvent,
    SysexEvent,
    MetaEvent
}

fn read_vlq(reader: &mut impl Read) -> Option<u32> {
    let mut out = 0;
    let mut buffer = [0];
    
    loop {
        if reader.read(&mut buffer).unwrap() == 0 { return None };
        out <<= 7;
        if buffer[0] & (1<<7) == 0 {
            out |= buffer[0] as u32;
            break;
        } else {
            buffer[0] ^= 1 << 7;
            out |= buffer[0] as u32;
        }
    }
    return Some(out);
}

fn read_event(reader: &mut impl Read) -> Option<(u32, Event)> {
    let dt = read_vlq(reader)?;
    todo!();
}

fn read_chunk(reader: &mut impl Read) -> Chunk {
    println!("==== Chunk ====");
    let mut name_bytes = [0_u8;4];
    let bytes = reader.read(&mut name_bytes).unwrap();
    if bytes == 0 {
        return Chunk::None;
    }

    let mut name = None;
    if let Ok(s) = str::from_utf8(&name_bytes) {
        name = Some(String::from(s));
    };
    
    

    let mut length_bytes = [0_u8;4];
    reader.read(&mut length_bytes).unwrap();
    let length = u32::from_be_bytes(length_bytes);

    println!("Length: {}", length);
    
    match name {
        Some(s) if s == "MThd" => {
            println!("HEADER CHUNK!");
            assert!(length == 6);
            let mut data = [0_u8;2];
            reader.read(&mut data).unwrap();
            let format = match u16::from_be_bytes(data.clone()) {
                0 => Format::SINGLE_TRACK,
                1 => Format::SIMUL_TRACK,
                2 => Format::SEQUENCE_TRACK,
                x => {
                    eprintln!("Unknown format {}", x);
                    return Chunk::None
                }
            };

            reader.read(&mut data).unwrap();
            let ntrks = u16::from_be_bytes(data.clone()) as u32;
            reader.read(&mut data).unwrap();
            let division = if data[0] >> 7 == 0 {
                Division::TicksPerQuarter(u16::from_be_bytes(data) as u32)
            } else {
                let frame_rate = i8::from_be_bytes([data[0]]);
                let ticks_per_frame = data[1];
                Division::TicksPerFrame((-frame_rate) as u32, ticks_per_frame as u32)
            };

            Chunk::Header { format, ntrks, division }
        },
        Some(s) if s == "MTrk"=> {
            println!("TRACK CHUNK!");
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            let mut track_reader = content.as_slice();
            let event = read_event(reader);
            todo!()
        },
        Some(_) | None => {
            println!("WARNING: Unkown chunk type");
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            Chunk::Unknown
        }
    }
}

fn read_midi(file_path: &str) -> io::Result<()> {
    let mut file = File::open(file_path).unwrap();
    let header = read_chunk(&mut file);
    if let Chunk::Header { .. } = header {
        println!("{:?}", header);
        let next_chunk = read_chunk(&mut file);
        todo!()
    } else {
        return Err(io::Error::new(io::ErrorKind::Other, "Midi file has to start with header"));
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    read_midi("Never-Gonna-Give-You-Up-3.mid")
    
}
