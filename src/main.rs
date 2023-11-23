mod audio;
mod input;
mod libretro;
mod video;
use audio::AudioBuffer;
use gilrs::{GamepadId, Gilrs, Gamepad};
use libretro_sys::PixelFormat;
use minifb::{Key, Window, WindowOptions};
use once_cell::sync::Lazy;
use rodio::{OutputStream, Sink};
use video::set_up_pixel_format;
use std::sync::atomic::AtomicU8;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::video::render_frame;

static BUTTONS_PRESSED: Lazy<Mutex<(Vec<i16>, Vec<i16>)>> = Lazy::new(|| {
    Mutex::new((vec![0; 16], vec![0; 16]))
});
static BYTES_PER_PIXEL: AtomicU8 = AtomicU8::new(4); // Default value of 4

static PIXEL_FORMAT_CHANNEL: Lazy<(Sender<PixelFormat>, Arc<Mutex<Receiver<PixelFormat>>>)> =
    Lazy::new(|| {
        let (sender, receiver) = channel::<PixelFormat>();
        (sender, Arc::new(Mutex::new(receiver)))
    });

static VIDEO_DATA_CHANNEL: Lazy<(Sender<VideoData>, Arc<Mutex<Receiver<VideoData>>>)> =
    Lazy::new(|| {
        let (sender, receiver) = channel::<VideoData>();
        (sender, Arc::new(Mutex::new(receiver)))
    });

static AUDIO_DATA_CHANNEL: Lazy<(
    Sender<Arc<Mutex<AudioBuffer>>>,
    Arc<Mutex<Receiver<Arc<Mutex<AudioBuffer>>>>>,
)> = Lazy::new(|| {
    let (sender, receiver) = channel::<Arc<Mutex<AudioBuffer>>>();
    (sender, Arc::new(Mutex::new(receiver)))
});

struct VideoData {
    frame_buffer: Vec<u32>,
    width: u32,
    height: u32,
    pitch: u32,
}

const WIDTH: usize = 256;
const HEIGHT: usize = 140;

fn main() {
    let (rom_name, library_name) = libretro::parse_command_line_arguments();
    let mut current_state = libretro::EmulatorState {
        rom_name,
        library_name,
        frame_buffer: None,
        screen_pitch: 0,
        screen_width: 0,
        screen_height: 0,
        current_save_slot: 0,
        av_info: None,
        pixel_format: video::EmulatorPixelFormat(PixelFormat::ARGB8888),
        bytes_per_pixel: 0,
    };

    let mut window = Window::new("Rust Game", WIDTH, HEIGHT, WindowOptions::default())
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

    window.limit_update_rate(Some(std::time::Duration::from_micros(16600))); // ~60fps

    let (core, updated_state) = libretro::Core::new(current_state);
    let core_api = &core.api;
    current_state = updated_state;

    // Extract the sample_rate before spawning the thread
    let sample_rate = current_state
        .av_info
        .as_ref()
        .map_or(0.0, |av_info| av_info.timing.sample_rate);

    let _audio_thread = thread::spawn(move || {
        println!("Audio Thread Started");
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        loop {
            let receiver = AUDIO_DATA_CHANNEL.1.lock().unwrap();
            for buffer_arc in receiver.try_iter() {
                let buffer = buffer_arc.lock().unwrap(); // Lock the mutex to access the data
                unsafe {
                    audio::play_audio(&sink, &*buffer, sample_rate as u32);
                }
            }
        }
    });

    // Set up libretro callbacks
    unsafe {
        (core_api.retro_init)();
        (core_api.retro_set_video_refresh)(video::libretro_set_video_refresh_callback);
        (core_api.retro_set_input_poll)(input::libretro_set_input_poll_callback);
        (core_api.retro_set_input_state)(input::libretro_set_input_state_callback);
        (core_api.retro_set_audio_sample)(audio::libretro_set_audio_sample_callback);
        (core_api.retro_set_audio_sample_batch)(audio::libretro_set_audio_sample_batch_callback);
        println!("About to load ROM: {}", &current_state.rom_name);
        libretro::load_rom_file(core_api, &current_state.rom_name);
    }

    // Prepare inputs/controllers
    let config = libretro::setup_config().unwrap();
    let key_device_map = input::key_device_map(&config);
    let joypad_device_map = input::setup_joypad_device_map();
    let gilrs = Gilrs::new().unwrap();
    let active_gamepad: &Option<GamepadId> = &None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        {
            let mut buttons = BUTTONS_PRESSED.lock().unwrap();
            let buttons_pressed = &mut buttons.0;
            let mut game_pad_active: bool = false;

            // Handle input
            if let Some(gamepad) = active_gamepad {
                input::handle_gamepad_input(
                    &joypad_device_map,
                    &gilrs,
                    &Some(*gamepad),
                    buttons_pressed,
                );
                game_pad_active = false;
            }
            input::handle_keyboard_input(
                core_api,
                &window,
                &mut current_state,
                buttons_pressed,
                &key_device_map,
                &config,
                game_pad_active
            );
        }
        unsafe {
            (core_api.retro_run)();
            // One time setup after core init
            if current_state.bytes_per_pixel == 0 {
                current_state = set_up_pixel_format(current_state);
            }

            // Render the current frame
            let rendered_frame = render_frame(current_state, window);
            current_state = rendered_frame.0;
            window = rendered_frame.1;
        }
    }
}
