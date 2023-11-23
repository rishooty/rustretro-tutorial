// libretro.rs
//
// This module provides the interface to the libretro core, including functions for
// loading ROMs, managing save states, and handling configurations.

use crate::PIXEL_FORMAT_CHANNEL;
use crate::video;
use clap::Parser;
use libc::c_void;
use libloading::Library;
use libretro_sys::GameInfo;
use libretro_sys::{CoreAPI, GameGeometry, PixelFormat, SystemAvInfo, SystemTiming};
use std::ffi::CString;
use std::fs;
use std::ptr;
use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

// Expected version of the libretro API.
const EXPECTED_LIB_RETRO_VERSION: u32 = 1;

// Represents the emulator state and configuration.
#[derive(Parser)]
pub struct EmulatorState {
    // Path to the ROM file to be loaded.
    #[arg(help = "Sets the path to the ROM file to load", index = 1)]
    pub rom_name: String,
    #[arg(short = 'L', default_value = "default_library")]
    // Name of the core library to be loaded.
    pub library_name: String,
    #[arg(skip)]
    pub frame_buffer: Option<Vec<u32>>,
    #[arg(skip)]
    pub screen_pitch: u32,
    #[arg(skip)]
    pub screen_width: u32,
    #[arg(skip)]
    pub screen_height: u32,
    #[arg(skip)]
    pub current_save_slot: u8,
    #[arg(skip)]
    pub av_info: Option<SystemAvInfo>,
    #[arg(skip)]
    pub pixel_format: video::EmulatorPixelFormat,
    #[arg(skip)]
    pub bytes_per_pixel: u8,
}

// Parses command-line arguments to obtain the ROM name and core library name.
pub fn parse_command_line_arguments() -> (String, String) {
    let emulator_state = EmulatorState::parse();

    println!("ROM name: {}", emulator_state.rom_name);
    println!("Core Library name: {}", emulator_state.library_name);

    (emulator_state.rom_name, emulator_state.library_name)
}

// Loads the specified ROM file using the provided Core API.
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

// Callback function for the libretro environment.
unsafe extern "C" fn libretro_environment_callback(command: u32, return_data: *mut c_void) -> bool {
    match command {
        libretro_sys::ENVIRONMENT_GET_CAN_DUPE => {
            *(return_data as *mut bool) = true; // Set the return_data to the value true
            println!("ENVIRONMENT_GET_CAN_DUPE");
        }
        libretro_sys::ENVIRONMENT_SET_PIXEL_FORMAT => {
            let pixel_format = *(return_data as *const u32);
            let sender = &PIXEL_FORMAT_CHANNEL.0; // Use the global sender
            sender
                .send(PixelFormat::from_uint(pixel_format).unwrap())
                .expect("Failed to send pixel format");
            return true;
        }
        _ => println!(
            "libretro_environment_callback Called with command: {}",
            command
        ),
    }
    false
}

// Represents a loaded libretro core with associated functions.
pub struct Core {
    pub dylib: Library,
    pub api: CoreAPI,
}

