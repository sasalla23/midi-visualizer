use std::fs::File;
//use std::io::BufReader;
use std::io::prelude::*;
use std::str;
use std::io;
use std::sync::Arc;
//use std::collections::VecDeque;

#[derive(Debug, Clone,Copy,PartialEq, Eq)]
enum Format {
    SINGLE_TRACK,
    SIMUL_TRACK,
    SEQUENCE_TRACK
}

//#[derive(Debug, Clone,Copy,PartialEq, Eq)]
//enum Key {
//    C = 0, CS, D, DS, E, F, FS, G, GS, A, AS, B //, KEY_COUNT
//}
//
//fn key_index(key: Key, octave: i8) -> usize {
//    (octave + 1) as usize * 12 + key as usize
//}

//impl TryFrom<u8> for Key {
//    type Error = String;
//
//    fn try_from(value: u8) -> Result<Self, Self::Error> {
//        match value {
//            0 => Ok(Self::C),
//            1 => Ok(Self::CS),
//            2 => Ok(Self::D),
//            3 => Ok(Self::DS),
//            4 => Ok(Self::E),
//            5 => Ok(Self::F),
//            6 => Ok(Self::FS),
//            7 => Ok(Self::G),
//            8 => Ok(Self::GS),
//            9 => Ok(Self::A),
//            10 => Ok(Self::AS),
//            11 => Ok(Self::B),
//            _ => Err(String::from("Invalid Value for key"))
//        }
//    }
//}

#[derive(Debug, Clone,Copy,PartialEq,Eq)]
enum Division {
    TicksPerQuarter(u32), // Ticks per quarter node
    TicksPerFrame(u32,u32) // (FPS,ticks per frame)
}

#[derive(Debug, Clone)]
enum MetaEvent {
    SequenceTrackName { text: String },
    SetTempo { tempo: u32 },
    TimeSignature { denominator: u8, numerator: u8, metronome_clocks: u8, notated_32s_per_quarter: u8 },
    Text { text: String },
    EndOfTrack,
    Unknown
}

//#[derive(Debug, Clone,Copy,PartialEq,Eq)]
//enum Program {
//    AcousticGrandPiano,
//    SynthDrum,
//    SynthBass2,
//    Pad3,
//    Pad1,
//    ElectricGuitarMuted,
//    ElectricGuitarClean,
//    BrassSection,
//    StringEnsemble2,
//    StringEnsemble1,
//    Lead3,
//    VoiceOohs,
//    AltoSax,
//    ElectricPiano2,
//    Clavinet,
//    SynthBrass2,
//    SynthBrass1,
//    SynthStrings1,
//    FretlessBass,
//    Lead1,
//    PercussiveOrgan,
//    OrchestraHit,
//    SynthStrings2,
//    RockOrgan,
//    Lead2,
//    Trumpet,
//    ChoirAahs,
//    Viola,
//    Bassoon,
//    FrenchHorn,
//    Trombone,
//    Timpani,
//    Clarinet,
//    OrchestralHarp,
//    Piccolo
//}

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
enum MidiEvent {
    ControlChange(ControllerMessage),
    ProgramChange(u8),
    PitchWheelChange(u32),
    NoteOn { key: u8, velocity: u8 },
    NoteOff { key: u8, velocity: u8 }
}

#[derive(Debug, Clone)]
struct HeaderChunk {
    format: Format,
    ntrks: u32,
    division: Division
}

#[derive(Debug,Clone)]
struct TrackChunk { 
    events: Vec<(u32, Event)>
}

#[derive(Debug, Clone)]
enum Chunk {
    Header(HeaderChunk),
    Track(TrackChunk),
    Unknown,
    None
}

#[derive(Debug,Clone)]
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

