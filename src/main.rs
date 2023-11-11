use minifb::{Key, Window, WindowOptions, KeyRepeat};
use std::fs::File;
use std::io::{BufReader, BufRead, Read};
use std::{fs, ptr, env, thread};
use std::time::Duration;
use rodio::{Decoder, OutputStream, Sink};
use rodio::source::{SineWave, Source};
use rodio::buffer::SamplesBuffer;
use std::ffi::{c_void, CString};
use std::path::{PathBuf, Path};
use std::collections::HashMap;
use libloading::Library;
use libretro_sys::{CoreAPI, GameInfo, PixelFormat};
use clap::Parser;
use shellexpand;
use std::sync::mpsc::{Sender,channel};

pub const DEVICE_ID_JOYPAD_B: libc::c_uint = 0;
pub const DEVICE_ID_JOYPAD_Y: libc::c_uint = 1;
pub const DEVICE_ID_JOYPAD_SELECT: libc::c_uint = 2;
pub const DEVICE_ID_JOYPAD_START: libc::c_uint = 3;
pub const DEVICE_ID_JOYPAD_UP: libc::c_uint = 4;
pub const DEVICE_ID_JOYPAD_DOWN: libc::c_uint = 5;
pub const DEVICE_ID_JOYPAD_LEFT: libc::c_uint = 6;
pub const DEVICE_ID_JOYPAD_RIGHT: libc::c_uint = 7;
pub const DEVICE_ID_JOYPAD_A: libc::c_uint = 8;
pub const DEVICE_ID_JOYPAD_X: libc::c_uint = 9;

unsafe fn play_audio( sink: &Sink, audio_samples: &Vec<i16>) {
    if sink.empty() {
        let audio_slice = std::slice::from_raw_parts(audio_samples.as_ptr() as *const i16, audio_samples.len());
        let source = SamplesBuffer::new(2, 32768, audio_slice);
        sink.append(source);
        sink.play();
        sink.sleep_until_end();
    }
}

fn get_save_state_path(save_directory: &String, game_file_name: &str, save_state_index: &u8) -> Option<PathBuf> {
    // Expand the tilde to the home directory
    let expanded_save_directory = shellexpand::tilde(save_directory);

    // Create a subdirectory named "saves" in the specified directory
    let saves_dir = PathBuf::from(expanded_save_directory.into_owned());
    if !saves_dir.exists() {
        match std::fs::create_dir_all(&saves_dir) {
            Ok(_) => {}
            Err(err) => panic!("Failed to create save directory: {:?} Error: {}", &saves_dir, err),
        }
    }

    // Generate the save state filename
    let game_name = Path::new(game_file_name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(" ", "_");
    let save_state_file_name = format!("{}_{}.state", game_name, save_state_index);

    // Combine the saves directory and the save state filename to create the full path
    let save_state_path = saves_dir.join(save_state_file_name);

    Some(save_state_path)
}

unsafe fn save_state(core_api: &CoreAPI, save_directory: &String) {
    let save_state_buffer_size =  (core_api.retro_serialize_size)();
    let mut state_buffer: Vec<u8> = vec![0; save_state_buffer_size];
    // Call retro_serialize to create the save state
    (core_api.retro_serialize)(state_buffer.as_mut_ptr() as *mut c_void, save_state_buffer_size);
    let file_path = get_save_state_path(save_directory, &CURRENT_EMULATOR_STATE.rom_name, &CURRENT_EMULATOR_STATE.current_save_slot).unwrap(); // hard coded save_slot to 0 for now
    std::fs::write(&file_path, &state_buffer).unwrap();
    println!("Save state saved to: {} with size: {}", file_path.display(), save_state_buffer_size);
}

unsafe fn load_state(core_api: &CoreAPI, save_directory: &String) {
    let file_path = get_save_state_path(save_directory, &CURRENT_EMULATOR_STATE.rom_name, &CURRENT_EMULATOR_STATE.current_save_slot).unwrap(); // Hard coded the save_slot to 0 for now
    let mut state_buffer = Vec::new();
    match File::open(&file_path) {
        Ok(mut file) => {
            // Read the save state file into a buffer
            match file.read_to_end(&mut state_buffer) {
                Ok(_) => {
                    // Call retro_unserialize to apply the save state
                    let result = (core_api.retro_unserialize)(state_buffer.as_mut_ptr() as *mut c_void, state_buffer.len() as usize);
                    if result {
                        println!("Save state loaded from: {}", &file_path.display());
                    } else {
                        println!("Failed to load save state: error code {}", result);
                    }
                }
                Err(err) => println!("Error reading save state file: {}", err),
            }
        }
        Err(_) => println!("Save state file not found"),
    }
}

fn get_retroarch_config_path() -> PathBuf {
    return match std::env::consts::OS {
        "windows" => PathBuf::from(env::var("APPDATA").ok().unwrap()).join("retroarch"),
        "macos" => PathBuf::from(env::var("HOME").ok().unwrap()).join("Library/Application Support/RetroArch"),
        _ => PathBuf::from(env::var("XDG_CONFIG_HOME").ok().unwrap()).join("retroarch"),
    };
}

fn parse_retroarch_config(config_file: &Path) -> Result<HashMap<String, String>, String> {
    let file = File::open(config_file).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let mut config_map = HashMap::new();
    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        if let Some((key, value)) = line.split_once("=") {
            config_map.insert(key.trim().to_string(), value.trim().replace("\"", "").to_string());
        }
    }
    Ok(config_map)
}

