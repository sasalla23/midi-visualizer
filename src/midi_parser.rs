
use std::io::Read;
use std::fmt::Display;
use std::str;
use std::fs::File;

#[derive(Debug, Clone,Copy,PartialEq, Eq)]
pub enum Format {
    SingleTrack,
    SimulTrack,
    SequenceTrack
}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
pub enum Division {
    TicksPerQuarter(u32), // Ticks per quarter node
    TicksPerFrame(u32,u32) // (FPS,ticks per frame)
}

#[derive(Debug, Clone)]
pub enum MetaEvent {
    SequenceTrackName { text: String },
    SetTempo { tempo: u32 },
    TimeSignature { denominator: u8, numerator: u8, metronome_clocks: u8, notated_32s_per_quarter: u8 },
    Text { text: String },
    EndOfTrack,
    Unknown
}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
pub enum ControllerMessage {
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
    NonRegisteredParameterNumberMSB(u8),
    NonRegisteredParameterNumberLSB(u8),
    DataEntryMSB(u8),
    DamperPedalOn(bool),
    AllSoundOff,
    AllNotesOff,
    ModulationWheel(u8),
    BreathControlMSB(u8),
    PortamentoOnOff(bool),
    PortamentoTimeMSB(u8),
    PolyModeOnOffAllNotesOff,
    GeneralPurposeController1MSB(u8),
    DataEntryLSB(u8),
}

#[derive(Debug,Clone)]
pub enum MidiEvent {
    ControlChange(ControllerMessage),
    ProgramChange(u8),
    PitchWheelChange(u32),
    NoteOn { key: u8, velocity: u8 },
    NoteOff { key: u8, velocity: u8 }
}

#[derive(Debug, Clone)]
pub struct HeaderChunk {
    pub format: Format,
    pub ntrks: u32,
    pub division: Division
}

#[derive(Debug,Clone)]
pub struct TrackChunk { 
    pub events: Vec<(u32, Event)>
}

#[derive(Debug, Clone)]
pub enum Chunk {
    Header(HeaderChunk),
    Track(TrackChunk),
    Unknown
}

#[derive(Debug,Clone)]
pub enum Event {
    Midi(u8,MidiEvent),
    Sysex,
    Meta(MetaEvent)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum MidiErrorType {
    IO,
    InvalidMidi
}

#[derive(Debug, Clone)]
pub struct MidiError {
    pub message: String,
    pub error_type: MidiErrorType
}

impl Display for MidiError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(fmt, "{:?}Error: {}", self.error_type, self.message)
    }
}

impl std::error::Error for MidiError {}



fn read_bytes(reader: &mut impl Read, buf: &mut [u8]) -> Result<(),MidiError> {
    match reader.read(buf) {
        Ok(length) => {
            if length == 0 {
                Err(MidiError { message: String::from("Unexpectedly reached end of file"), error_type: MidiErrorType::InvalidMidi })
            } else {
                Ok(())
            }
        },
        Err(err) => {
            Err(MidiError { message: err.to_string(), error_type: MidiErrorType::IO })
        }
    }
}

// Read a variable length quantity as used in midi files
fn read_vlq(reader: &mut impl Read) -> Result<u32,MidiError> {
    let mut out = 0;
    let mut buffer = [0];
    
    loop {
        read_bytes(reader, &mut buffer)?;
        out <<= 7;
        if buffer[0] & (1<<7) == 0 {
            out |= buffer[0] as u32;
            break;
        } else {
            buffer[0] ^= 1 << 7;
            out |= buffer[0] as u32;
        }
    }
    return Ok(out);
}

