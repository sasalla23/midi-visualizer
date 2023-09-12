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

#[derive(Debug, Clone,Copy,PartialEq, Eq)]
enum Key {
    C = 0, CS, D, DS, E, F, FS, G, GS, A, AS, B, KEY_COUNT
}

impl TryFrom<u8> for Key {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::C),
            1 => Ok(Self::CS),
            2 => Ok(Self::D),
            3 => Ok(Self::DS),
            4 => Ok(Self::E),
            5 => Ok(Self::F),
            6 => Ok(Self::FS),
            7 => Ok(Self::G),
            8 => Ok(Self::GS),
            9 => Ok(Self::A),
            10 => Ok(Self::AS),
            11 => Ok(Self::B),
            _ => Err(String::from("Invalid Value for key"))
        }
    }
}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
enum Division {
    TicksPerQuarter(u32), // Ticks per quarter node
    TicksPerFrame(u32,u32) // (FPS,ticks per frame)
}

#[derive(Debug)]
enum MetaEvent {
    SequenceTrackName { text: String },
    SetTempo { tempo: u32 },
    TimeSignature { denominator: u8, numerator: u8, metronome_clocks: u8, notated_32s_per_quarter: u8 },
    Text { text: String },
    EndOfTrack,
    Unknown
}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
enum Program {
    AcousticGrandPiano,
    SynthDrum,
    SynthBass2,
    Pad3,
    ElectricGuitarMuted,
    ElectricGuitarClean,
    BrassSection,
    StringEnsemble2,
    Lead3,
    VoiceOohs,
    AltoSax,
}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
enum ControllerMessage {
    ResetAllControllers,
    BankSelectMSB(u8),
    BankSelectLSB(u8),
    ChannelVolumeMSB(u8),
    PanMSB(u8),
    ExpressionControllerMSB(u8),
    EffectsDepth1LSB(u8),
    EffectsDepth3LSB(u8),
    RegisteredParameterNumberMSB(u8),
    RegisteredParameterNumberLSB(u8),
    DataEntryMSB(u8),
}

#[derive(Debug)]
enum MidiEvent {
    ControlChange(ControllerMessage),
    ProgramChange(Program),
    PitchWheelChange(u32),
    NoteOn { octave: i8, key: Key, velocity: u8 },
    NoteOff { octave: i8, key: Key, velocity: u8 }
}

#[derive(Debug)]
enum Chunk {
    Header { format: Format, ntrks: u32, division: Division },
    Track { events: Vec<(u32, Event)> },
    Unknown,
    None
}

