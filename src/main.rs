// This implementation is based on the guide provided by [RetroGameDeveloper/RetroReversing].
// Original guide can be found at [https://www.retroreversing.com/CreateALibRetroFrontEndInRust].
// Copyright (c) 2023 Nicholas Ricciuti

// Import necessary modules from other files and crates
mod audio;
mod input;
mod libretro;
mod video;
use audio::AudioBuffer;
use gilrs::{GamepadId, Gilrs};
use libretro_sys::PixelFormat;
use minifb::{Key, Window, WindowOptions};
use once_cell::sync::Lazy;
use rodio::{OutputStream, Sink};
use std::sync::atomic::AtomicU8;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

// Define global static variables for handling input, pixel format, video, and audio data
static BUTTONS_PRESSED: Lazy<Mutex<(Vec<i16>, Vec<i16>)>> =
    Lazy::new(|| Mutex::new((vec![0; 16], vec![0; 16])));
static BYTES_PER_PIXEL: AtomicU8 = AtomicU8::new(4); // Default value for bytes per pixel
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

// Structure to hold video data
struct VideoData {
    frame_buffer: Vec<u32>,
    width: u32,
    height: u32,
    pitch: u32,
}

// The main function, entry point of the application
fn main() {
    // Parse command line arguments to get ROM and library names
    let (rom_name, library_name) = libretro::parse_command_line_arguments();
    // Initialize emulator state with default values
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

    // Create a new window with specific options
    let mut window = Window::new(
        "Test", // Window title
        256,    // Window width
        144,    // Window height
        WindowOptions {
            resize: true, // Allow window resizing
            ..WindowOptions::default()
        },
    )
    .expect("Unable to open Window");

    // Limit window update rate to approximately 60 frames per second
    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    // Initialize the core of the emulator and update the emulator state
    let (core, updated_state) = libretro::Core::new(current_state);
    let core_api = &core.api; // Reference to the core API
    current_state = updated_state;

    // Extract the audio sample rate from the emulator state
    let sample_rate = current_state
        .av_info
        .as_ref()
        .map_or(0.0, |av_info| av_info.timing.sample_rate);

    // Spawn a new thread for audio handling
    let _audio_thread = thread::spawn(move || {
        println!("Audio Thread Started");
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        loop {
            let receiver = AUDIO_DATA_CHANNEL.1.lock().unwrap();
            // Play audio in a loop
            for buffer_arc in receiver.try_iter() {
                let buffer = buffer_arc.lock().unwrap();
                unsafe {
                    audio::play_audio(&sink, &*buffer, sample_rate as u32);
                }
            }
        }
    });

    // Set up libretro callbacks for video, input, and audio
    unsafe {
        (core_api.retro_init)();
        (core_api.retro_set_video_refresh)(video::libretro_set_video_refresh_callback);
        (core_api.retro_set_input_poll)(input::libretro_set_input_poll_callback);
        (core_api.retro_set_input_state)(input::libretro_set_input_state_callback);
        (core_api.retro_set_audio_sample)(audio::libretro_set_audio_sample_callback);
        (core_api.retro_set_audio_sample_batch)(audio::libretro_set_audio_sample_batch_callback);
        println!("About to load ROM: {}", &current_state.rom_name);
        // Load the ROM file
        libretro::load_rom_file(core_api, &current_state.rom_name);
    }

    // Prepare configurations for input handling
    let config = libretro::setup_config().unwrap();
    let key_device_map = input::key_device_map(&config);
    let joypad_device_map = input::setup_joypad_device_map();
    let gilrs = Gilrs::new().unwrap(); // Initialize gamepad handling
    let active_gamepad: &Option<GamepadId> = &None;

    // Main application loop
    while window.is_open() && !window.is_key_down(Key::Escape) {
        {
            let mut buttons = BUTTONS_PRESSED.lock().unwrap();
            let buttons_pressed = &mut buttons.0;
            let mut game_pad_active: bool = false;

            // Handle gamepad and keyboard input
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
                game_pad_active,
            );
        }
        unsafe {
            // Run one frame of the emulator
            (core_api.retro_run)();
            // If needed, set up pixel format
            if current_state.bytes_per_pixel == 0 {
                current_state = video::set_up_pixel_format(current_state);
            }

            // Render the frame
            let rendered_frame = video::render_frame(current_state, window);
            current_state = rendered_frame.0;
            window = rendered_frame.1;
        }
    }
}