fn read_midi_event(reader: &mut impl Read, midi_event_type: u8, first_byte: u8) -> Result<MidiEvent,MidiError> {
    Ok(match midi_event_type {
        0b1011 => { // Control Change
            let mut control_change_data = [0];
            read_bytes(reader,&mut control_change_data)?;
            MidiEvent::ControlChange(match first_byte {
                0x79 => ControllerMessage::ResetAllControllers,
                0x00 => ControllerMessage::BankSelectMSB(control_change_data[0]),
                0x20 => ControllerMessage::BankSelectLSB(control_change_data[0]),
                0x07 => ControllerMessage::ChannelVolumeMSB(control_change_data[0]),
                0x0A => ControllerMessage::PanMSB(control_change_data[0]),
                0x0B => ControllerMessage::ExpressionControllerMSB(control_change_data[0]),
                0x5B => ControllerMessage::EffectsDepth1LSB(control_change_data[0]),
                0x5D => ControllerMessage::EffectsDepth3LSB(control_change_data[0]),
                0x63 => ControllerMessage::NonRegisteredParameterNumberMSB(control_change_data[0]),
                0x62 => ControllerMessage::NonRegisteredParameterNumberLSB(control_change_data[0]),
                0x65 => ControllerMessage::RegisteredParameterNumberMSB(control_change_data[0]),
                0x64 => ControllerMessage::RegisteredParameterNumberLSB(control_change_data[0]),
                0x06 => ControllerMessage::DataEntryMSB(control_change_data[0]),
                0x40 => ControllerMessage::DamperPedalOn(control_change_data[0] >= 64),
                0x78 => ControllerMessage::AllSoundOff,
                0x7B => ControllerMessage::AllNotesOff,
                0x01 => ControllerMessage::ModulationWheel(control_change_data[0]),
                0x02 => ControllerMessage::BreathControlMSB(control_change_data[0]),
                0x41 => ControllerMessage::PortamentoOnOff(control_change_data[0] >= 64),
                0x05 => ControllerMessage::PortamentoTimeMSB(control_change_data[0]),
                0x7E => ControllerMessage::PolyModeOnOffAllNotesOff,
                0x12 => ControllerMessage::GeneralPurposeController1MSB(control_change_data[0]),
                0x26 => ControllerMessage::DataEntryLSB(control_change_data[0]),
                _ => todo!("{}", first_byte)
            })
        },
        0b1100 => { // Program Change
            MidiEvent::ProgramChange(first_byte)
        },
        0b1110 => { // Pitch Wheel Change
            let mut pitch_change_data = [0];
            read_bytes(reader,&mut pitch_change_data)?;
            let pitch = ((pitch_change_data[0] as u32) << 7) | (first_byte as u32);
            MidiEvent::PitchWheelChange(pitch)
        }
        0b1001 => { // Note On
            let mut note_data = [0];
            read_bytes(reader,&mut note_data)?;
            MidiEvent::NoteOn { key: first_byte, velocity: note_data[0] }
        }
        0b1000 => { // Note Off
            let mut note_data = [0];
            read_bytes(reader,&mut note_data)?;
            MidiEvent::NoteOff { key: first_byte, velocity: note_data[0] }
        }
        _ => todo!("Midi type: {:b}", midi_event_type)
    })
}

fn get_utf8(data: &Vec<u8>) -> Result<String,MidiError> {
    let name_str = str::from_utf8(data).map_err(|x| MidiError { message: x.to_string(), error_type: MidiErrorType::InvalidMidi })?;
    Ok(name_str.to_string())
}

fn read_event(reader: &mut impl Read, last_status: &mut u8) -> Result<(u32, Event), MidiError> {
    let dt = read_vlq(reader)?;
    Ok((dt, {
        let mut data = [0];
        read_bytes(reader, &mut data)?;
        let signal = data[0];
        if signal == 0xFF { // META EVENT
            read_bytes(reader, &mut data)?;
            let meta_type = data[0];
            let length = read_vlq(reader)?; // This should always work though the documentation is unclear
            Event::Meta(match meta_type {
                0x03 => { // Sequence/Track Name
                    let mut name_data = vec![0; length as usize];
                    read_bytes(reader, &mut name_data)?;
                    let name_str = get_utf8(&name_data)?;
                    MetaEvent::SequenceTrackName { text: name_str }

                }
                0x51 => { // Set Tempo
                    assert!(length == 3);
                    let mut tempo_data = vec![0;3];
                    read_bytes(reader, &mut tempo_data)?;
                    tempo_data.insert(0,0);
                    let tempo = u32::from_be_bytes(tempo_data.try_into().unwrap());
                    MetaEvent::SetTempo { tempo }
                },
                0x58 => { // Time Signature
                    assert!(length == 4);
                    let mut time_sig_data = [0;4];
                    read_bytes(reader, &mut time_sig_data)?;
                    let denominator = time_sig_data[0];
                    let numerator = 2_u8.pow(time_sig_data[1] as u32);
                    let metronome_clocks = time_sig_data[2];
                    let notated_32s_per_quarter = time_sig_data[3];
                    MetaEvent::TimeSignature { denominator, numerator, metronome_clocks, notated_32s_per_quarter }
                }
                0x01 => { // Text
                    let mut text_data = vec![0; length as usize];
                    read_bytes(reader, &mut text_data[..])?;
                    let text_str = get_utf8(&text_data)?;
                    MetaEvent::Text { text: text_str }
                },
                0x2F => {
                    MetaEvent::EndOfTrack
                }
                _ => { // Unknown/Not implemented
                    let mut unknown_data = vec![0; length as usize];
                    read_bytes(reader, &mut unknown_data)?;
                    println!("Unknown Meta Event: {:X}", meta_type);
                    MetaEvent::Unknown
                }
            })

        } else if signal >> 4 == 0xF { // Sysex
            match signal {
                0xF0 => {
                    let mut ignored_data = [0];
                    loop {
                        read_bytes(reader, &mut ignored_data)?;
                        if ignored_data[0] == 0b11110111 {
                            break;
                        }
                    }
                    Event::Sysex
                }
                _ => todo!()
            }
        } else if signal & 0b1000_0000 != 0 { // Midi
            let midi_event_type = signal >> 4;
            let channel = signal & 0xF;
            let mut first_byte_data = [0];
            read_bytes(reader, &mut first_byte_data)?;
            *last_status = signal;
            Event::Midi(channel, read_midi_event(reader, midi_event_type, first_byte_data[0])?)
        } else { // Signal Byte was omitted
            let midi_event_type = *last_status >> 4;
            let channel = *last_status & 0xF;
            Event::Midi(channel, read_midi_event(reader, midi_event_type, signal)?)
        }
    }))
}

