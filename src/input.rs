// This implementation is based on the guide provided by [RetroGameDeveloper/RetroReversing].
// Original guide can be found at [https://www.retroreversing.com/CreateALibRetroFrontEndInRust].
// Copyright (c) 2023 Nicholas Ricciuti

use gilrs::{Button, GamepadId, Gilrs};
use libretro_sys::CoreAPI;
use minifb::{KeyRepeat, Window};
use std::collections::HashMap;

use crate::{
    libretro::{self, EmulatorState},
    BUTTONS_PRESSED,
};

pub const BUTTON_ARRAY: [Button; 14] = [
    Button::South,
    Button::North,
    Button::East,
    Button::West,
    Button::Start,
    Button::Select,
    Button::DPadDown,
    Button::DPadUp,
    Button::DPadLeft,
    Button::DPadRight,
    Button::LeftTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger,
    Button::RightTrigger2,
];

pub fn key_device_map(config: &HashMap<String, String>) -> HashMap<String, usize> {
    HashMap::from([
        (
            config["input_player1_a"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_A as usize,
        ),
        (
            config["input_player1_b"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_B as usize,
        ),
        (
            config["input_player1_x"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_X as usize,
        ),
        (
            config["input_player1_y"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_Y as usize,
        ),
        (
            config["input_player1_l"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_L as usize,
        ),
        (
            config["input_player1_r"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_R as usize,
        ),
        (
            config["input_player1_down"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_DOWN as usize,
        ),
        (
            config["input_player1_up"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_UP as usize,
        ),
        (
            config["input_player1_right"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_RIGHT as usize,
        ),
        (
            config["input_player1_left"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_LEFT as usize,
        ),
        (
            config["input_player1_start"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_START as usize,
        ),
        (
            config["input_player1_select"].clone(),
            libretro_sys::DEVICE_ID_JOYPAD_SELECT as usize,
        ),
    ])
}

pub unsafe extern "C" fn libretro_set_input_poll_callback() {
    println!("libretro_set_input_poll_callback")
}

pub unsafe extern "C" fn libretro_set_input_state_callback(
    port: libc::c_uint,
    device: libc::c_uint,
    index: libc::c_uint,
    id: libc::c_uint,
) -> i16 {
    let buttons = BUTTONS_PRESSED.lock().unwrap();
    buttons.0.get(id as usize).copied().unwrap_or(0)
}

pub fn setup_joypad_device_map() -> HashMap<Button, usize> {
    return HashMap::from([
        (Button::South, libretro_sys::DEVICE_ID_JOYPAD_A as usize),
        (Button::East, libretro_sys::DEVICE_ID_JOYPAD_B as usize),
        (Button::West, libretro_sys::DEVICE_ID_JOYPAD_X as usize),
        (Button::North, libretro_sys::DEVICE_ID_JOYPAD_Y as usize),
        (
            Button::LeftTrigger,
            libretro_sys::DEVICE_ID_JOYPAD_L as usize,
        ),
        (
            Button::LeftTrigger2,
            libretro_sys::DEVICE_ID_JOYPAD_L2 as usize,
        ),
        (
            Button::RightTrigger,
            libretro_sys::DEVICE_ID_JOYPAD_R as usize,
        ),
        (
            Button::RightTrigger2,
            libretro_sys::DEVICE_ID_JOYPAD_R2 as usize,
        ),
        (
            Button::DPadDown,
            libretro_sys::DEVICE_ID_JOYPAD_DOWN as usize,
        ),
        (Button::DPadUp, libretro_sys::DEVICE_ID_JOYPAD_UP as usize),
        (
            Button::DPadRight,
            libretro_sys::DEVICE_ID_JOYPAD_RIGHT as usize,
        ),
        (
            Button::DPadLeft,
            libretro_sys::DEVICE_ID_JOYPAD_LEFT as usize,
        ),
        (Button::Start, libretro_sys::DEVICE_ID_JOYPAD_START as usize),
        (
            Button::Select,
            libretro_sys::DEVICE_ID_JOYPAD_SELECT as usize,
        ),
    ]);
}

pub fn handle_gamepad_input(
    joypad_device_map: &HashMap<Button, usize>,
    gilrs: &Gilrs,
    active_gamepad: &Option<GamepadId>,
    buttons_pressed: &mut Vec<i16>,
) {
    if let Some(gamepad) = active_gamepad.map(|id| gilrs.gamepad(id)) {
        for button in BUTTON_ARRAY {
            if let Some(&libretro_button) = joypad_device_map.get(&button) {
                buttons_pressed[libretro_button as usize] = gamepad.is_pressed(button) as i16;
            }
        }
    }
}

pub fn handle_keyboard_input(
    core_api: &CoreAPI,
    window: &Window,
    current_state: &mut EmulatorState,
    buttons_pressed: &mut Vec<i16>,
    key_device_map: &HashMap<String, usize>,
    config: &HashMap<String, String>,
    game_pad_active: bool,
) {
    let mini_fb_keys_pressed = window.get_keys_pressed(KeyRepeat::No);
    for key in mini_fb_keys_pressed {
        let key_as_string = format!("{:?}", key).to_ascii_lowercase();

        if !game_pad_active {
            if let Some(&device_id) = key_device_map.get(&key_as_string) {
                buttons_pressed[device_id as usize] = 1;
            }
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

    if !game_pad_active {
        let mini_fb_keys_released = window.get_keys_released();
        for key in &mini_fb_keys_released {
            let key_as_string = format!("{:?}", key).to_ascii_lowercase();

            if let Some(&device_id) = key_device_map.get(&key_as_string) {
                buttons_pressed[device_id as usize] = 0;
            } else {
                println!(
                    "Unhandled Key Pressed: {} input_player1_a: {}",
                    key_as_string, config["input_player1_a"]
                );
            }
        }
    }
}