fn setup_config() -> Result<HashMap<String, String>, String> {
    let retro_arch_config_path = get_retroarch_config_path();
    let our_config = parse_retroarch_config(Path::new("./rustroarch.cfg"));
    let retro_arch_config = parse_retroarch_config(&retro_arch_config_path.join("config/retroarch.cfg"));
    let mut merged_config: HashMap<String, String> = HashMap::from([
        ("input_player1_a", "a"),
        ("input_player1_b", "s"),
        ("input_player1_x", "z"),
        ("input_player1_y", "x"),
        ("input_player1_l", "q"),
        ("input_player1_r", "w"),
        ("input_player1_down", "down"),
        ("input_player1_up", "up"),
        ("input_player1_left", "left"),
        ("input_player1_right", "right"),
        ("input_player1_select", "space"),
        ("input_player1_start", "enter"),
        ("input_reset", "h"),
        ("input_save_state", "f2"),
        ("input_load_state", "f4"),
        ("input_screenshot", "f8"),
        ("savestate_directory", "./states"),
        ]).iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    match retro_arch_config {
        Ok(config) => merged_config.extend(config),
        _ => println!("We don't have RetroArch config")
    }
    match our_config {
        Ok(config) => merged_config.extend(config),
       _ => println!("We don't have RustroArch config",)
    }
    // println!("retro_arch_config_path: {} merged_config: {:?}", retro_arch_config_path.join("config/retroarch.cfg").display(), merged_config);
    Ok(merged_config)
}

unsafe extern "C" fn libretro_set_video_refresh_callback(frame_buffer_data: *const libc::c_void, width: libc::c_uint, height: libc::c_uint, pitch: libc::size_t) {
    if (frame_buffer_data == ptr::null()) {
        println!("frame_buffer_data was null");
        return;
    }
    let length_of_frame_buffer = ((pitch as u32) * height) * CURRENT_EMULATOR_STATE.bytes_per_pixel as u32;
    let buffer_slice = std::slice::from_raw_parts(frame_buffer_data as *const u8, length_of_frame_buffer as usize);
    let result = convert_pixel_array_from_rgb565_to_xrgb8888(buffer_slice);

    // Create a Vec<u8> from the slice
    let buffer_vec = Vec::from(result);

    // Wrap the Vec<u8> in an Some Option and assign it to the frame_buffer field
    CURRENT_EMULATOR_STATE.frame_buffer = Some(buffer_vec);
    CURRENT_EMULATOR_STATE.screen_height = height;
    CURRENT_EMULATOR_STATE.screen_width = width;
    CURRENT_EMULATOR_STATE.screen_pitch = pitch as u32;
}

unsafe extern "C" fn libretro_set_input_poll_callback() {
    println!("libretro_set_input_poll_callback")
}