fn read_chunk(reader: &mut impl Read) -> Result<Chunk, MidiError> {
    let mut name_bytes = [0_u8;4];
    read_bytes(reader,&mut name_bytes)?;

    let mut name = None;
    if let Ok(s) = str::from_utf8(&name_bytes) {
        name = Some(String::from(s));
    };

    let mut length_bytes = [0_u8;4];
    read_bytes(reader, &mut length_bytes)?;
    let length = u32::from_be_bytes(length_bytes);
    
    Ok(match name {
        Some(s) if s == "MThd" => {
            assert!(length == 6);
            let mut data = [0_u8;2];
            read_bytes(reader,&mut data)?;
            let format = match u16::from_be_bytes(data.clone()) {
                0 => Ok(Format::SingleTrack),
                1 => Ok(Format::SimulTrack),
                2 => Ok(Format::SequenceTrack),
                x => {
                    Err(MidiError { message: format!("Unknown midi format {}", x), error_type: MidiErrorType::InvalidMidi })
                }
            }?;

            read_bytes(reader,&mut data)?;
            let ntrks = u16::from_be_bytes(data.clone()) as u32;
            read_bytes(reader,&mut data)?;
            let division = if data[0] >> 7 == 0 {
                Division::TicksPerQuarter(u16::from_be_bytes(data) as u32)
            } else {
                let frame_rate = i8::from_be_bytes([data[0]]);
                let ticks_per_frame = data[1];
                Division::TicksPerFrame((-frame_rate) as u32, ticks_per_frame as u32)
            };

            Chunk::Header(HeaderChunk { format, ntrks, division })
        },
        Some(s) if s == "MTrk"=> {
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            let mut track_reader = content.as_slice();
            let mut events = vec![];
            let mut stored_signal = 0;
            loop {
                let event = read_event(&mut track_reader, &mut stored_signal)?;
                if let (_,Event::Meta(MetaEvent::EndOfTrack)) = event {
                    events.push(event);
                    break;
                } else {
                    events.push(event);
                }
                
                
            }
            Chunk::Track(TrackChunk { events } )
        },
        Some(_) | None => {
            println!("WARNING: Unkown chunk type");
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            Chunk::Unknown
        }
    })
}

#[derive(Clone)]
pub struct MidiFile {
    pub header: HeaderChunk,
    pub tracks: Vec<TrackChunk>,
}

impl MidiFile {
    pub fn read_midi(file_path: &str) -> Result<Self,MidiError> {
        let mut file = File::open(file_path).map_err(|e|
            MidiError { message: e.to_string(), error_type: MidiErrorType::IO }
        )?;
        let header_chunk = read_chunk(&mut file)?;
        if let Chunk::Header(header) = header_chunk {
            let mut tracks = vec![];
            for _ in 0..header.ntrks {
                let next_chunk = read_chunk(&mut file)?;
                if let Chunk::Track(track) = next_chunk {
                    tracks.push(track);
                }
            }
            Ok(Self { header, tracks })
        } else {
            Err(MidiError { message: String::from("A midi file has to start with a header chunk"), error_type: MidiErrorType::InvalidMidi })
        }
    }

    pub fn get_simult_track_count(&self) -> usize {
        match self.header.format {
            Format::SingleTrack => 1,
            Format::SimulTrack => self.header.ntrks as usize,
            Format::SequenceTrack => 1,
        }
    }
}