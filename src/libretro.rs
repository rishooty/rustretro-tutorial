use std::{path::{PathBuf, Path}, fs::File, io::{Read, BufReader, BufRead}, env, collections::HashMap};
use libc::c_void;
use libretro_sys::CoreAPI;

use crate::CURRENT_EMULATOR_STATE;

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

pub unsafe fn save_state(core_api: &CoreAPI, save_directory: &String) {
    let save_state_buffer_size =  (core_api.retro_serialize_size)();
    let mut state_buffer: Vec<u8> = vec![0; save_state_buffer_size];
    // Call retro_serialize to create the save state
    (core_api.retro_serialize)(state_buffer.as_mut_ptr() as *mut c_void, save_state_buffer_size);
    let file_path = get_save_state_path(save_directory, &CURRENT_EMULATOR_STATE.rom_name, &CURRENT_EMULATOR_STATE.current_save_slot).unwrap(); // hard coded save_slot to 0 for now
    std::fs::write(&file_path, &state_buffer).unwrap();
    println!("Save state saved to: {} with size: {}", file_path.display(), save_state_buffer_size);
}

pub unsafe fn load_state(core_api: &CoreAPI, save_directory: &String) {
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

pub fn setup_config() -> Result<HashMap<String, String>, String> {
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