unsafe extern "C" fn libretro_set_input_state_callback(port: libc::c_uint, device: libc::c_uint, index: libc::c_uint, id: libc::c_uint) -> i16 {
    // println!("libretro_set_input_state_callback port: {} device: {} index: {} id: {}", port, device, index, id);
    let is_pressed = match &CURRENT_EMULATOR_STATE.buttons_pressed {
        Some(buttons_pressed) => buttons_pressed[id as usize],
        None => 0
    };

    return is_pressed;
}

unsafe extern "C" fn libretro_set_audio_sample_callback(left: i16, right: i16) {
    println!("libretro_set_audio_sample_callback");
}

const AUDIO_CHANNELS: usize = 2; // left and right
unsafe extern "C" fn libretro_set_audio_sample_batch_callback(
    audio_data: *const i16,
    frames: libc::size_t,
) -> libc::size_t {
    let audio_slice = std::slice::from_raw_parts(audio_data, frames * AUDIO_CHANNELS);
    CURRENT_EMULATOR_STATE.audio_data = Some(audio_slice.to_vec());
    return frames;
}

unsafe fn send_audio_to_thread(sender: &Sender<&Vec<i16>>) {
    // Send the audio samples to the audio thread using the channel
    match &CURRENT_EMULATOR_STATE.audio_data {
        Some(data) => {
            sender.send(data).unwrap();
        },
        None => {},
    }; 
}

pub struct EmulatorPixelFormat(PixelFormat);

impl Default for EmulatorPixelFormat {
    fn default() -> Self {
        EmulatorPixelFormat(PixelFormat::ARGB8888)
    }
}


#[derive(Parser)]
struct EmulatorState {
    #[arg(help = "Sets the path to the ROM file to load", index = 1)]
    rom_name: String,
    #[arg(short = 'L', default_value = "default_library")]
    library_name: String,
    #[arg(skip)]
    frame_buffer: Option<Vec<u32>>,
    #[arg(skip)]
    pixel_format: EmulatorPixelFormat,
    #[arg(skip)]
    bytes_per_pixel: u8,
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
    audio_data: Option<Vec<i16>>,
}

static mut CURRENT_EMULATOR_STATE: EmulatorState = EmulatorState {
    rom_name: String::new(),
    library_name: String::new(),
    frame_buffer: None,
    pixel_format: EmulatorPixelFormat(PixelFormat::ARGB8888),
    bytes_per_pixel: 4,
    screen_pitch: 0,
    screen_width: 0,
    screen_height: 0,
    buttons_pressed: None,
    current_save_slot: 0,
    audio_data: None
};

fn parse_command_line_arguments() -> EmulatorState {
    let emulator_state = EmulatorState::parse();

    println!("ROM name: {}", emulator_state.rom_name);
    println!("Core Library name: {}", emulator_state.library_name);
    return emulator_state;
}

unsafe fn load_rom_file(core_api: &CoreAPI, rom_name: &String) -> bool {
    let rom_name_cptr = CString::new(rom_name.clone()).expect("Failed to create CString").as_ptr();
    let contents = fs::read(rom_name).expect("Failed to read file");
    let data: *const c_void = contents.as_ptr() as *const c_void;
    let game_info = GameInfo {
        path: rom_name_cptr,
        data,
        size: contents.len(),
        meta: ptr::null(),
    };
    let was_load_successful = (core_api.retro_load_game)(&game_info);
    if(!was_load_successful) {
        panic!("Rom Load was not successful");
    }
    return was_load_successful;
}


pub type EnvironmentCallback = unsafe extern "C" fn(command: libc::c_uint, data: *mut libc::c_void) -> bool;

