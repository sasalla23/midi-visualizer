use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::str;
use std::io;
use std::collections::VecDeque;

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

fn key_index(key: Key, octave: i8) -> usize {
    (octave + 1) as usize * 12 + key as usize
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

#[derive(Debug,Clone)]
enum MidiEvent {
    ControlChange(ControllerMessage),
    ProgramChange(Program),
    PitchWheelChange(u32),
    NoteOn { octave: i8, key: Key, velocity: u8 },
    NoteOff { octave: i8, key: Key, velocity: u8 }
}

#[derive(Debug)]
struct HeaderChunk {
    format: Format,
    ntrks: u32,
    division: Division
}

#[derive(Debug)]
struct TrackChunk { 
    events: Vec<(u32, Event)>
}

#[derive(Debug)]
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

            Chunk::Header(HeaderChunk { format, ntrks, division })
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

struct MidiFile {
    header: HeaderChunk,
    tracks: Vec<TrackChunk>,
}

impl MidiFile {
    fn read_midi(file_path: &str) -> io::Result<Self> {
        let mut file = File::open(file_path).unwrap();
        let header_chunk = read_chunk(&mut file);
        if let Chunk::Header(header) = header_chunk {
            println!("{:?}", header);
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

fn get_ticks_per_frame(fps: u32, tempo: u32, ticks_per_quarter: u32) -> u32 { // TEMPO IN USECS
    (ticks_per_quarter as f32 / (tempo as f32 * 1.0e-6) / (fps as f32)) as u32
}

fn get_tick_time(ticks: u32, tempo: u32, ticks_per_quarter: u32) -> u32 {
    tempo * ticks / ticks_per_quarter
}



use hound;
use std::f64::consts::PI;
use std::i16;

const STANDARD_TEMPO: u32 = 500_000;

fn generate_audio(file: &MidiFile, channel: u8, wav_file_path: &str) {
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
    let mut tempo = STANDARD_TEMPO;
    let mut usec_per_tick = if let Division::TicksPerQuarter(ticks) = file.header.division {
        tempo / ticks
    } else {
        todo!("Other divisions")
    };

    assert!(file.header.format == Format::SINGLE_TRACK);

    let mut key_map: [Option<f64>;127] = [None; 127];

    for (dt,event) in file.tracks[0].events.iter() {
        let elapsing_samples = usec_per_tick as u64 * *dt as u64 * spec.sample_rate as u64 / 1_000_000;
        let sec_per_sample = 1.0 / spec.sample_rate as f64;
        
        for sample in 0..elapsing_samples {
            let mut s = 0.0;
            let mut pressed_keys = 0;
            for (i,key_time) in key_map.iter_mut().enumerate() {
                if let Some(t) = key_time {
                    s += (*t * NOTE_FREQUENCIES[i] * 2.0 * PI).sin();
                    *t += sec_per_sample;
                }
                pressed_keys += 1;
            }
            s /= pressed_keys as f64;
            writer.write_sample((i16::MAX as f64 * s) as i16).unwrap();
        }

        match event {
            Event::Meta(MetaEvent::SetTempo { tempo: t }) => {
                tempo = *t;
                usec_per_tick = if let Division::TicksPerQuarter(ticks) = file.header.division {
                    tempo / ticks
                } else {
                    todo!("Other divisions")
                };
            },
            Event::Midi(c,MidiEvent::NoteOn { octave, key, ..}) => {
                let index = key_index(*key, *octave);
                if *c == channel {
                    key_map[index] = Some(0.0);
                  // println!("HHHHHHSDHFSDFSD?????");
                }
            }
            Event::Midi(c,MidiEvent::NoteOff { octave, key, ..}) => {
                let index = key_index(*key, *octave);
                if *c == channel {
                    key_map[index] = None;
                }
            }
            _ => {}
        }
    }
    writer.finalize().unwrap();
}


use raylib::core::audio::Music;
use raylib::core::audio::RaylibAudio;

const LISTEN_CHANNEL: u8 = 15;

fn main() -> std::io::Result<()> {
    let file =  MidiFile::read_midi("Never-Gonna-Give-You-Up-3.mid")?;
    let wav_file_path = "test.wav";
    generate_audio(&file, LISTEN_CHANNEL, wav_file_path);
    assert!(file.header.format == Format::SINGLE_TRACK);

    const WINDOW_WIDTH: i32 = 2560;
    const WINDOW_HEIGHT: i32 = 1440;
    const FPS: u32 = 60;


    set_trace_log(TraceLogLevel::LOG_NONE);
    let (mut rl, thread) = raylib::init().width(WINDOW_WIDTH).height(WINDOW_HEIGHT).title("Mididi").build();

    let mut rl_audio = RaylibAudio::init_audio_device();
    let mut music = Music::load_music_stream(&thread, wav_file_path).unwrap();
    
    rl.set_exit_key(None);
    rl.set_target_fps(FPS);
    
    
    let mut tempo = STANDARD_TEMPO;
    for (_,event) in file.tracks[0].events.iter() {
        if let Event::Meta(MetaEvent::SetTempo { tempo: t }) = event {
            tempo = *t;
            break;
        }
    }

    //let mut ticks_per_frame = if let Division::TicksPerQuarter(ticks) = file.header.division {
    //    get_ticks_per_frame(FPS, tempo, ticks)
    //} else {
    //    todo!("Other divisions")
    //};
    let ticks_per_quarter = if let Division::TicksPerQuarter(ticks) = file.header.division {
        ticks
    } else {
        todo!("Other divisions")
    };

    let mut event_pointer: usize = 0;
    let mut dt_counter: u32 = 0;

    
    let mut key_map  = [false; 128];
    //key_map[60] = true;
    //key_map[63] = true;
    //key_map[67] = true;

    //let mut event_queue = VecDeque::<Event>::new();
    //let mut queue_timer = 0;
    //let queue_pop_time = 1;
    rl_audio.play_music_stream(&mut music);
    rl_audio.set_music_volume(&mut music, 1.0);
    while !rl.window_should_close() {
        //queue_timer += 30;
        //if queue_timer > queue_pop_time && event_queue.len() > 0 {
        //    event_queue.pop_back();
        //    queue_timer = 0;
        //}
        rl_audio.update_music_stream(&mut music);
        
        dt_counter += (rl.get_frame_time() * 1.0e6) as u32;
        while event_pointer < file.tracks[0].events.len() {
            let (dt, event) = &file.tracks[0].events[event_pointer];
            let dt_time = get_tick_time(*dt, tempo, ticks_per_quarter);
            if dt_counter < dt_time { break; }
            match *event {
                Event::Meta(MetaEvent::SetTempo { tempo: t }) => {
                    tempo = t;
                },
                Event::Midi(channel,MidiEvent::ProgramChange(program)) => {
                    if channel == LISTEN_CHANNEL {
                        println!("Program for channel {}: {:?}", channel, program);
                    }
                }
                Event::Midi(channel,MidiEvent::NoteOn { octave, key, ..}) => {
                    let index = key_index(key, octave);
                    if channel == LISTEN_CHANNEL {
                        key_map[index] = true;
                      // println!("HHHHHHSDHFSDFSD?????");
                    }
                }
                Event::Midi(channel,MidiEvent::NoteOff { octave, key, ..}) => {
                    let index = key_index(key, octave);
                    if channel == LISTEN_CHANNEL {
                        key_map[index] = false;
                    }
                }
                _ => {}
            }
            
            
            //event_queue.push_front(event.clone());
            dt_counter -= dt_time;
            event_pointer += 1;
        }
        //dt_counter = 0;

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        //d.draw_text("Hello World", 23, 23, 23, Color::RAYWHITE);
        let key_board_bounds = Rectangle::new(0.0, WINDOW_HEIGHT as f32 * 7.0/8.0, WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32 / 8.0);
        draw_keyboard(&mut d, key_board_bounds, key_map);
        //for (i,event) in event_queue.iter().enumerate() {
        //    d.draw_text(&format!("{:?}", event), 23, 23 + i as i32 * 40, 23, Color::WHITE);
        //}
    }

    Ok(())
    
}