fn read_midi_event(reader: &mut impl Read, midi_event_type: u8, first_byte: u8) -> Option<MidiEvent> {
    Some(match midi_event_type {
        0b1011 => { // Control Change
            let mut control_change_data = [0];
            reader.read(&mut control_change_data).ok()?;
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
            //MidiEvent::ProgramChange(match program_data[0] { // Maybe use predefined array/hashmap/enum
            //    0 => Program::AcousticGrandPiano,
            //    118 => Program::SynthDrum,
            //    39 => Program::SynthBass2,
            //    90 => Program::Pad3,
            //    28 => Program::ElectricGuitarMuted,
            //    27 => Program::ElectricGuitarClean,
            //    61 => Program::BrassSection,
            //    48 => Program::StringEnsemble1,
            //    82 => Program::Lead3,
            //    53 => Program::VoiceOohs,
            //    65 => Program::AltoSax,
            //    5  => Program::ElectricPiano2,
            //    49 => Program::StringEnsemble2,
            //    7 => Program::Clavinet,
            //    63 => Program::SynthBrass2,
            //    88 => Program::Pad1,
            //    62 => Program::SynthBrass1,
            //    50 => Program::SynthStrings1,
            //    35 => Program::FretlessBass,
            //    80 => Program::Lead1,
            //    17 => Program::PercussiveOrgan,
            //    55 => Program::OrchestraHit,
            //    51 => Program::SynthStrings2,
            //    18 => Program::RockOrgan,
            //    81 => Program::Lead2,
            //    56 => Program::Trumpet,
            //    52 => Program::ChoirAahs,
            //    42 => Program::Viola,
            //    70 => Program::Bassoon,
            //    60 => Program::FrenchHorn,
            //    57 => Program::Trombone,
            //    47 => Program::Timpani,
            //    71 => Program::Clarinet,
            //    46 => Program::OrchestralHarp,
            //    73 => Program::Piccolo,
            //    _ => Program::AcousticGrandPiano,
            //    //_ => todo!("{}", program_data[0])
            //})
        },
        0b1110 => { // Pitch Wheel Change
            let mut pitch_change_data = [0];
            reader.read(&mut pitch_change_data).ok()?;
            let pitch = ((pitch_change_data[0] as u32) << 7) | (first_byte as u32);
            MidiEvent::PitchWheelChange(pitch)
        }
        0b1001 => { // Note On
            let mut note_data = [0];
            reader.read(&mut note_data).ok()?;
            MidiEvent::NoteOn { key: first_byte, velocity: note_data[0] }
        }
        0b1000 => {
            let mut note_data = [0];
            reader.read(&mut note_data).ok()?;
            MidiEvent::NoteOff { key: first_byte, velocity: note_data[0] }
        }
        _ => todo!("Midi type: {:b}", midi_event_type)
    })
}