unsafe extern "C" fn libretro_environment_callback(command: u32, return_data: *mut c_void) -> bool {
    match command{
        libretro_sys::ENVIRONMENT_GET_CAN_DUPE => {
            *(return_data as *mut bool) = true; // Set the return_data to the value true
            println!("ENVIRONMENT_GET_CAN_DUPE");
        },
        libretro_sys::ENVIRONMENT_SET_PIXEL_FORMAT => {
            let pixel_format = *(return_data as *const u32);
            let pixel_format_as_enum = PixelFormat::from_uint(pixel_format).unwrap();
            CURRENT_EMULATOR_STATE.pixel_format.0 = pixel_format_as_enum;
            match pixel_format_as_enum {
                PixelFormat::ARGB1555 => {
                    println!("Core will send us pixel data in the RETRO_PIXEL_FORMAT_0RGB1555 format");
                    CURRENT_EMULATOR_STATE.bytes_per_pixel = 2;
                },
                PixelFormat::RGB565 => {
                    println!("Core will send us pixel data in the RETRO_PIXEL_FORMAT_RGB565 format");
                    CURRENT_EMULATOR_STATE.bytes_per_pixel = 2;
                }
                PixelFormat::ARGB8888 => {
                    println!("Core will send us pixel data in the RETRO_PIXEL_FORMAT_XRGB8888 format");
                    CURRENT_EMULATOR_STATE.bytes_per_pixel = 4;
                },
                _ => {
                    panic!("Core is trying to use an Unknown Pixel Format")
                }
            }
            return true
        },
        _ => println!("libretro_environment_callback Called with command: {}", command)
    }
    false
}

fn convert_pixel_array_from_rgb565_to_xrgb8888(color_array: &[u8]) -> Box<[u32]> {
    let bytes_per_pixel = 2;
    assert_eq!(color_array.len() % bytes_per_pixel, 0, "color_array length must be a multiple of 2 (16-bits per pixel)");

    let num_pixels = color_array.len() / bytes_per_pixel;
    let mut result = vec![0u32; num_pixels];

    for i in 0..num_pixels {
        // This Rust code is decoding a 16-bit color value, represented by two bytes of data, into its corresponding red, green, and blue components.
        let first_byte = color_array[bytes_per_pixel*i];
        let second_byte = color_array[(bytes_per_pixel*i)+1];

        // First extract the red component from the first byte. The first byte contains the most significant 8 bits of the 16-bit color value. The & operator performs a bitwise AND operation on first_byte and 0b1111_1000, which extracts the 5 most significant bits of the byte. The >> operator then shifts the extracted bits to the right by 3 positions, effectively dividing by 8, to get the value of the red component on a scale of 0-31.
        let red = (first_byte & 0b1111_1000) >> 3;
        // Next extract the green component from both bytes. The first part of the expression ((first_byte & 0b0000_0111) << 3) extracts the 3 least significant bits of first_byte and shifts them to the left by 3 positions, effectively multiplying by 8. The second part of the expression ((second_byte & 0b1110_0000) >> 5) extracts the 3 most significant bits of second_byte and shifts them to the right by 5 positions, effectively dividing by 32. The two parts are then added together to get the value of the green component on a scale of 0-63.
        let green = ((first_byte & 0b0000_0111) << 3) + ((second_byte & 0b1110_0000) >> 5);
        // Next extract the blue component from the second byte. The & operator performs a bitwise AND operation on second_byte and 0b0001_1111, which extracts the 5 least significant bits of the byte. This gives the value of the blue component on a scale of 0-31.
        let blue = second_byte & 0b0001_1111;

        // Use high bits for empty low bits as we have more bits available in XRGB8888
        let red = (red << 3) | (red >> 2);
        let green = (green << 2) | (green >> 3);
        let blue = (blue << 3) | (blue >> 2);

        // Finally save the pixel data in the result array as an XRGB8888 value
        result[i] = ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32);
    }

    result.into_boxed_slice()
}

const EXPECTED_LIB_RETRO_VERSION: u32 = 1;

struct Core {
    dylib: Library,
    api: CoreAPI
}

