mod audio;
mod input;
mod libretro;
mod video;
use audio::AudioBuffer;
use clap::Parser;
use gilrs::{Event, Gilrs};
use libretro_sys::{CoreAPI, GameInfo, PixelFormat, SystemAvInfo};
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use once_cell::sync::Lazy;
use rodio::{OutputStream, Sink};
use std::ffi::{c_void, CString};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::{fs, ptr, thread};

static BUTTONS_PRESSED: Mutex<(Vec<i16>, Vec<i16>)> = Mutex::new((vec![], vec![]));

static BYTES_PER_PIXEL: AtomicU8 = AtomicU8::new(4); // Default value of 4

static PIXEL_FORMAT_CHANNEL: Lazy<(Sender<PixelFormat>, Arc<Mutex<Receiver<PixelFormat>>>)> =
    Lazy::new(|| {
        let (sender, receiver) = channel::<PixelFormat>();
        (sender, Arc::new(Mutex::new(receiver)))
    });

static VIDEO_DATA_CHANNEL: Lazy<(Sender<VideoData>, Arc<Mutex<Receiver<VideoData>>>)> = Lazy::new(|| {
    let (sender, receiver) = channel::<VideoData>();
    (sender, Arc::new(Mutex::new(receiver)))
});

static AUDIO_DATA_CHANNEL: Lazy<(Sender<Arc<Mutex<AudioBuffer>>>, Arc<Mutex<Receiver<Arc<Mutex<AudioBuffer>>>>>)> = Lazy::new(|| {
    let (sender, receiver) = channel::<Arc<Mutex<AudioBuffer>>>();
    (sender, Arc::new(Mutex::new(receiver)))
});

struct VideoData {
    frame_buffer: Vec<u32>,
    width: u32,
    height: u32,
    pitch: u32,
}

#[derive(Parser)]
pub struct EmulatorState {
    #[arg(help = "Sets the path to the ROM file to load", index = 1)]
    rom_name: String,
    #[arg(short = 'L', default_value = "default_library")]
    library_name: String,
    #[arg(skip)]
    frame_buffer: Option<Vec<u32>>,
    #[arg(skip)]
    screen_pitch: u32,
    #[arg(skip)]
    screen_width: u32,
    #[arg(skip)]
    screen_height: u32,
    #[arg(skip)]
    current_save_slot: u8,
    #[arg(skip)]
    av_info: Option<SystemAvInfo>,
    #[arg(skip)]
    pixel_format: video::EmulatorPixelFormat,
    #[arg(skip)]
    bytes_per_pixel: u8,
}

fn parse_command_line_arguments() -> (String, String) {
    let emulator_state = EmulatorState::parse();

    println!("ROM name: {}", emulator_state.rom_name);
    println!("Core Library name: {}", emulator_state.library_name);

    (emulator_state.rom_name, emulator_state.library_name)
}

pub unsafe fn load_rom_file(core_api: &CoreAPI, rom_name: &String) -> bool {
    let cstr_rom_name = CString::new(rom_name.clone()).expect("Failed to create CString");
    let contents = fs::read(rom_name).expect("Failed to read file");
    let data: *const c_void = contents.as_ptr() as *const c_void;

    let game_info = GameInfo {
        path: cstr_rom_name.as_ptr(),
        data,
        size: contents.len(),
        meta: ptr::null(),
    };

    let was_load_successful = (core_api.retro_load_game)(&game_info);
    if !was_load_successful {
        panic!("Rom Load was not successful");
    }
    return was_load_successful;
}

const WIDTH: usize = 256;
const HEIGHT: usize = 140;