impl Core {
    pub fn new(mut state: EmulatorState) -> (Self, EmulatorState) {
        unsafe {
            let dylib = Library::new(&state.library_name).expect("Failed to load Core");

            let core_api = CoreAPI {
                retro_set_environment: *(dylib.get(b"retro_set_environment").unwrap()),
                retro_set_video_refresh: *(dylib.get(b"retro_set_video_refresh").unwrap()),
                retro_set_audio_sample: *(dylib.get(b"retro_set_audio_sample").unwrap()),
                retro_set_audio_sample_batch: *(dylib
                    .get(b"retro_set_audio_sample_batch")
                    .unwrap()),
                retro_set_input_poll: *(dylib.get(b"retro_set_input_poll").unwrap()),
                retro_set_input_state: *(dylib.get(b"retro_set_input_state").unwrap()),

                retro_init: *(dylib.get(b"retro_init").unwrap()),
                retro_deinit: *(dylib.get(b"retro_deinit").unwrap()),

                retro_api_version: *(dylib.get(b"retro_api_version").unwrap()),

                retro_get_system_info: *(dylib.get(b"retro_get_system_info").unwrap()),
                retro_get_system_av_info: *(dylib.get(b"retro_get_system_av_info").unwrap()),
                retro_set_controller_port_device: *(dylib
                    .get(b"retro_set_controller_port_device")
                    .unwrap()),

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
            let mut av_info = SystemAvInfo {
                geometry: GameGeometry {
                    base_width: 0,
                    base_height: 0,
                    max_width: 0,
                    max_height: 0,
                    aspect_ratio: 0.0,
                },
                timing: SystemTiming {
                    fps: 0.0,
                    sample_rate: 0.0,
                },
            };
            (core_api.retro_get_system_av_info)(&mut av_info);
            println!("AV Info: {:?}", &av_info);
            state.av_info = Some(av_info);

            // Construct and return a Core instance
            (
                Core {
                    dylib,
                    api: core_api,
                },
                state,
            )
        }
    }
}

// Handles dropping of the Core, which could include cleanup tasks.
impl Drop for Core {
    fn drop(&mut self) {
        // Cleanup code here...
    }
}

// Utility functions for managing save states and configuration files follow.

// `get_save_state_path` computes the path for a save state file.
fn get_save_state_path(
    save_directory: &String,
    game_file_name: &str,
    save_state_index: &u8,
) -> Option<PathBuf> {
    // Expand the tilde to the home directory
    let expanded_save_directory = shellexpand::tilde(save_directory);

    // Create a subdirectory named "saves" in the specified directory
    let saves_dir = PathBuf::from(expanded_save_directory.into_owned());
    if !saves_dir.exists() {
        match std::fs::create_dir_all(&saves_dir) {
            Ok(_) => {}
            Err(err) => panic!(
                "Failed to create save directory: {:?} Error: {}",
                &saves_dir, err
            ),
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

// `save_state` saves the current state of the emulator to a file.
pub unsafe fn save_state(
    core_api: &CoreAPI,
    save_directory: &String,
    rom_name: &String,
    save_index: &u8,
) {
    let save_state_buffer_size = (core_api.retro_serialize_size)();
    let mut state_buffer: Vec<u8> = vec![0; save_state_buffer_size];
    // Call retro_serialize to create the save state
    (core_api.retro_serialize)(
        state_buffer.as_mut_ptr() as *mut c_void,
        save_state_buffer_size,
    );

    let file_path = get_save_state_path(save_directory, &rom_name, &save_index).unwrap();

    std::fs::write(&file_path, &state_buffer).unwrap();
    println!(
        "Save state saved to: {} with size: {}",
        file_path.display(),
        save_state_buffer_size
    );
}

// `load_state` loads the emulator state from a file.
pub unsafe fn load_state(
    core_api: &CoreAPI,
    save_directory: &String,
    rom_name: &String,
    save_index: &u8,
) {
    let file_path = get_save_state_path(save_directory, &rom_name, &save_index).unwrap();

    let mut state_buffer = Vec::new();
    match File::open(&file_path) {
        Ok(mut file) => {
            // Read the save state file into a buffer
            match file.read_to_end(&mut state_buffer) {
                Ok(_) => {
                    // Call retro_unserialize to apply the save state
                    let result = (core_api.retro_unserialize)(
                        state_buffer.as_mut_ptr() as *mut c_void,
                        state_buffer.len() as usize,
                    );
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

// `get_retroarch_config_path` finds the path to the RetroArch configuration.
fn get_retroarch_config_path() -> PathBuf {
    return match std::env::consts::OS {
        "windows" => PathBuf::from(env::var("APPDATA").ok().unwrap()).join("retroarch"),
        "macos" => PathBuf::from(env::var("HOME").ok().unwrap())
            .join("Library/Application Support/RetroArch"),
        _ => PathBuf::from(env::var("XDG_CONFIG_HOME").ok().unwrap()).join("retroarch"),
    };
}

// `parse_retroarch_config` parses the RetroArch configuration file.
fn parse_retroarch_config(config_file: &Path) -> Result<HashMap<String, String>, String> {
    let file = File::open(config_file).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = BufReader::new(file);
    let mut config_map = HashMap::new();
    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        if let Some((key, value)) = line.split_once("=") {
            config_map.insert(
                key.trim().to_string(),
                value.trim().replace("\"", "").to_string(),
            );
        }
    }
    Ok(config_map)
}

// `setup_config` merges various configuration sources into a single HashMap.
pub fn setup_config() -> Result<HashMap<String, String>, String> {
    let retro_arch_config_path = get_retroarch_config_path();
    let our_config = parse_retroarch_config(Path::new("./rustroarch.cfg"));
    let retro_arch_config =
        parse_retroarch_config(&retro_arch_config_path.join("config/retroarch.cfg"));
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
    ])
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    match retro_arch_config {
        Ok(config) => merged_config.extend(config),
        _ => println!("We don't have RetroArch config"),
    }
    match our_config {
        Ok(config) => merged_config.extend(config),
        _ => println!("We don't have RustroArch config",),
    }
    // println!("retro_arch_config_path: {} merged_config: {:?}", retro_arch_config_path.join("config/retroarch.cfg").display(), merged_config);
    Ok(merged_config)
}