impl Core {
    fn new(core_name : &String) -> Self {
        unsafe {
            let dylib = Library::new(core_name).expect("Failed to load Core");
    
            let core_api = CoreAPI {
                retro_set_environment: *(dylib.get(b"retro_set_environment").unwrap()),
                retro_set_video_refresh: *(dylib.get(b"retro_set_video_refresh").unwrap()),
                retro_set_audio_sample: *(dylib.get(b"retro_set_audio_sample").unwrap()),
                retro_set_audio_sample_batch: *(dylib.get(b"retro_set_audio_sample_batch").unwrap()),
                retro_set_input_poll: *(dylib.get(b"retro_set_input_poll").unwrap()),
                retro_set_input_state: *(dylib.get(b"retro_set_input_state").unwrap()),
    
                retro_init: *(dylib.get(b"retro_init").unwrap()),
                retro_deinit: *(dylib.get(b"retro_deinit").unwrap()),
    
                retro_api_version: *(dylib.get(b"retro_api_version").unwrap()),
    
                retro_get_system_info: *(dylib.get(b"retro_get_system_info").unwrap()),
                retro_get_system_av_info: *(dylib.get(b"retro_get_system_av_info").unwrap()),
                retro_set_controller_port_device: *(dylib.get(b"retro_set_controller_port_device").unwrap()),
    
                retro_reset: *(dylib.get(b"retro_reset").unwrap()),
                retro_run: *(dylib.get(b"retro_run").unwrap()),
    
                retro_serialize_size: *(dylib.get(b"retro_serialize_size").unwrap()),
                retro_serialize: *(dylib.get(b"retro_serialize").unwrap()),
                retro_unserialize: *(dylib.get(b"retro_unserialize").unwrap()),
    
                retro_cheat_reset: *(dylib.get(b"retro_cheat_reset").unwrap()),
                retro_cheat_set: *(dylib.get(b"retro_cheat_set").unwrap()),
    
                retro_load_game: *(dylib.get(b"retro_load_game").unwrap()),
                retro_load_game_special: *(dylib.get(b"retro_load_game_special").unwrap()),
                retro_unload_game: *(dylib.get(b"retro_unload_game").unwrap()),
    
                retro_get_region: *(dylib.get(b"retro_get_region").unwrap()),
                retro_get_memory_data: *(dylib.get(b"retro_get_memory_data").unwrap()),
                retro_get_memory_size: *(dylib.get(b"retro_get_memory_size").unwrap()),
            };
    
            let api_version = (core_api.retro_api_version)();
            println!("API Version: {}", api_version);
            if api_version != EXPECTED_LIB_RETRO_VERSION {
                panic!("The Core has been compiled with a LibRetro API that is unexpected, we expected version to be: {} but it was: {}", EXPECTED_LIB_RETRO_VERSION, api_version)
            }
            (core_api.retro_set_environment)(libretro_environment_callback);
            (core_api.retro_init)();
            
            // Construct and return a Core instance
            Core {
                dylib,
                api: core_api
            }
    }
}}

impl Drop for Core {
    fn drop(&mut self) {
        // If you need to do any cleanup when the Core is dropped, do it here.
    }
}

const WIDTH: usize = 256;
const HEIGHT: usize = 140;

