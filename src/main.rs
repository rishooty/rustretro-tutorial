mod audio;
mod input;
mod libretro;
mod video;
use clap::Parser;
use gilrs::{Event, Gilrs};
use libretro_sys::{CoreAPI, GameInfo, PixelFormat, SystemAvInfo};
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use once_cell::sync::Lazy;
use rodio::{OutputStream, Sink};
use std::ffi::{c_void, CString};
use std::sync::mpsc::{self, Sender, channel, Receiver};
use std::sync::{Arc, Mutex};
use std::{fs, ptr, thread};

static PIXEL_FORMAT_CHANNEL: Lazy<(Sender<PixelFormat>, Arc<Mutex<Receiver<PixelFormat>>>)> = Lazy::new(|| {
    let (sender, receiver) = mpsc::channel::<PixelFormat>();
    (sender, Arc::new(Mutex::new(receiver)))
});

static mut PIXEL_FORMAT: video::EmulatorPixelFormat = video::EmulatorPixelFormat(PixelFormat::ARGB8888);
static mut BYTES_PER_PIXEL: u8 = 4;

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
    buttons_pressed: Option<Vec<i16>>,
    #[arg(skip)]
    current_save_slot: u8,
    #[arg(skip)]
    audio_data: Option<Arc<Mutex<Vec<i16>>>>,
    #[arg(skip)]
    av_info: Option<SystemAvInfo>,
}

static CURRENT_STATE: Lazy<Arc<Mutex<EmulatorState>>> = Lazy::new(|| {
    Arc::new(Mutex::new(EmulatorState {
        rom_name: String::new(),
        library_name: String::new(),
        frame_buffer: None,
        screen_pitch: 0,
        screen_width: 0,
        screen_height: 0,
        buttons_pressed: None,
        current_save_slot: 0,
        audio_data: None,
        av_info: None,
    }))
});

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
    {
        let mut state = CURRENT_STATE.lock().unwrap();
        state.rom_name = rom_name;
        state.library_name = library_name;
    }

    let mut window = Window::new("Rust Game", WIDTH, HEIGHT, WindowOptions::default())
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

    window.limit_update_rate(Some(std::time::Duration::from_micros(16600))); // ~60fps

    let core = libretro::Core::new();
    let core_api = &core.api;

    // Create a channel for passing audio samples from the main thread to the audio thread
    let (audio_sender, audio_receiver) = channel::<Arc<Mutex<Vec<i16>>>>();

    // Extract the sample_rate before spawning the thread
    let sample_rate = {
        let state = CURRENT_STATE.lock().unwrap();
        state
            .av_info
            .as_ref()
            .map_or(0.0, |av_info| av_info.timing.sample_rate)
    };

    let _audio_thread = thread::spawn(move || {
        println!("Audio Thread Started");
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();
        loop {
            // Receive the next set of audio samples from the channel
            let buffer_arc = audio_receiver.recv().unwrap();
            let buffer = buffer_arc.lock().unwrap(); // Lock the mutex to access the data
            unsafe {
                audio::play_audio(&sink, &*buffer, sample_rate as u32);
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
        {
            let state = CURRENT_STATE.lock().unwrap();
            println!("About to load ROM: {}", &state.rom_name);
            load_rom_file(core_api, &state.rom_name);
        }
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
        {
            let mut state = CURRENT_STATE.lock().unwrap();

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
                                &state.rom_name,
                                &state.current_save_slot,
                            );
                        } // f2
                        continue;
                    }
                    if &key_as_string == &config["input_load_state"] {
                        unsafe {
                            libretro::load_state(
                                &core_api,
                                &config["savestate_directory"],
                                &state.rom_name,
                                &state.current_save_slot,
                            );
                        } // f4
                        continue;
                    }
                    if &key_as_string == &config["input_state_slot_increase"] {
                        if state.current_save_slot != 255 {
                            state.current_save_slot += 1;
                            println!(
                                "Current save slot increased to: {}",
                                state.current_save_slot
                            );
                        }

                        continue;
                    }

                    if &key_as_string == &config["input_state_slot_decrease"] {
                        if state.current_save_slot != 0 {
                            state.current_save_slot -= 1;
                            println!(
                                "Current save slot decreased to: {}",
                                state.current_save_slot
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

            audio::send_audio_to_thread(&audio_sender, &state);
        }

        unsafe {
            (core_api.retro_run)();
            let pixel_format_receiver = &PIXEL_FORMAT_CHANNEL.1.lock().unwrap();

            for pixel_format in  pixel_format_receiver.try_iter() {
                PIXEL_FORMAT.0 = pixel_format;
                match pixel_format {
                    PixelFormat::ARGB1555 => {
                        println!(
                            "Core will send us pixel data in the RETRO_PIXEL_FORMAT_0RGB1555 format"
                        );
                        BYTES_PER_PIXEL = 2;
                    }
                    PixelFormat::RGB565 => {
                        println!(
                            "Core will send us pixel data in the RETRO_PIXEL_FORMAT_RGB565 format"
                        );
                        BYTES_PER_PIXEL = 2;
                    }
                    PixelFormat::ARGB8888 => {
                        println!(
                            "Core will send us pixel data in the RETRO_PIXEL_FORMAT_XRGB8888 format"
                        );
                        BYTES_PER_PIXEL = 4;
                    }
                }
                // Handle the pixel format update, e.g., updating bytes_per_pixel
            }
            {
                let mut state = CURRENT_STATE.lock().unwrap();
                match &state.frame_buffer {
                    Some(buffer) => {
                        let width = (state.screen_pitch / BYTES_PER_PIXEL as u32) as usize;
                        let height = state.screen_height as usize;
                        let slice_of_pixel_buffer: &[u32] =
                            std::slice::from_raw_parts(buffer.as_ptr() as *const u32, buffer.len()); // convert to &[u32] slice reference
                        if slice_of_pixel_buffer.len() < width * height * 4 {
                            // The frame buffer isn't big enough so lets add additional pixels just so we can display it
                            let mut vec: Vec<u32> = slice_of_pixel_buffer.to_vec();
                            vec.resize((width * height * 4) as usize, 0x0000FFFF); // Add any missing pixels with colour blue
                            window.update_with_buffer(&vec, width, height).unwrap();
                        } else {
                            let _ =
                                window.update_with_buffer(&slice_of_pixel_buffer, width, height);
                        }
                    }
                    None => {
                        println!("We don't have a buffer to display");
                    }
                }

                state.buttons_pressed = Some(this_frames_pressed_buttons.clone());
            }
        }
    }
}
