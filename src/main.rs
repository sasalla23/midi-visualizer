use std::fmt::Display;
use std::fs::File;
use std::io::prelude::*;
use std::str;
use std::io;
use std::sync::Arc;

#[derive(Debug, Clone,Copy,PartialEq, Eq)]
enum Format {
    SingleTrack,
    SimulTrack,
    SequenceTrack
}

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
    Unknown
}

#[derive(Debug,Clone)]
enum Event {
    Midi(u8,MidiEvent),
    Sysex,
    Meta(MetaEvent)
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum MidiErrorType {
    IO,
    InvalidMidi
}

#[derive(Debug, Clone)]
struct MidiError {
    message: String,
    error_type: MidiErrorType
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
struct MidiFile {
    header: HeaderChunk,
    tracks: Vec<TrackChunk>,
}

impl MidiFile {
    fn read_midi(file_path: &str) -> Result<Self,MidiError> {
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

    fn get_simult_track_count(&self) -> usize {
        match self.header.format {
            Format::SingleTrack => 1,
            Format::SimulTrack => self.header.ntrks as usize,
            Format::SequenceTrack => 1,
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

#[derive(Debug, Clone)]
struct NormalizedTrack {
    events: Vec<(f64, NormalizedEvent)>,
}

impl NormalizedTrack {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}

use std::thread;
use std::sync::Mutex;

#[derive(Clone,Debug)]
struct ProgressInfo {
    track: usize,
    track_progress: f64,
    result: Option<Vec<NormalizedTrack>>,
    error: Option<MidiError>,
}

impl ProgressInfo {
    fn new() -> Self {
        Self { track: 0, track_progress: 0.0, result: None, error: None }
    }
}

fn generate_audio(file: Arc<MidiFile>, wav_file_path: &str, progress_info: Arc<Mutex<ProgressInfo>>) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int
    };
    let mut writer = hound::WavWriter::create(wav_file_path, spec).unwrap();

    let simult_tracks: usize = file.get_simult_track_count();

    assert!(file.header.format != Format::SequenceTrack);

    let mut sample_buffer = Vec::<i16>::new();
    let mut tempo_changes = Vec::new();

    let mut tempo = STANDARD_TEMPO;
    let mut usec_per_tick = if let Division::TicksPerQuarter(ticks) = file.header.division {
        tempo / ticks
    } else {
        todo!("Other divisions")
    };

    let mut normalized_tracks = vec![NormalizedTrack::new(); simult_tracks];

    for track in 0..simult_tracks {
        {
            let mut pi = progress_info.lock().unwrap();
            pi.track = track;
        }
        let mut channels = [(0,127);256];
        let mut pressed_keys = Vec::<PressedKeyInfo>::new();
        let mut sample_pointer = 0;

        for (sample_index, usecs) in tempo_changes.iter() {
            if sample_pointer == *sample_index {
                usec_per_tick = *usecs;
            }
        }

        for (event_index, (dt,event)) in file.tracks[track].events.iter().enumerate() {
            {
                let mut pi = progress_info.lock().unwrap();
                pi.track_progress = event_index as f64 / file.tracks[track].events.len() as f64;
            }
            
            let sec_per_sample = 1.0 / spec.sample_rate as f64;
            let mut dt_float = *dt as f64;
            let mut tick_per_sample = 1_000_000.0 as f64 / usec_per_tick as f64 / spec.sample_rate as f64;

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
                        
                        s += note_function(key_info.elapsed_time, key_info.key as usize) * channels[key_info.channel as usize].1 as f64 / 127.0 * key_info.velocity as f64 / 127.0;
                        key_info.elapsed_time += sec_per_sample;
                }
                s /= 10.0;
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
                    tempo_changes.push((sample_pointer, usec_per_tick));
                },
                Event::Midi(c,MidiEvent::NoteOff { key, .. }) | Event::Midi(c, MidiEvent::NoteOn { key, velocity: 0 }) => {
                    for i in 0..pressed_keys.len() {
                        if pressed_keys[i].channel == *c && pressed_keys[i].key == *key {
                            pressed_keys.remove(i);
                            break;
                        }
                    }
                    normalized_tracks[track].events.push((sample_pointer as f64 * sec_per_sample, NormalizedEvent::KeyOff { key: *key, program: channels[*c as usize].0, channel: *c }));
                },
                Event::Midi(c,MidiEvent::NoteOn { key, velocity }) => {
                    pressed_keys.push(PressedKeyInfo { elapsed_time: 0.0, channel: *c, key: *key, velocity: *velocity });
                    normalized_tracks[track].events.push((sample_pointer as f64 * sec_per_sample, NormalizedEvent::KeyOn { key: *key, program: channels[*c as usize].0, channel: *c }));
                }
                
                Event::Midi(c, MidiEvent::ControlChange(ControllerMessage::ChannelVolumeMSB(new_volume))) => {
                    channels[*c as usize].1 = *new_volume;
                },
                Event::Midi(c, MidiEvent::ProgramChange(prog)) => {
                    channels[*c as usize].0 = *prog;
                }
                _ => {}
            }
        }
    }

    for s in sample_buffer {
        if let Err(e) = writer.write_sample(s) {
            let mut pi = progress_info.lock().unwrap();
            pi.error = Some(MidiError { message: e.to_string(), error_type: MidiErrorType::IO });
            return;
        }
    }

    if let Err(e) = writer.finalize() {
        let mut pi = progress_info.lock().unwrap();
        pi.error = Some(MidiError { message: e.to_string(), error_type: MidiErrorType::IO });
        return;
    }

