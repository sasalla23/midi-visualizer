use std::sync::Arc;

use raylib::prelude::*;
use raylib::core::logging::set_trace_log;

mod midi_parser;
mod audio_generator;


const WHITE_KEY_COUNT: usize = 75;
const KEY_COUNT: usize = 128;

fn white_key_rect(index: usize, bounds: Rectangle) -> Rectangle {
    let key_width = bounds.width / WHITE_KEY_COUNT as f32;
    Rectangle::new(bounds.x + key_width * index as f32, bounds.y, key_width, bounds.height)
}

fn black_key_rect(left_white_index: usize, bounds: Rectangle, black_key_width: f32, black_key_height: f32) -> Rectangle {
    let white_rect = white_key_rect(left_white_index, bounds);
    Rectangle::new(white_rect.x + white_rect.width - black_key_width, white_rect.y, black_key_width, black_key_height)
}

fn is_black_key(key: u8) -> bool {
    match key%12 {
        1 | 3 | 6 | 8 | 10 => true,
        _ => false
    }
}

fn key_rect(key: u8, bounds: Rectangle) -> Rectangle {
    let white_key_width = bounds.width / WHITE_KEY_COUNT as f32;
    let black_key_width = white_key_width / 2.0;
    let black_key_height = bounds.height * 0.75;

    let get_white_key_index = |white_key|  {
        let note = white_key % 12;
        let mut key_id = white_key / 12 * 7;
        key_id += note;
        if note > 1 { key_id -= 1 }
        if note > 3 { key_id -= 1 }
        if note > 6 { key_id -= 1 }
        if note > 8 { key_id -= 1 }
        if note > 10 { key_id -= 1 }
        key_id as usize
    };

    if is_black_key(key) {
        let white_key = key - 1;
        black_key_rect(get_white_key_index(white_key), bounds, black_key_width, black_key_height)
    } else {
        white_key_rect(get_white_key_index(key), bounds)
    }
}

fn draw_keyboard(d: &mut impl RaylibDraw, bounds: Rectangle, key_map: [Option<Color>;128]) {
    // Draw white keys
    for i in 0..KEY_COUNT {
        if !is_black_key(i as u8) {
            let draw_color = if let Some(color) = key_map[i] { color } else { Color::WHITE };
            let rect = key_rect(i as u8, bounds);
            d.draw_rectangle_rec(rect, draw_color);
            d.draw_rectangle_lines_ex(rect, 1, Color::GRAY);
        }
    }

    // Draw black keys
    for i in 0..KEY_COUNT {
        if is_black_key(i as u8) {
            let draw_color = if let Some(color) = key_map[i] { color } else { Color::BLACK };
            d.draw_rectangle_rec(key_rect(i as u8, bounds), draw_color);
        }
    }
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

fn get_color(channel: u8) -> Color {
    COLORS[channel as usize % COLORS.len()]
}

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

struct NoteVisual {
    channel: u8,
    key: u8,
    track: usize,
    start_time: f64,
    stop_time: Option<f64>,
    rect: Option<Rectangle>,
}

const TIME_OFFSET: f64 = 3.0;
fn compute_keyboard_bounds(rl: &RaylibHandle) -> Rectangle {
    Rectangle::new(0.0, rl.get_screen_height() as f32 * 7.0/8.0, rl.get_screen_width() as f32, rl.get_screen_height() as f32 / 8.0)
}

fn compute_time_scale(rl: &RaylibHandle) -> f64 {
    (rl.get_screen_height() as f64 * 7.0 / 8.0) / TIME_OFFSET
}

impl NoteVisual {
    fn new(channel: u8, key: u8, track: usize, start_time: f64) -> Self { 
        Self { channel, key, track, stop_time: None, start_time, rect: None }
    }

    fn get_rect(&self, rl: &RaylibHandle, curr_time: f64, keyboard_bounds: Rectangle) -> Rectangle {
        let keyboard_key_rect = key_rect(self.key, keyboard_bounds);
        let time_scale = compute_time_scale(rl);
        let start_y = match self.stop_time {
            Some(t) => (curr_time - t) * time_scale,
            None => 0.0
        };
        let end_y = (curr_time - self.start_time) * time_scale;
        Rectangle::new(keyboard_key_rect.x, start_y as f32, keyboard_key_rect.width, (end_y - start_y) as f32)
    }

    fn render(&mut self, curr_time: f64, d: &mut RaylibDrawHandle, keyboard_bounds: Rectangle) {
        let color = get_color(self.channel);
        let exact_rect = self.get_rect(d, curr_time, keyboard_bounds);
        const LERP_VALUE: f64 = 0.25;
        self.rect = match self.rect {
            None => Some(exact_rect),
            Some(rect) => Some(
                Rectangle {
                    x: exact_rect.x,
                    //y: if self.stop_time.is_none() { 0.0 } else { lerp(rect.y, exact_rect.y, LERP_VALUE as f32) },
                    y: lerp(rect.y, exact_rect.y, LERP_VALUE as f32),
                    width: exact_rect.width,
                    //height: if self.stop_time.is_none() { lerp(rect.height, exact_rect.height, LERP_VALUE as f32) } else { exact_rect.height },
                    height: lerp(rect.height, exact_rect.height, LERP_VALUE as f32),
                }
                
            )
        };
        //d.draw_rectangle_rec(self.rect.unwrap(), color);
        d.draw_rectangle_rec(self.rect.unwrap(), color);
    }
}

use audio_generator::{NormalizedTrack, NormalizedEvent};

fn update_track_players(track_players: &mut Vec<TrackPlayer>, normed_tracks: &Vec<NormalizedTrack>, elapsed_time: f64, note_visuals: &mut Vec<NoteVisual>) {
    'outer: for (i, player) in track_players.iter_mut().enumerate() {
                    
        if player.event_pointer >= normed_tracks[i].events.len() {
            continue 'outer;
        }

        while elapsed_time > normed_tracks[i].events[player.event_pointer].0 {
            match normed_tracks[i].events[player.event_pointer].1 {
                NormalizedEvent::KeyOff { key, channel, .. } => {
                    //key_map[key as usize] = None;
                    for nv in note_visuals.iter_mut() {
                        if nv.track == i && nv.channel == channel && nv.key == key && nv.stop_time.is_none() {
                            nv.stop_time = Some(elapsed_time);
                            break;
                        }
                    }
                },
                NormalizedEvent::KeyOn { key, channel, .. } => {
                    //let color = COLORS[channel as usize % COLORS.len()];
                    //key_map[key as usize] = Some(color);
                    note_visuals.push(NoteVisual::new(channel, key, i, elapsed_time));
                },
            }
            player.event_pointer += 1;
            if player.event_pointer >= normed_tracks[i].events.len() {
                continue 'outer;
            }
        
        }
        
    }
}