fn main() {
    let (rom_name, library_name) = parse_command_line_arguments();
    let mut current_state = EmulatorState {
        rom_name,
        library_name,
        frame_buffer: None,
        screen_pitch: 0,
        screen_width: 0,
        screen_height: 0,
        current_save_slot: 0,
        av_info: None,
        pixel_format: video::EmulatorPixelFormat(PixelFormat::ARGB8888),
        bytes_per_pixel: 4,
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

    unsafe {
        (core_api.retro_init)();
        (core_api.retro_set_video_refresh)(video::libretro_set_video_refresh_callback);
        (core_api.retro_set_input_poll)(input::libretro_set_input_poll_callback);
        (core_api.retro_set_input_state)(input::libretro_set_input_state_callback);
        (core_api.retro_set_audio_sample)(audio::libretro_set_audio_sample_callback);
        (core_api.retro_set_audio_sample_batch)(audio::libretro_set_audio_sample_batch_callback);
        println!("About to load ROM: {}", &current_state.rom_name);
        load_rom_file(core_api, &current_state.rom_name);
    }

    let mut this_frames_pressed_buttons = vec![0; 16];

    let config = libretro::setup_config().unwrap();

    let key_device_map = input::key_device_map(&config);

    let joypad_device_map = input::setup_joypad_device_map();

    let mut gilrs = Gilrs::new().unwrap();

    // Iterate over all connected gamepads
    for (_id, gamepad) in gilrs.gamepads() {
        println!("{} is {:?}", gamepad.name(), gamepad.power_info());
    }

    let mut active_gamepad = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Gamepad input Handling
        // Examine new events to check which gamepad is currently being used
        while let Some(Event { id, .. }) = gilrs.next_event() {
            // println!("{:?} New event from {}: {:?}", time, id, event);
            active_gamepad = Some(id);
        }

        // Now Lets check what buttons are pressed and map them to the libRetro buttons
        if let Some(gamepad) = active_gamepad.map(|id| gilrs.gamepad(id)) {
            for button in input::BUTTON_ARRAY {
                if gamepad.is_pressed(button) {
                    println!("Button Pressed: {:?}", button);
                    let libretro_button = joypad_device_map.get(&button).unwrap();
                    this_frames_pressed_buttons[*libretro_button] = 1;
                }
            }
        }

        let mini_fb_keys_pressed = window.get_keys_pressed(KeyRepeat::No);
        if !mini_fb_keys_pressed.is_empty() {
            for key in mini_fb_keys_pressed {
                let key_as_string = format!("{:?}", key).to_ascii_lowercase();

                if let Some(device_id) = key_device_map.get(&key_as_string) {
                    this_frames_pressed_buttons[*device_id] = 1;
                }
                if &key_as_string == &config["input_save_state"] {
                    unsafe {
                        libretro::save_state(
                            &core_api,
                            &config["savestate_directory"],
                            &current_state.rom_name,
                            &current_state.current_save_slot,
                        );
                    } // f2
                    continue;
                }
                if &key_as_string == &config["input_load_state"] {
                    unsafe {
                        libretro::load_state(
                            &core_api,
                            &config["savestate_directory"],
                            &current_state.rom_name,
                            &current_state.current_save_slot,
                        );
                    } // f4
                    continue;
                }
                if &key_as_string == &config["input_state_slot_increase"] {
                    if current_state.current_save_slot != 255 {
                        current_state.current_save_slot += 1;
                        println!(
                            "Current save slot increased to: {}",
                            current_state.current_save_slot
                        );
                    }

                    continue;
                }

                if &key_as_string == &config["input_state_slot_decrease"] {
                    if current_state.current_save_slot != 0 {
                        current_state.current_save_slot -= 1;
                        println!(
                            "Current save slot decreased to: {}",
                            current_state.current_save_slot
                        );
                    }

                    continue;
                }

                println!("Unhandled Key Pressed: {} ", key_as_string);
            }
        }

        let mini_fb_keys_released = window.get_keys_released();
        for key in &mini_fb_keys_released {
            let key_as_string = format!("{:?}", key).to_ascii_lowercase();

            if let Some(device_id) = key_device_map.get(&key_as_string) {
                this_frames_pressed_buttons[*device_id] = 0;
            } else {
                println!(
                    "Unhandled Key Pressed: {} input_player1_a: {}",
                    key_as_string, config["input_player1_a"]
                );
            }
        }

        unsafe {
            (core_api.retro_run)();
            let pixel_format_receiver = &PIXEL_FORMAT_CHANNEL.1.lock().unwrap();
            let video_data_receiver = VIDEO_DATA_CHANNEL.1.lock().unwrap();

            for pixel_format in pixel_format_receiver.try_iter() {
                current_state.pixel_format.0 = pixel_format;
                let bpp = match pixel_format {
                    PixelFormat::ARGB1555 | PixelFormat::RGB565 => 2,
                    PixelFormat::ARGB8888 => 4,
                };
                println!("Core will send us pixel data in format {:?}", pixel_format);
                BYTES_PER_PIXEL.store(bpp, Ordering::SeqCst);
                current_state.bytes_per_pixel = bpp;
            }

            for video_data in video_data_receiver.try_iter() {
                current_state.frame_buffer = Some(video_data.frame_buffer);
                current_state.screen_height = video_data.height;
                current_state.screen_width = video_data.width;
                current_state.screen_pitch = video_data.pitch;
            }

            match &current_state.frame_buffer {
                Some(buffer) => {
                    let width = (current_state.screen_pitch / current_state.bytes_per_pixel as u32)
                        as usize;
                    let height = current_state.screen_height as usize;
                    let slice_of_pixel_buffer: &[u32] =
                        std::slice::from_raw_parts(buffer.as_ptr() as *const u32, buffer.len()); // convert to &[u32] slice reference
                    if slice_of_pixel_buffer.len() < width * height * 4 {
                        // The frame buffer isn't big enough so lets add additional pixels just so we can display it
                        let mut vec: Vec<u32> = slice_of_pixel_buffer.to_vec();
                        vec.resize((width * height * 4) as usize, 0x0000FFFF); // Add any missing pixels with colour blue
                        window.update_with_buffer(&vec, width, height).unwrap();
                    } else {
                        let _ = window.update_with_buffer(&slice_of_pixel_buffer, width, height);
                    }
                }
                None => {
                    println!("We don't have a buffer to display");
                }
            }

            {
                let mut buttons = BUTTONS_PRESSED.lock().unwrap();
                buttons.0 = this_frames_pressed_buttons.clone(); // Update the current frame's inputs
            }
        }
    }
}