fn read_event(reader: &mut impl Read, last_status: &mut u8) -> Option<(u32, Event)> {
    let dt = read_vlq(reader)?;
    Some((dt, {
        let mut data = [0];
        reader.read(&mut data).ok()?;
        let signal = data[0];
        if signal == 0xFF { // META EVENT
            reader.read(&mut data).ok()?;
            let meta_type = data[0];
            let length = read_vlq(reader)?; // This should always work though the documentation is unclear
            Event::Meta(match meta_type {
                0x03 => { // Sequence/Track Name
                    let mut name_data = vec![0; length as usize];
                    reader.read(&mut name_data[..]).ok()?;
                    let name_str = std::str::from_utf8(&name_data).ok()?;
                    MetaEvent::SequenceTrackName { text: name_str.to_string() }

                }
                0x51 => { // Set Tempo
                    assert!(length == 3);
                    let mut tempo_data = vec![0;3];
                    reader.read(&mut tempo_data).ok()?;
                    tempo_data.insert(0,0);
                    let tempo = u32::from_be_bytes(tempo_data.try_into().unwrap());
                    MetaEvent::SetTempo { tempo }
                },
                0x58 => { // Time Signature
                    assert!(length == 4);
                    let mut time_sig_data = [0;4];
                    reader.read(&mut time_sig_data).ok()?;
                    let denominator = time_sig_data[0];
                    let numerator = 2_u8.pow(time_sig_data[1] as u32);
                    let metronome_clocks = time_sig_data[2];
                    let notated_32s_per_quarter = time_sig_data[3];
                    MetaEvent::TimeSignature { denominator, numerator, metronome_clocks, notated_32s_per_quarter }
                }
                0x01 => { // Text
                    let mut text_data = vec![0; length as usize];
                    reader.read(&mut text_data[..]).ok()?;
                    let text_str = std::str::from_utf8(&text_data).ok()?;
                    MetaEvent::Text { text: text_str.to_string() }
                },
                0x2F => {
                    let mut end_data = [0];
                    reader.read(&mut end_data).ok()?;
                    MetaEvent::EndOfTrack
                }
                _ => {
                    let mut unknown_data = vec![0; length as usize];
                    reader.read(&mut unknown_data).ok()?;
                    println!("Unknown Meta Event: {:X}", meta_type);
                    MetaEvent::Unknown
                }
            })

        } else if signal >> 4 == 0xF { // Sysex
            match signal {
                0xF0 => {
                    let mut ignored_data = [0];
                    loop {
                        if reader.read(&mut ignored_data).ok()? == 0 { return None };
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
            reader.read(&mut first_byte_data).ok()?;
            *last_status = signal;
            Event::Midi(channel, read_midi_event(reader, midi_event_type, first_byte_data[0])?)
        } else {
            let midi_event_type = *last_status >> 4;
            //if midi_event_type == 0 {
            //    println!("Last status: {:b}, current status: {:b}", *last_status, signal);
            //}
            let channel = *last_status & 0xF;
            Event::Midi(channel, read_midi_event(reader, midi_event_type, signal)?)
        }
    }))
}

fn read_chunk(reader: &mut impl Read) -> Chunk {
    //println!("==== Chunk ====");
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

    //println!("Length: {}", length);
    
    match name {
        Some(s) if s == "MThd" => {
            //println!("HEADER CHUNK!");
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

            Chunk::Header(HeaderChunk { format, ntrks, division })
        },
        Some(s) if s == "MTrk"=> {
            //println!("TRACK CHUNK!");
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            let mut track_reader = content.as_slice();
            let mut events = vec![];
            let mut stored_signal = 0;
            while let Some(event) = read_event(&mut track_reader, &mut stored_signal) {
                //println!("Event: {:?}", &event);
                events.push(event);
            }
            Chunk::Track(TrackChunk { events } )
        },
        Some(_) | None => {
            println!("WARNING: Unkown chunk type");
            let mut content = vec![0;length as usize];
            reader.read(&mut content).unwrap();
            Chunk::Unknown
        }
    }
}

#[derive(Clone)]
struct MidiFile {
    header: HeaderChunk,
    tracks: Vec<TrackChunk>,
}

impl MidiFile {
    fn read_midi(file_path: &str) -> io::Result<Self> {
        let mut file = File::open(file_path).unwrap();
        let header_chunk = read_chunk(&mut file);
        if let Chunk::Header(header) = header_chunk {
            //println!("{:?}", header);
            let mut tracks = vec![];
            for _ in 0..header.ntrks {
                let next_chunk = read_chunk(&mut file);
                if let Chunk::Track(track) = next_chunk {
                    tracks.push(track);
                }
            }
            Ok(Self { header, tracks })
        } else {
            return Err(io::Error::new(io::ErrorKind::Other, "Midi file has to start with header"));
        }
    }

    fn get_simult_track_count(&self) -> usize {
        match self.header.format {
            Format::SINGLE_TRACK => 1,
            Format::SIMUL_TRACK => self.header.ntrks as usize,
            Format::SEQUENCE_TRACK => 1,
        }
    }
}
const NOTE_FREQUENCIES: [f64;128] = [8.175798915643682, 
    8.661957218027228,
    9.17702399741896,
    9.722718241315002,
    10.300861153527157,
    10.913382232281341,
    11.562325709738543,
    12.249857374429633,
    12.978271799373253,
    13.749999999999964,
    14.567617547440275,
    15.43385316425384,
    16.351597831287375,
    17.323914436054462,
    18.35404799483793,
    19.44543648263001,
    20.601722307054324,
    21.826764464562697,
    23.1246514194771,
    24.49971474885928,
    25.956543598746517,
    27.499999999999947,
    29.13523509488056,
    30.867706328507698,
    32.703195662574764,
    34.647828872108946,
    36.708095989675876,
    38.890872965260044,
    41.20344461410867,
    43.65352892912541,
    46.24930283895422,
    48.99942949771858,
    51.913087197493056,
    54.999999999999915,
    58.270470189761156,
    61.73541265701542,
    65.40639132514957,
    69.29565774421793,
    73.4161919793518,
    77.78174593052012,
    82.40688922821738,
    87.30705785825087,
    92.4986056779085,
    97.99885899543722,
    103.82617439498618,
    109.99999999999989,
    116.54094037952237,
    123.4708253140309,
    130.8127826502992,
    138.59131548843592,
    146.83238395870364,
    155.56349186104035,
    164.81377845643485,
    174.61411571650183,
    184.9972113558171,
    195.99771799087452,
    207.65234878997245,
    219.9999999999999,
    233.08188075904488,
    246.94165062806198,
    261.6255653005985,
    277.182630976872,
    293.66476791740746,
    311.1269837220808,
    329.62755691286986,
    349.2282314330038,
    369.99442271163434,
    391.99543598174927,
    415.3046975799451,
    440.0,
    466.1637615180899,
    493.8833012561241,
    523.2511306011974,
    554.3652619537443,
    587.3295358348153,
    622.253967444162,
    659.2551138257401,
    698.456462866008,
    739.988845423269,
    783.990871963499,
    830.6093951598907,
    880.0000000000003,
    932.3275230361803,
    987.7666025122488,
    1046.5022612023952,
    1108.7305239074892,
    1174.659071669631,
    1244.5079348883246,
    1318.5102276514808,
    1396.912925732017,
    1479.977690846539,
    1567.9817439269987,
    1661.218790319782,
    1760.000000000002,
    1864.6550460723618,
    1975.5332050244986,
    2093.0045224047913,
    2217.4610478149793,
    2349.3181433392633,
    2489.0158697766506,
    2637.020455302963,
    2793.8258514640347,
    2959.9553816930793,
    3135.963487853999,
    3322.437580639566,
    3520.0000000000055,
    3729.310092144725,
    3951.0664100489994,
    4186.009044809585,
    4434.922095629961,
    4698.636286678529,
    4978.031739553304,
    5274.040910605929,
    5587.651702928073,
    5919.910763386162,
    6271.926975708001,
    6644.875161279136,
    7040.000000000014,
    7458.620184289454,
    7902.132820098003,
    8372.018089619174,
    8869.844191259926,
    9397.272573357064,
    9956.063479106611,
    10548.081821211863,
    11175.303405856152,
    11839.82152677233,
    12543.853951416007,
];


use raylib::prelude::*;
use raylib::core::logging::set_trace_log;


fn draw_keyboard(d: &mut impl RaylibDraw, bounds: Rectangle, key_map: [Option<Color>;128]) {
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
        let draw_color = if let Some(color) = key_map[key_id] { color } else { Color::WHITE };

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
            let draw_color = if let Some(color) = key_map[key_id] { color } else { Color::BLACK };
            d.draw_rectangle_rec(black_key_bounds, draw_color);
        }
    }
}

//fn get_ticks_per_frame(fps: u32, tempo: u32, ticks_per_quarter: u32) -> u32 { // TEMPO IN USECS
//    (ticks_per_quarter as f32 / (tempo as f32 * 1.0e-6) / (fps as f32)) as u32
//}

fn get_tick_time(ticks: u32, tempo: u32, ticks_per_quarter: u32) -> u32 {
    (tempo as u64 * ticks as u64 / ticks_per_quarter as u64) as u32
}


use hound;
use std::f64::consts::PI;
use std::i16;

const STANDARD_TEMPO: u32 = 400_000;


fn note_sine(t: f64, note: usize) -> f64 {
    (t * NOTE_FREQUENCIES[note] * 2.0 * PI).sin()
}

fn note_square(t: f64, note: usize) -> f64 {
    let period_length = 1.0;
    let period_progress = t*NOTE_FREQUENCIES[note] % period_length;
    //println!("t={},period_length={},period_progress={}",t,period_length,period_progress);
    const DEFAULT_VOLUME: f64 = 0.25;
    if period_progress >= period_length / 2.0 {
        DEFAULT_VOLUME
    } else {
        -DEFAULT_VOLUME
    }
}

fn note_drum(t: f64, _: usize) -> f64 {
    let noise = get_random_value::<i32>(-i16::MAX as i32, i16::MAX as i32) as f64 / i16::MAX as f64;
    if t < 0.15 && t >= 0.0 {
        noise * (1.0-t/0.15)
    } else {
        0.0
    }
}

fn note_saw_tooth(t: f64, note: usize) -> f64 {
    let period_length = 1.0;
    let period_progress = t*NOTE_FREQUENCIES[note] % period_length;
    //println!("t={},period_length={},period_progress={}",t,period_length,period_progress);
    (period_progress / period_length * 2.0 - 1.0) * 0.25
}

#[derive(Clone,Copy,PartialEq,Debug)]
struct PressedKeyInfo {
    elapsed_time: f64,
    channel: u8,
    key: u8,
    velocity: u8,
}



#[derive(Debug,Clone)]
enum NormalizedEvent {
    KeyOn { key: u8, program: u8, channel: u8 },
    KeyOff { key: u8, program: u8, channel: u8 }
}

// Store events with time in seconds
#[derive(Debug, Clone)]
struct NormalizedTrack {
    events: Vec<(f64, NormalizedEvent)>,
}

impl NormalizedTrack {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}

const DEFAULT_TRACK: usize = 0;

use std::thread;
use std::sync::mpsc;

#[derive(Clone, Debug)]
struct ProgressInfo {
    track: usize,
    track_progress: f64,
    result: Option<Vec<NormalizedTrack>>,
}

fn generate_audio(file: Arc<MidiFile>, wav_file_path: &str, sender: mpsc::Sender<ProgressInfo>) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int
    };
    let mut writer = hound::WavWriter::create(wav_file_path, spec).unwrap();
    //for t in (0 .. 44100).map(|x| x as f32 / 44100.0) {
    //    let sample = (t * 440.0 * 2.0 * PI).sin();
    //    let amplitude = i16::MAX as f32;
    //    writer.write_sample((sample * amplitude) as i16).unwrap();
    //}
    

    let simult_tracks: usize = file.get_simult_track_count();

    assert!(file.header.format != Format::SEQUENCE_TRACK);

    let mut sample_buffer = Vec::<i16>::new();
    let mut tempo_changes = Vec::new();

    let mut tempo = STANDARD_TEMPO;
    let mut usec_per_tick = if let Division::TicksPerQuarter(ticks) = file.header.division {
        tempo / ticks
    } else {
        todo!("Other divisions")
    };

    let mut noramalized_tracks = vec![NormalizedTrack::new(); simult_tracks];
    let mut progress_info = ProgressInfo { track: 0, track_progress: 0.0, result: None };
    for track in 0..simult_tracks {
        progress_info.track = track;
        //let track = DEFAULT_TRACK;
        let mut channels = [(0,127);256]; // (Program, Volume)
        let mut pressed_keys = Vec::<PressedKeyInfo>::new();
        let mut sample_pointer = 0;

        for (sample_index, usecs) in tempo_changes.iter() {
            if sample_pointer == *sample_index {
                usec_per_tick = *usecs;
            }
        }

        for (event_index, (dt,event)) in file.tracks[track].events.iter().enumerate() {
            progress_info.track_progress = event_index as f64 / file.tracks[track].events.len() as f64;
            //let elapsing_samples = usec_per_tick as u64 * *dt as u64 * spec.sample_rate as u64 / 1_000_000;
            let sec_per_sample = 1.0 / spec.sample_rate as f64;
            let mut dt_float = *dt as f64;
            let mut tick_per_sample = 1_000_000.0 as f64 / usec_per_tick as f64 / spec.sample_rate as f64;

            //for _ in 0..elapsing_samples {
            while dt_float > tick_per_sample {
                dt_float -= tick_per_sample;
                let mut s = 0.0;
                for key_info in pressed_keys.iter_mut() {
                        let program = channels[key_info.channel as usize].0;
                        let note_function = if program <= 7 { // piano
                            note_sine
                        } else if 8 <= program && program <= 15 { // Chromatic Percussion
                            note_sine
                        } else if 16 <= program && program <= 23 { // Organ
                            note_sine
                        } else if 24 <= program && program <= 31 { // Guitar
                            note_sine
                        } else if 32 <= program && program <= 39 { // Bass
                            note_saw_tooth
                        } else if 40 <= program && program <= 47 { // Strings
                            note_square
                        } else if 48 <= program && program <= 55 { // Ensemble
                            note_square
                        } else if 56 <= program && program <= 63 { // Brass
                            note_saw_tooth
                        } else if 64 <= program && program <= 71 { // Reed
                            note_saw_tooth
                        } else if 72 <= program && program <= 79 { // Pipe
                            note_square
                        } else if 80 <= program && program <= 87 { // Synth Lead
                            note_square
                        } else if 88 <= program && program <= 95 { // Synth Pad
                            note_sine
                        } else if 96 <= program && program <= 103 { // Synth Effects
                            note_drum
                        } else if 104 <= program && program <= 111 { // Ethnic
                            note_sine
                        } else if 112 <= program && program <= 119 { // Percussive
                            note_drum
                        } else if 120 <= program && program <= 127 { // Sound Effects
                            note_drum
                        } else {
                            unreachable!()
                        };
                        //    //Program::AcousticGrandPiano => note_sine(*t,i),
                        //    Program::StringEnsemble2
                        //        | Program::StringEnsemble1
                        //        | Program::SynthStrings1
                        //        | Program::SynthStrings2
                        //        | Program::Lead1
                        //        | Program::BrassSection
                        //        | Program::SynthBrass1
                        //        | Program::SynthBrass2
                        //        => note_square,
                        //    Program::SynthDrum
                        //        | Program::Pad1
                        //        | Program::Pad3
                        //        | Program::OrchestraHit
                        //        => note_drum,
                        //    Program::SynthBass2
                        //        | Program::FretlessBass
                        //        | Program::AltoSax
                        //        | Program::Lead2
                        //        => note_saw_tooth,
                        //    _ => note_sine
                        //};
                        s += note_function(key_info.elapsed_time, key_info.key as usize) * channels[key_info.channel as usize].1 as f64 / 127.0 * key_info.velocity as f64 / 127.0;
                        key_info.elapsed_time += sec_per_sample;
                }
                s /= 10.0; //pressed_keys.len() as f64;
                //writer.write_sample((i16::MAX as f64 * s) as i16).unwrap();
                if s.abs() >= 1.0 {
                    s = s / s.abs();
                }
                let sample_value = (i16::MAX as f64 * s) as i16;
                if sample_pointer >= sample_buffer.len() {
                    sample_buffer.push(sample_value);
                } else {
                    if sample_buffer[sample_pointer] == 0 {
                        sample_buffer[sample_pointer] = sample_value;
                    } else if (sample_buffer[sample_pointer] as i64 + sample_value as i64).abs() >= i16::MAX as i64 {
                        sample_buffer[sample_pointer] = sample_buffer[sample_pointer] / sample_buffer[sample_pointer].abs() * i16::MAX;
                    } else {
                        sample_buffer[sample_pointer] += sample_value;
                    }
                }
                for (sample_index, usecs) in tempo_changes.iter() {
                    if sample_pointer == *sample_index {
                        usec_per_tick = *usecs;
                        tick_per_sample = 1_000_000.0 as f64 / usec_per_tick as f64 / spec.sample_rate as f64;
                    }
                }
                sample_pointer += 1;
            }
            
            

            match event {
                Event::Meta(MetaEvent::SetTempo { tempo: t }) => {
                    tempo = *t;
                    usec_per_tick = if let Division::TicksPerQuarter(ticks) = file.header.division {
                        tempo / ticks
                    } else {
                        todo!("Other divisions")
                    };
                    //assert!(track==0);
                    tempo_changes.push((sample_pointer, usec_per_tick));
                    //println!("TEMPO CHANGE");
                },
                Event::Midi(c,MidiEvent::NoteOff { key, .. }) | Event::Midi(c, MidiEvent::NoteOn { key, velocity: 0 }) => {
                    for i in 0..pressed_keys.len() {
                        if pressed_keys[i].channel == *c && pressed_keys[i].key == *key {
                            pressed_keys.remove(i);
                            break;
                        }
                    }
                    noramalized_tracks[track].events.push((sample_pointer as f64 * sec_per_sample, NormalizedEvent::KeyOff { key: *key, program: channels[*c as usize].0, channel: *c }));
                },
                Event::Midi(c,MidiEvent::NoteOn { key, velocity }) => {
                    //if *c == channel {
                        pressed_keys.push(PressedKeyInfo { elapsed_time: 0.0, channel: *c, key: *key, velocity: *velocity });
                      // println!("HHHHHHSDHFSDFSD?????");
                    //}
                    noramalized_tracks[track].events.push((sample_pointer as f64 * sec_per_sample, NormalizedEvent::KeyOn { key: *key, program: channels[*c as usize].0, channel: *c }));
                }
                
                Event::Midi(c, MidiEvent::ControlChange(ControllerMessage::ChannelVolumeMSB(new_volume))) => {
                    channels[*c as usize].1 = *new_volume;
                },
                Event::Midi(c, MidiEvent::ProgramChange(prog)) => {
                    channels[*c as usize].0 = *prog;
                }
                _ => {}
            }
            if event_index % 100 == 0 { sender.send(progress_info.clone()).unwrap(); }
        }
    }
    //println!("END_OF_GENERATION");
    for s in sample_buffer {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    progress_info.result = Some(noramalized_tracks);
    sender.send(progress_info).unwrap();
}


