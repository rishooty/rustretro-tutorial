use gilrs::Button;
use std::collections::HashMap;

pub fn setup_joypad_device_map() -> HashMap<Button, usize> {
    return HashMap::from([
        (
            Button::South,
            libretro_sys::DEVICE_ID_JOYPAD_A as usize,
        ),
        (
            Button::East,
            libretro_sys::DEVICE_ID_JOYPAD_B as usize,
        ),
        (
            Button::West,
            libretro_sys::DEVICE_ID_JOYPAD_X as usize,
        ),
        (
            Button::North,
            libretro_sys::DEVICE_ID_JOYPAD_Y as usize,
        ),
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
        (
            Button::DPadUp,
            libretro_sys::DEVICE_ID_JOYPAD_UP as usize,
        ),
        (
            Button::DPadRight,
            libretro_sys::DEVICE_ID_JOYPAD_RIGHT as usize,
        ),
        (
            Button::DPadLeft,
            libretro_sys::DEVICE_ID_JOYPAD_LEFT as usize,
        ),
        (
            Button::Start,
            libretro_sys::DEVICE_ID_JOYPAD_START as usize,
        ),
        (
            Button::Select,
            libretro_sys::DEVICE_ID_JOYPAD_SELECT as usize,
        ),
    ]);
}