#[derive(Debug)]
enum Event {
    Midi(u8,MidiEvent),
    Sysex,
    Meta(MetaEvent)
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
    let mut data = [0];
    reader.read(&mut data).ok()?;
    let signal = data[0];
    //println!("Signal Byte: {:X}", signal);
    if signal == 0xFF { // META EVENT
        reader.read(&mut data).ok()?;
        let meta_type = data[0];
        
        let length = read_vlq(reader)?; // This should always work though the documentation is unclear
        //println!("Meta Type: {:X}, Length: {}", meta_type, length);
        match meta_type {
            0x03 => { // Sequence/Track Name
                let mut name_data = vec![0; length as usize];
                reader.read(&mut name_data[..]).ok()?;
                let name_str = std::str::from_utf8(&name_data).ok()?;
                //println!("Track Name: {}", name_str);
                Some((dt, Event::Meta(MetaEvent::SequenceTrackName { text: name_str.to_string() })))

            }
            0x51 => { // Set Tempo
                assert!(length == 3);
                let mut tempo_data = vec![0;3];
                reader.read(&mut tempo_data).ok()?;
                tempo_data.insert(0,0);
                let tempo = u32::from_be_bytes(tempo_data.try_into().unwrap());
                Some((dt, Event::Meta(MetaEvent::SetTempo { tempo })))
            },
            0x58 => { // Time Signature
                assert!(length == 4);
                let mut time_sig_data = [0;4];
                reader.read(&mut time_sig_data).ok()?;
                let denominator = time_sig_data[0];
                let numerator = 2_u8.pow(time_sig_data[1] as u32);
                let metronome_clocks = time_sig_data[2];
                let notated_32s_per_quarter = time_sig_data[3];
                Some((dt, Event::Meta(MetaEvent::TimeSignature { denominator, numerator, metronome_clocks, notated_32s_per_quarter })))
            }
            0x01 => { // Text
                let mut text_data = vec![0; length as usize];
                reader.read(&mut text_data[..]).ok()?;
                let text_str = std::str::from_utf8(&text_data).ok()?;
                Some((dt, Event::Meta(MetaEvent::Text { text: text_str.to_string() })))
            }
            _ => {
                let mut unknown_data = vec![0; length as usize];
                reader.read(&mut unknown_data).ok()?;
                Some((dt,Event::Meta(MetaEvent::Unknown)))
            }
        }

    } else if signal >> 4 == 0xF { // Sysex
        match signal {
            0xF0 => {
                let mut ignored_data = [0];
                loop {
                    if reader.read(&mut ignored_data).ok()? == 0 { return None };
                    if ignored_data[0] >> 7 == 1 {
                        break;
                    }
                }
                Some((dt, Event::Sysex))
            }
            _ => todo!()
        }
    } else { // Midi
        let midi_event_type = signal >> 4;
        let channel = signal & 0xF;
        match midi_event_type {
            0b1011 => { // Control Change
                let mut control_change_data = [0;2];
                reader.read(&mut control_change_data).ok()?;
                Some((dt, match control_change_data[0] {
                    0x79 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::ResetAllControllers)),
                    0x00 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::BankSelectMSB(
                        control_change_data[1]
                    ))),
                    0x20 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::BankSelectLSB(
                        control_change_data[1]
                    ))),
                    0x07 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::ChannelVolumeMSB(
                        control_change_data[1]
                    ))),
                    0x0A => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::PanMSB(
                        control_change_data[1]
                    ))),
                    0x0B => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::ExpressionControllerMSB(
                        control_change_data[1]
                    ))),
                    0x5B => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::EffectsDepth1LSB(
                        control_change_data[1]
                    ))),
                    0x5D => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::EffectsDepth3LSB(
                        control_change_data[1]
                    ))),
                    0x65 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::RegisteredParameterNumberMSB(
                        control_change_data[1]
                    ))),
                    0x64 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::RegisteredParameterNumberLSB(
                        control_change_data[1]
                    ))),
                    0x06 => Event::Midi(channel, MidiEvent::ControlChange(ControllerMessage::DataEntryMSB(
                        control_change_data[1]
                    ))),
                    _ => todo!("{}", control_change_data[0])
                }))
            },
            0b1100 => { // Program Change
                let mut program_data = [0];
                reader.read(&mut program_data).ok()?;
                match program_data[0] { // Maybe use predefined array/hashmap/enum
                    0 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::AcousticGrandPiano)))),
                    118 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::SynthDrum)))),
                    39 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::SynthBass2)))),
                    90 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::Pad3)))),
                    28 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::ElectricGuitarMuted)))),
                    27 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::ElectricGuitarClean)))),
                    61 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::BrassSection)))),
                    48 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::StringEnsemble2)))),
                    82 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::Lead3)))),
                    53 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::VoiceOohs)))),
                    65 => Some((dt, Event::Midi(channel,MidiEvent::ProgramChange(Program::AltoSax)))),
                    _ => todo!("{}", program_data[0])
                }
            },
            0b1110 => { // Pitch Wheel Change
                let mut pitch_change_data = [0;2];
                reader.read(&mut pitch_change_data).ok()?;
                let pitch = ((pitch_change_data[1] as u32) << 7) | (pitch_change_data[0] as u32);
                //println!("{:b} {:b} {:b}", pitch, pitch_change_data[0], pitch_change_data[1]);
                Some((dt,Event::Midi(channel, MidiEvent::PitchWheelChange(pitch))))
            }
            0b1001 => { // Note On
                let mut note_data = [0;2];
                reader.read(&mut note_data).ok()?;
                let octave = (note_data[0] / Key::KEY_COUNT as u8) as i8 - 1;
                let key: Key = (note_data[0] % Key::KEY_COUNT as u8).try_into().unwrap();
                //println!("{}, {}, {}, {:?}", channel, note_data[0], octave, key);
                Some((dt,Event::Midi(channel, MidiEvent::NoteOn { octave, key, velocity: note_data[1] })))
            }
            0b1000 => {
                let mut note_data = [0;2];
                reader.read(&mut note_data).ok()?;
                let octave = (note_data[0] / Key::KEY_COUNT as u8) as i8 - 1;
                let key: Key = (note_data[0] % Key::KEY_COUNT as u8).try_into().unwrap();

                Some((dt,Event::Midi(channel, MidiEvent::NoteOff { octave, key, velocity: note_data[1] })))
            }
            _ => todo!("{:b}", midi_event_type)
        }
    }
    
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
            let mut events = vec![];
            while let Some(event) = read_event(&mut track_reader) {
                events.push(event);
            }
            Chunk::Track { events }
        },
        Some(_) | None => {
            println!("WARNING: Unkown chunk type");
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            Chunk::Unknown
        }
    }
}