use std::thread;
use std::sync::Mutex;
use audio_generator::{ProgressInfo, generate_audio};
use midi_parser::{Format, MidiFile};

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
    let (mut rl, thread) = raylib::init().width(WINDOW_WIDTH).height(WINDOW_HEIGHT).title("Mididi").resizable().build();
    

    let mut rl_audio = RaylibAudio::init_audio_device();
    let mut music = None;
    
    rl.set_exit_key(None);
    rl.set_target_fps(FPS);
    
    let simult_tracks = file.get_simult_track_count(); 
    let mut track_players = vec![TrackPlayer::new(); simult_tracks as usize];
    
    let mut key_map  = [None; 128];
    let mut note_visuals: Vec<NoteVisual> = Vec::new();
    let mut key_board_bounds = compute_keyboard_bounds(&rl);
    //println!("TIME OFFSET: {}", time_offset);
    while !rl.window_should_close() {
        let fps = rl.get_fps();
        if rl.is_window_resized() {
            key_board_bounds = compute_keyboard_bounds(&rl);
        }
        match state {
            State::VISUALIZING => {
                
                let pi = progress_info.lock().unwrap();
                let music = music.as_mut().unwrap();
                let normed_tracks = pi.result.as_ref().unwrap();
                
                

                rl_audio.update_music_stream(music);
                let elapsed_time = rl_audio.get_music_time_played(music) as f64 + TIME_OFFSET;
                
                update_track_players(&mut track_players, &normed_tracks, elapsed_time, &mut note_visuals);
                
                let initial_note_visual_count = note_visuals.len();
                for i in 0..initial_note_visual_count {
                    let index = initial_note_visual_count-(i+1);
                    let nv = &note_visuals[index];
                    let nv_rect = nv.get_rect(&rl, elapsed_time, key_board_bounds);
                    if nv_rect.check_collision_recs(&key_rect(nv.key, key_board_bounds)) {
                        key_map[nv.key as usize] = Some(get_color(nv.channel));
                    }
                    if nv_rect.y > key_board_bounds.y {
                        key_map[nv.key as usize] = None;
                        note_visuals.remove(index);
                    }
                }

                if rl.is_key_pressed(KeyboardKey::KEY_B) {
                    rl.take_screenshot(&thread, "screenshot.png");
                }
        
                let mut d = rl.begin_drawing(&thread);
                d.clear_background(Color::BLACK);
                
                for key in note_visuals.iter_mut() {
                    key.render(elapsed_time, &mut d, key_board_bounds)
                }
                draw_keyboard(&mut d, key_board_bounds, key_map);
                d.draw_text(&format!("{}", fps), 23,23, 23, Color::WHITE);

            },
            State::RENDERING => {
                {
                    let pi = progress_info.lock().unwrap();

                    if let Some(err) = &pi.error {
                        eprintln!("{}", err);
                        return;
                    }

                    if let Some(normed_tracks) = &pi.result {
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
                        let frames = (TIME_OFFSET * FPS as f64) as usize;
                        for frame in 0..frames {
                            update_track_players(&mut track_players, normed_tracks, frame as f64 / FPS as f64, &mut note_visuals);
                        }
                    }
                }

                const PROGRESS_RECT_WIDTH: f32 = WINDOW_WIDTH as f32 / 3.0 * 2.0;
                const PROGRESS_RECT_HEIGHT: f32 = 23.0;

                let mut d = rl.begin_drawing(&thread);
                d.clear_background(Color::BLACK);

                {
                    let pi = progress_info.lock().unwrap();
                    d.draw_text(&format!("Track: {}/{}, Progress: {}", pi.track, file.header.ntrks, pi.track_progress), 23, 23, 23, Color::WHITE);
                    let mut progress_rect = Rectangle::new(d.get_screen_width() as f32 / 2.0 - PROGRESS_RECT_WIDTH / 2.0, d.get_screen_height() as f32 / 2.0 - PROGRESS_RECT_HEIGHT / 2.0, PROGRESS_RECT_WIDTH, PROGRESS_RECT_HEIGHT);
                    d.draw_rectangle_rec(progress_rect, Color::GRAY);
                    progress_rect.width *= pi.track_progress as f32;
                    d.draw_rectangle_rec(progress_rect, Color::GREEN);
                }
            }
        }
        
    }
}