fn main() {
    unsafe { CURRENT_EMULATOR_STATE = parse_command_line_arguments()};

    let mut window = Window::new("Rust Game", WIDTH, HEIGHT, WindowOptions::default())
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

    window.limit_update_rate(Some(std::time::Duration::from_micros(16600))); // ~60fps

    let core = Core::new(unsafe { &CURRENT_EMULATOR_STATE.library_name });
    let core_api = &core.api;
    // Audio Setup
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();

    // Create a channel for passing audio samples from the main thread to the audio thread
    let (sender, receiver) = channel();

    // Spawn a new thread to play back audio
    let audio_thread = thread::spawn(move || {
        let sink = Sink::try_new(&stream_handle).unwrap();
        loop {
            // Receive the next set of audio samples from the channel
            let audio_samples = receiver.recv().unwrap();
            unsafe { play_audio(&sink, audio_samples); } // pass the audio samples to the play_audio function
        }
    });

    unsafe {
        (core_api.retro_init)();
        (core_api.retro_set_video_refresh)(libretro_set_video_refresh_callback);
        (core_api.retro_set_input_poll)(libretro_set_input_poll_callback);
        (core_api.retro_set_input_state)(libretro_set_input_state_callback);
        (core_api.retro_set_audio_sample)(libretro_set_audio_sample_callback);
        (core_api.retro_set_audio_sample_batch)(libretro_set_audio_sample_batch_callback);
        println!("About to load ROM: {}", &CURRENT_EMULATOR_STATE.rom_name);
        load_rom_file(core_api, &CURRENT_EMULATOR_STATE.rom_name);
    }

    let mut this_frames_pressed_buttons = vec![0; 16];

    let config = setup_config().unwrap();

    let key_device_map = HashMap::from([
            (&config["input_player1_a"], libretro_sys::DEVICE_ID_JOYPAD_A as usize),
            (&config["input_player1_b"], libretro_sys::DEVICE_ID_JOYPAD_B as usize),
            (&config["input_player1_x"], libretro_sys::DEVICE_ID_JOYPAD_X as usize),
            (&config["input_player1_y"], libretro_sys::DEVICE_ID_JOYPAD_Y as usize),
            (&config["input_player1_l"], libretro_sys::DEVICE_ID_JOYPAD_L as usize),
            (&config["input_player1_r"], libretro_sys::DEVICE_ID_JOYPAD_R as usize),
            (&config["input_player1_down"], libretro_sys::DEVICE_ID_JOYPAD_DOWN as usize),
            (&config["input_player1_up"], libretro_sys::DEVICE_ID_JOYPAD_UP as usize),
            (&config["input_player1_right"], libretro_sys::DEVICE_ID_JOYPAD_RIGHT as usize),
            (&config["input_player1_left"], libretro_sys::DEVICE_ID_JOYPAD_LEFT as usize),
            (&config["input_player1_start"], libretro_sys::DEVICE_ID_JOYPAD_START as usize),
            (&config["input_player1_select"], libretro_sys::DEVICE_ID_JOYPAD_SELECT as usize),
    ]);


    while window.is_open() && !window.is_key_down(Key::Escape) {
        let mini_fb_keys_pressed = window.get_keys_pressed(KeyRepeat::No);
        if !mini_fb_keys_pressed.is_empty(){
            for key in mini_fb_keys_pressed {
                let key_as_string = format!("{:?}", key).to_ascii_lowercase();

                if let Some(device_id) = key_device_map.get(&key_as_string) {
                    this_frames_pressed_buttons[*device_id] = 1;
                }
                if &key_as_string == &config["input_save_state"] {
                    unsafe { save_state(&core_api,  &config["savestate_directory"]); } // f2
                    continue;
                } 
                if &key_as_string == &config["input_load_state"] {
                    unsafe { load_state(&core_api,  &config["savestate_directory"]); } // f4
                    continue;
                }
                if &key_as_string == &config["input_state_slot_increase"] {
                    unsafe{
                        if CURRENT_EMULATOR_STATE.current_save_slot != 255 {
                            CURRENT_EMULATOR_STATE.current_save_slot += 1;
                            println!("Current save slot increased to: {}", CURRENT_EMULATOR_STATE.current_save_slot);
                        }
                    }
                    continue;
                }
                
                if &key_as_string == &config["input_state_slot_decrease"] {
                    unsafe{
                        if CURRENT_EMULATOR_STATE.current_save_slot != 0 {
                            CURRENT_EMULATOR_STATE.current_save_slot -= 1;
                            println!("Current save slot decreased to: {}", CURRENT_EMULATOR_STATE.current_save_slot);
                        }
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
                println!("Unhandled Key Pressed: {} input_player1_a: {}", key_as_string, config["input_player1_a"]);
            }
        }

       unsafe {
        (core_api.retro_run)();

        send_audio_to_thread(&sender);

        match &CURRENT_EMULATOR_STATE.frame_buffer {
            Some(buffer) => {
                let width = (CURRENT_EMULATOR_STATE.screen_pitch / CURRENT_EMULATOR_STATE.bytes_per_pixel as u32) as usize;
                let height = CURRENT_EMULATOR_STATE.screen_height as usize;
                let slice_of_pixel_buffer: &[u32] = std::slice::from_raw_parts(buffer.as_ptr() as *const u32, buffer.len()); // convert to &[u32] slice reference
                if slice_of_pixel_buffer.len() < width*height*4 {
                     // The frame buffer isn't big enough so lets add additional pixels just so we can display it
                     let mut vec: Vec<u32> = slice_of_pixel_buffer.to_vec();
                     vec.resize((width*height*4) as usize, 0x0000FFFF); // Add any missing pixels with colour blue
                     window.update_with_buffer(&vec, width, height).unwrap();
                } else{
                    let _ = window.update_with_buffer(&slice_of_pixel_buffer, width, height);
                }
            }
            None => {
                println!("We don't have a buffer to display");
            }
        }
        CURRENT_EMULATOR_STATE.buttons_pressed = Some(this_frames_pressed_buttons.clone());
       }
    }
}