struct MidiFile {
    header: Chunk,
    tracks: Vec<Chunk>
}

impl MidiFile {
    fn read_midi(file_path: &str) -> io::Result<Self> {
        let mut file = File::open(file_path).unwrap();
        let header = read_chunk(&mut file);
        if let Chunk::Header {  ntrks , ..} = header {
            println!("{:?}", header);
            let mut tracks = vec![];
            for _ in 0..ntrks {
                let next_chunk = read_chunk(&mut file);
                if let Chunk::Track { .. } = next_chunk {
                    tracks.push(next_chunk);
                }
            }
            Ok(Self { header, tracks })
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Midi file has to start with header"));
        }
    }
}

use raylib::prelude::*;
use raylib::core::logging::set_trace_log;

fn draw_keyboard(d: &mut impl RaylibDraw, bounds: Rectangle, key_map: [bool;128]) {
    let white_key_count: usize = 75;
    let white_key_width = bounds.width / white_key_count as f32;
    let black_key_width = white_key_width / 2.0;
    let black_key_height = bounds.height * 0.75;
    let key_rect = |i|
        Rectangle::new(bounds.x + white_key_width * i as f32, bounds.y, white_key_width, bounds.height);
    
    let get_key_id = |i| {
        let white_key = i % 7;
        let mut key_id = (i / 7) * 12;
        key_id += white_key;
        if white_key > 0 { key_id += 1 }
        if white_key > 1 { key_id += 1 }
        if white_key > 3 { key_id += 1 }
        if white_key > 4 { key_id += 1 }
        if white_key > 5 { key_id += 1 }
        key_id
    };

    for i in 0..white_key_count {
        let key_id = get_key_id(i);
        let draw_color = if key_map[key_id] { Color::GREEN } else { Color::WHITE };

        let key_rect = key_rect(i);
        d.draw_rectangle_rec(key_rect, draw_color);
        d.draw_rectangle_lines_ex(key_rect, 1, Color::GRAY);
    }

    for i in 0..white_key_count {
        let key_id = get_key_id(i)+1;

        let white_key = i % 7;
        let key_rect = key_rect(i);
        if white_key != 2 && white_key != 6 && i != white_key_count-1 {
            
            let black_key_bounds =
                Rectangle::new(
                    key_rect.x + key_rect.width - black_key_width * 0.5,
                    bounds.y,
                    black_key_width,
                    black_key_height
                );
            let draw_color = if key_map[key_id] { Color::GREEN } else { Color::BLACK };
            d.draw_rectangle_rec(black_key_bounds, draw_color);
        }
    }
}

fn main() -> std::io::Result<()> {
    let file =  MidiFile::read_midi("Never-Gonna-Give-You-Up-3.mid")?;
    
    const WINDOW_WIDTH: i32 = 1920;
    const WINDOW_HEIGHT: i32 = 1080;

    set_trace_log(TraceLogLevel::LOG_NONE);
    let (mut rl, thread) = raylib::init().width(WINDOW_WIDTH).height(WINDOW_HEIGHT).title("Mididi").build();
    rl.set_exit_key(None);
    

    let mut key_map  = [false; 128];
    key_map[60] = true;
    key_map[63] = true;
    key_map[67] = true;
    while !rl.window_should_close() {
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_text("Hello World", 23, 23, 23, Color::RAYWHITE);
        let key_board_bounds = Rectangle::new(0.0, WINDOW_HEIGHT as f32 * 7.0/8.0, WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32 / 8.0);
        draw_keyboard(&mut d, key_board_bounds, key_map);
    }

    Ok(())
    
}