    let mut pi = progress_info.lock().unwrap();
    pi.result = Some(normalized_tracks);
}

use raylib::core::audio::Music;
use raylib::core::audio::RaylibAudio;

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

#[derive(Debug,Clone,Copy)]
struct TrackPlayer {
    event_pointer: usize
}

impl TrackPlayer {
    fn new() -> Self {
        Self { event_pointer: 0 }
    }
}

use std::env;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
enum State {
    RENDERING,
    VISUALIZING
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 3 {
        eprintln!("usage: {} [input] [output]", args[0]);
        return;
    }

    let file =  Arc::new({
        match MidiFile::read_midi(&args[1]) {
            Ok(file) => file,
            Err(err) => {
                eprintln!("{}", err);
                return;
            }
        }
    });
    let wav_file_path = args[2].clone();

    let mut state = State::RENDERING;
    let progress_info = Arc::new(Mutex::new(ProgressInfo::new()));
    {
        let file_pointer = Arc::clone(&file);
        let wav_file_path = wav_file_path.clone();
        let progress_info_pointer = Arc::clone(&progress_info);
        thread::spawn(move || generate_audio(file_pointer, &wav_file_path, progress_info_pointer));
    }
    assert!(file.header.format != Format::SequenceTrack);

    const WINDOW_WIDTH: i32 = 1280;
    const WINDOW_HEIGHT: i32 = 720;
    const FPS: u32 = 60;


    set_trace_log(TraceLogLevel::LOG_NONE);
    let (mut rl, thread) = raylib::init().width(WINDOW_WIDTH).height(WINDOW_HEIGHT).title("Mididi").build();
    

    let mut rl_audio = RaylibAudio::init_audio_device();
    let mut music = None;
    
    rl.set_exit_key(None);
    rl.set_target_fps(FPS);
    
    let simult_tracks = file.get_simult_track_count(); 
    let mut track_players = vec![TrackPlayer::new(); simult_tracks as usize];
    
    let mut key_map  = [None; 128];
    
    while !rl.window_should_close() {
        match state {
            State::VISUALIZING => {
                let pi = progress_info.lock().unwrap();
                let music = music.as_mut().unwrap();
                let normed_tracks = pi.result.as_ref().unwrap();

                rl_audio.update_music_stream(music);
                let elapsed_time = rl_audio.get_music_time_played(music) as f64;
                'outer: for (i, player) in track_players.iter_mut().enumerate() {
                    
                    if player.event_pointer >= normed_tracks[i].events.len() {
                        continue 'outer;
                    }

                    while elapsed_time > normed_tracks[i].events[player.event_pointer].0 {
                        match normed_tracks[i].events[player.event_pointer].1 {
                            NormalizedEvent::KeyOff { key, .. } => {
                                key_map[key as usize] = None;
                            },
                            NormalizedEvent::KeyOn { key, channel, .. } => {
                                key_map[key as usize] = Some(COLORS[channel as usize % COLORS.len()]);
                            },
                        }
                        player.event_pointer += 1;
                        if player.event_pointer >= normed_tracks[i].events.len() {
                            continue 'outer;
                        }
                    
                    }
                    
                }
                

                if rl.is_key_pressed(KeyboardKey::KEY_B) {
                    rl.take_screenshot(&thread, "screenshot.png");
                }
        
                let mut d = rl.begin_drawing(&thread);
                d.clear_background(Color::BLACK);
                
                let key_board_bounds = Rectangle::new(0.0, WINDOW_HEIGHT as f32 * 7.0/8.0, WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32 / 8.0);
                draw_keyboard(&mut d, key_board_bounds, key_map);
            },
            State::RENDERING => {
                {
                    let pi = progress_info.lock().unwrap();

                    if let Some(err) = &pi.error {
                        eprintln!("{}", err);
                        return;
                    }

                    if let Some(_) = &pi.result {
                        state = State::VISUALIZING;
                        music = Some({
                            match Music::load_music_stream(&thread, &wav_file_path) {
                                Ok(m) => m,
                                Err(err) => {
                                    eprintln!("IOError: Failed to reload the generated audio {}: {}", &wav_file_path, err);
                                    return;
                                }
                            }
                        });
                        rl_audio.play_music_stream(&mut music.as_mut().unwrap());
                    }
                }

                const PROGRESS_RECT_WIDTH: f32 = WINDOW_WIDTH as f32 / 3.0 * 2.0;
                const PROGRESS_RECT_HEIGHT: f32 = 23.0;

                let mut d = rl.begin_drawing(&thread);
                d.clear_background(Color::BLACK);

                {
                    let pi = progress_info.lock().unwrap();
                    d.draw_text(&format!("Track: {}/{}, Progress: {}", pi.track, file.header.ntrks, pi.track_progress), 23, 23, 23, Color::WHITE);
                    let mut progress_rect = Rectangle::new(WINDOW_WIDTH as f32 / 2.0 - PROGRESS_RECT_WIDTH / 2.0, WINDOW_HEIGHT as f32 / 2.0 - PROGRESS_RECT_HEIGHT / 2.0, PROGRESS_RECT_WIDTH, PROGRESS_RECT_HEIGHT);
                    d.draw_rectangle_rec(progress_rect, Color::GRAY);
                    progress_rect.width *= pi.track_progress as f32;
                    d.draw_rectangle_rec(progress_rect, Color::GREEN);
                }
            }
        }
    }
}
