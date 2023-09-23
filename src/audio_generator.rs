
use crate::midi_parser::{MidiFile, MidiError, Format, Division, Event, MidiEvent, MetaEvent, ControllerMessage, MidiErrorType};
use raylib::get_random_value;
use std::sync::{Arc, Mutex};

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
pub enum NormalizedEvent {
    KeyOn { key: u8, program: u8, channel: u8 },
    KeyOff { key: u8, program: u8, channel: u8 }
}

#[derive(Debug, Clone)]
pub struct NormalizedTrack {
    pub events: Vec<(f64, NormalizedEvent)>,
}

impl NormalizedTrack {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
}

#[derive(Clone,Debug)]
pub struct ProgressInfo {
    pub track: usize,
    pub track_progress: f64,
    pub result: Option<Vec<NormalizedTrack>>,
    pub error: Option<MidiError>,
}

impl ProgressInfo {
    pub fn new() -> Self {
        Self { track: 0, track_progress: 0.0, result: None, error: None }
    }
}

pub fn generate_audio(file: Arc<MidiFile>, wav_file_path: &str, progress_info: Arc<Mutex<ProgressInfo>>) {
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