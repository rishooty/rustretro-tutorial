// This implementation is based on the guide provided by [RetroGameDeveloper/RetroReversing].
// Original guide can be found at [https://www.retroreversing.com/CreateALibRetroFrontEndInRust].
// Copyright (c) 2023 Nicholas Ricciuti

use gilrs::{Button, GamepadId, Gilrs};
use libretro_sys::{
    CoreAPI, DEVICE_ID_JOYPAD_A, DEVICE_ID_JOYPAD_B, DEVICE_ID_JOYPAD_DOWN, DEVICE_ID_JOYPAD_L,
    DEVICE_ID_JOYPAD_LEFT, DEVICE_ID_JOYPAD_R, DEVICE_ID_JOYPAD_RIGHT, DEVICE_ID_JOYPAD_SELECT,
    DEVICE_ID_JOYPAD_START, DEVICE_ID_JOYPAD_UP, DEVICE_ID_JOYPAD_X, DEVICE_ID_JOYPAD_Y,
};
use minifb::{KeyRepeat, Window};
use std::collections::HashMap;

use crate::{
    libretro::{self, EmulatorState},
    BUTTONS_PRESSED,
};

pub fn key_device_map(config: &HashMap<String, String>) -> HashMap<String, usize> {
    HashMap::from([
        (
            config["input_player1_a"].clone(),
            DEVICE_ID_JOYPAD_A as usize,
        ),
        (
            config["input_player1_b"].clone(),
            DEVICE_ID_JOYPAD_B as usize,
        ),
        (
            config["input_player1_x"].clone(),
            DEVICE_ID_JOYPAD_X as usize,
        ),
        (
            config["input_player1_y"].clone(),
            DEVICE_ID_JOYPAD_Y as usize,
        ),
        (
            config["input_player1_l"].clone(),
            DEVICE_ID_JOYPAD_L as usize,
        ),
        (
            config["input_player1_r"].clone(),
            DEVICE_ID_JOYPAD_R as usize,
        ),
        (
            config["input_player1_down"].clone(),
            DEVICE_ID_JOYPAD_DOWN as usize,
        ),
        (
            config["input_player1_up"].clone(),
            DEVICE_ID_JOYPAD_UP as usize,
        ),
        (
            config["input_player1_right"].clone(),
            DEVICE_ID_JOYPAD_RIGHT as usize,
        ),
        (
            config["input_player1_left"].clone(),
            DEVICE_ID_JOYPAD_LEFT as usize,
        ),
        (
            config["input_player1_start"].clone(),
            DEVICE_ID_JOYPAD_START as usize,
        ),
        (
            config["input_player1_select"].clone(),
            DEVICE_ID_JOYPAD_SELECT as usize,
        ),
    ])
}

pub fn setup_joypad_device_map(config: &HashMap<String, String>) -> HashMap<String, usize> {
    HashMap::from([
        (
            config
                .get("input_player1_a_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_A.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_A as usize,
        ),
        (
            config
                .get("input_player1_b_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_B.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_B as usize,
        ),
        (
            config
                .get("input_player1_x_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_X.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_X as usize,
        ),
        (
            config
                .get("input_player1_y_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_Y.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_Y as usize,
        ),
        (
            config
                .get("input_player1_l_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_L.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_L as usize,
        ),
        (
            config
                .get("input_player1_r_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_R.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_R as usize,
        ),
        (
            config
                .get("input_player1_down_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_DOWN.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_DOWN as usize,
        ),
        (
            config
                .get("input_player1_up_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_UP.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_UP as usize,
        ),
        (
            config
                .get("input_player1_right_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_RIGHT.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_RIGHT as usize,
        ),
        (
            config
                .get("input_player1_left_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_LEFT.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_LEFT as usize,
        ),
        (
            config
                .get("input_player1_start_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_START.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_START as usize,
        ),
        (
            config
                .get("input_player1_select_btn")
                .unwrap_or(&DEVICE_ID_JOYPAD_SELECT.to_string())
                .clone(),
            DEVICE_ID_JOYPAD_SELECT as usize,
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

fn libretro_to_button(libretro_button: u32) -> Option<Button> {
    match libretro_button {
        DEVICE_ID_JOYPAD_A => Some(Button::East),
        DEVICE_ID_JOYPAD_B => Some(Button::South),
        DEVICE_ID_JOYPAD_X => Some(Button::North),
        DEVICE_ID_JOYPAD_Y => Some(Button::West),
        DEVICE_ID_JOYPAD_L => Some(Button::LeftTrigger),
        DEVICE_ID_JOYPAD_R => Some(Button::RightTrigger),
        DEVICE_ID_JOYPAD_DOWN => Some(Button::DPadDown),
        DEVICE_ID_JOYPAD_UP => Some(Button::DPadUp),
        DEVICE_ID_JOYPAD_RIGHT => Some(Button::DPadRight),
        DEVICE_ID_JOYPAD_LEFT => Some(Button::DPadLeft),
        DEVICE_ID_JOYPAD_START => Some(Button::Start),
        DEVICE_ID_JOYPAD_SELECT => Some(Button::Select),
        _ => None,
    }
}

pub fn handle_gamepad_input(
    joypad_device_map: &HashMap<String, usize>,
    gilrs: &Gilrs,
    active_gamepad: &Option<GamepadId>,
    buttons_pressed: &mut Vec<i16>,
) {
    if let Some(gamepad) = active_gamepad.map(|id| gilrs.gamepad(id)) {
        for (button, libretro_button) in joypad_device_map {
            if let Some(gilrs_button) = libretro_to_button(*libretro_button as u32) {
                buttons_pressed[*libretro_button as usize] =
                    gamepad.is_pressed(gilrs_button) as i16;
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
