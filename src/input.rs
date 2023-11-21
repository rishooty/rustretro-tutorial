use gilrs::{Button, GamepadId, Gilrs};
use libretro_sys::CoreAPI;
use minifb::{KeyRepeat, Window};
use std::collections::{hash_map::RandomState, HashMap};

use crate::{
    libretro::{EmulatorState, self},
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

pub fn key_device_map(config: &HashMap<String, String>) -> HashMap<&String, usize> {
    HashMap::from([
        (
            &config["input_player1_a"],
            libretro_sys::DEVICE_ID_JOYPAD_A as usize,
        ),
        (
            &config["input_player1_b"],
            libretro_sys::DEVICE_ID_JOYPAD_B as usize,
        ),
        (
            &config["input_player1_x"],
            libretro_sys::DEVICE_ID_JOYPAD_X as usize,
        ),
        (
            &config["input_player1_y"],
            libretro_sys::DEVICE_ID_JOYPAD_Y as usize,
        ),
        (
            &config["input_player1_l"],
            libretro_sys::DEVICE_ID_JOYPAD_L as usize,
        ),
        (
            &config["input_player1_r"],
            libretro_sys::DEVICE_ID_JOYPAD_R as usize,
        ),
        (
            &config["input_player1_down"],
            libretro_sys::DEVICE_ID_JOYPAD_DOWN as usize,
        ),
        (
            &config["input_player1_up"],
            libretro_sys::DEVICE_ID_JOYPAD_UP as usize,
        ),
        (
            &config["input_player1_right"],
            libretro_sys::DEVICE_ID_JOYPAD_RIGHT as usize,
        ),
        (
            &config["input_player1_left"],
            libretro_sys::DEVICE_ID_JOYPAD_LEFT as usize,
        ),
        (
            &config["input_player1_start"],
            libretro_sys::DEVICE_ID_JOYPAD_START as usize,
        ),
        (
            &config["input_player1_select"],
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
    mut this_frames_pressed_buttons: Vec<i16>,
) -> Vec<i16> {
    if let Some(gamepad) = active_gamepad.map(|id| gilrs.gamepad(id)) {
        for button in BUTTON_ARRAY {
            let libretro_button = joypad_device_map.get(&button).unwrap();
            if gamepad.is_pressed(button) {
                println!("Button Pressed: {:?}", button);
                this_frames_pressed_buttons[*libretro_button] = 1;
            } else {
                this_frames_pressed_buttons[*libretro_button] = 0;
            }
        }
    }

    return this_frames_pressed_buttons;
}

pub fn handle_keyboard_input(
    core_api: &CoreAPI,
    window: &Window,
    mut current_state: EmulatorState,
    mut this_frames_pressed_buttons: Vec<i16>,
    key_device_map: &HashMap<&String, usize>,
    config: &HashMap<String, String, RandomState>,
) -> (EmulatorState, Vec<i16>) {
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

    return (current_state, this_frames_pressed_buttons);
}