use raylib::core::audio::Music;
use raylib::core::audio::RaylibAudio;

//const LISTEN_CHANNEL: u8 = 8;

const COLORS: [Color; 8] = [
    Color::GREEN,
    Color::RED,
    Color::BLUE,
    Color::MAGENTA,
    Color::GOLD,
    Color::PINK,
    Color::YELLOW,
    Color::SKYBLUE
];

//#[derive(Debug,Clone,Copy)]
//struct TrackPlayer {
//    event_pointer: usize,
//    dt_counter: u32,
//}
//
//impl TrackPlayer {
//    fn new() -> Self {
//        Self { event_pointer: 0, dt_counter: 0 }
//    }
//}

#[derive(Debug,Clone,Copy)]
struct TrackPlayer {
    event_pointer: usize,
    elapsed_time: f64,
}

impl TrackPlayer {
    fn new() -> Self {
        Self { event_pointer: 0, elapsed_time: 0.0 }
    }
}

use std::env;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum State {
    RENDERING,
    VISUALIZING
}

fn main() -> std::io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!("usage: {} [input] [output]", args[0]);
        return Ok(());
    }

    let file =  Arc::new(MidiFile::read_midi(&args[1])?);
    

    let wav_file_path = args[2].clone();

    let mut state = State::RENDERING;
    let (sender,receiver) = mpsc::channel();
    {
        let file_pointer = Arc::clone(&file);
        let wav_file_path = wav_file_path.clone();
        thread::spawn(move || generate_audio(file_pointer, &wav_file_path, sender));
    }
    assert!(file.header.format != Format::SEQUENCE_TRACK);

    const WINDOW_WIDTH: i32 = 1280;
    const WINDOW_HEIGHT: i32 = 720;
    const FPS: u32 = 60;


    set_trace_log(TraceLogLevel::LOG_NONE);
    let (mut rl, thread) = raylib::init().width(WINDOW_WIDTH).height(WINDOW_HEIGHT).title("Mididi").build();

    let mut rl_audio = RaylibAudio::init_audio_device();
    let mut music = None;
    //let mut music = Music::load_music_stream(&thread, &wav_file_path).unwrap();
    
    rl.set_exit_key(None);
    rl.set_target_fps(FPS);

    let mut normed_tracks: Option<Vec<NormalizedTrack>> = None;
    
    
    //let mut tempo = STANDARD_TEMPO;

    //let mut ticks_per_frame = if let Division::TicksPerQuarter(ticks) = file.header.division {
    //    get_ticks_per_frame(FPS, tempo, ticks)
    //} else {
    //    todo!("Other divisions")
    //};
    //let ticks_per_quarter = if let Division::TicksPerQuarter(ticks) = file.header.division {
    //    ticks
    //} else {
    //    todo!("Other divisions")
    //};
    let simult_tracks = file.get_simult_track_count(); 
    let mut track_players = vec![TrackPlayer::new(); simult_tracks as usize];
    
    let mut key_map  = [None; 128];
    let mut progress_info = ProgressInfo { track: 0, track_progress: 0.0, result: None };

    
    
    while !rl.window_should_close() {
        //queue_timer += 30;
        //if queue_timer > queue_pop_time && event_queue.len() > 0 {
        //    event_queue.pop_back();
        //    queue_timer = 0;
        //}
        match state {
        //rl_audio.update_music_stream(&mut music);
            State::VISUALIZING => {
                
                if rl_audio.is_music_playing(music.as_ref().unwrap()) {
                    let dt = rl.get_frame_time() as f64;
                    'outer: for (i, player) in track_players.iter_mut().enumerate() {
                        if player.event_pointer >= normed_tracks.as_ref().unwrap()[i].events.len() {
                            continue 'outer;
                        }
                        
                        while player.elapsed_time > normed_tracks.as_ref().unwrap()[i].events[player.event_pointer].0 {
                            match normed_tracks.as_ref().unwrap()[i].events[player.event_pointer].1 {
                                NormalizedEvent::KeyOff { key, .. } => {
                                    key_map[key as usize] = None;
                                },
                                NormalizedEvent::KeyOn { key, channel, .. } => {
                                    key_map[key as usize] = Some(COLORS[channel as usize % COLORS.len()]);
                                },
                            }
                            player.event_pointer += 1;
                            if player.event_pointer >= normed_tracks.as_ref().unwrap()[i].events.len() {
                                continue 'outer;
                            }
                        
                        }
                        player.elapsed_time += dt;
                    }
                }
                if !rl_audio.is_music_playing(music.as_ref().unwrap()) {
                    rl_audio.play_music_stream(&mut music.as_mut().unwrap());
                }
                rl_audio.update_music_stream(&mut music.as_mut().unwrap());

                if rl.is_key_pressed(KeyboardKey::KEY_B) {
                    rl.take_screenshot(&thread, "screenshot.png");
                }
        
                let mut d = rl.begin_drawing(&thread);
                d.clear_background(Color::BLACK);
                //d.draw_text("Hello World", 23, 23, 23, Color::RAYWHITE);
                let key_board_bounds = Rectangle::new(0.0, WINDOW_HEIGHT as f32 * 7.0/8.0, WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32 / 8.0);
                draw_keyboard(&mut d, key_board_bounds, key_map);
            },
            State::RENDERING => {
                
                match receiver.recv_timeout(std::time::Duration::from_secs_f64(1.0 / FPS as f64)) {
                    Ok(ProgressInfo { result: Some(ntracks), .. }) => {
                        normed_tracks = Some(ntracks);
                        state = State::VISUALIZING;
                        music = Some(Music::load_music_stream(&thread, &wav_file_path).unwrap());
                    },
                    Ok(info) => {
                        progress_info = info;
                    },
                    _ => {}
                }

                const PROGRESS_RECT_WIDTH: f32 = WINDOW_WIDTH as f32 / 3.0 * 2.0;
                const PROGRESS_RECT_HEIGHT: f32 = 23.0;

                let mut d = rl.begin_drawing(&thread);
                d.clear_background(Color::BLACK);

                d.draw_text(&format!("Track: {}/{}, Progress: {}", progress_info.track, file.header.ntrks, progress_info.track_progress), 23, 23, 23, Color::WHITE);
                let mut progress_rect = Rectangle::new(WINDOW_WIDTH as f32 / 2.0 - PROGRESS_RECT_WIDTH / 2.0, WINDOW_HEIGHT as f32 / 2.0 - PROGRESS_RECT_HEIGHT / 2.0, PROGRESS_RECT_WIDTH, PROGRESS_RECT_HEIGHT);
                d.draw_rectangle_rec(progress_rect, Color::GRAY);
                progress_rect.width *= progress_info.track_progress as f32;
                d.draw_rectangle_rec(progress_rect, Color::GREEN);
            }
        }
        //let dt = rl.get_frame_time();
        //for (i,player) in track_players.iter_mut().enumerate() {
        //    //if i != DEFAULT_TRACK { continue; }
        //    player.dt_counter += (dt * 1.0e6) as u32;
        //    while player.event_pointer < file.tracks[i].events.len() {
        //        let (dt, event) = &file.tracks[i].events[player.event_pointer];
        //        let dt_time = get_tick_time(*dt, tempo, ticks_per_quarter);
        //        if player.dt_counter < dt_time { break; }
        //        match *event {
        //            Event::Meta(MetaEvent::SetTempo { tempo: t }) => {
        //                tempo = t;
        //                println!("TEMPO CHANGE");
        //            },
        //            //Event::Midi(channel,MidiEvent::ProgramChange(program)) => {
        //            //    //if channel == LISTEN_CHANNEL {
        //            //    //    println!("Program for channel {}: {:?}", channel, program);
        //            //    //}
        //            //}
        //            Event::Midi(channel,MidiEvent::NoteOn { key, ..}) => {
        //                
        //                key_map[key as usize] = Some(COLORS[channel as usize % COLORS.len() ]);
        //            }
        //            Event::Midi(_, MidiEvent::NoteOff { key, ..}) => {
        //                key_map[key as usize] = None;
        //            }
        //            _ => {}
        //        }
        //        
        //        
        //        //event_queue.push_front(event.clone());
        //        player.dt_counter -= dt_time;
        //        player.event_pointer += 1;
        //    }
        //}
        //dt_counter = 0;
        
        
        
        //for (i,event) in event_queue.iter().enumerate() {
        //    d.draw_text(&format!("{:?}", event), 23, 23 + i as i32 * 40, 23, Color::WHITE);
        //}
    }

    Ok(())
    
}
