use minifb::{Key, Window, WindowOptions};
use std::fs;
use std::time::{Duration, Instant};
use std::ffi::{c_void, CString};
use std::ptr;
use libloading::Library;
use libretro_sys::CoreAPI;
use libretro_sys::GameInfo;
use clap::Parser;

unsafe extern "C" fn libretro_set_video_refresh_callback(frame_buffer_data: *const libc::c_void, width: libc::c_uint, height: libc::c_uint, pitch: libc::size_t) {
    if frame_buffer_data == ptr::null() {
        println!("frame_buffer_data was null");
        return;
    }
    println!("libretro_set_video_refresh_callback, width: {}, height: {}, pitch: {}", width, height, pitch);
    let length_of_frame_buffer = width*height;
    let buffer_slice = std::slice::from_raw_parts(frame_buffer_data as *const u8, length_of_frame_buffer as usize);
    
    let buffer_vec = Vec::from(buffer_slice);
    CURRENT_EMULATOR_STATE.frame_buffer = Some(buffer_vec);
    println!("Frame Buffer: {:?}", CURRENT_EMULATOR_STATE.frame_buffer)
}

unsafe extern "C" fn libretro_set_input_poll_callback() {
    println!("libretro_set_input_poll_callback")
}

unsafe extern "C" fn libretro_set_input_state_callback(port: libc::c_uint, device: libc::c_uint, index: libc::c_uint, id: libc::c_uint) -> i16 {
    println!("libretro_set_input_state_callback");
    return 0; // Hard coded 0 for now means nothing is pressed
}

unsafe extern "C" fn libretro_set_audio_sample_callback(left: i16, right: i16) {
    println!("libretro_set_audio_sample_callback");
}

unsafe extern "C" fn libretro_set_audio_sample_batch_callback(data: *const i16, frames: libc::size_t) -> libc::size_t {
    println!("libretro_set_audio_sample_batch_callback");
    return 1;
}

#[derive(Parser)]
struct EmulatorState {
    #[arg(help = "Sets the path to the ROM file to load", index = 2)]
    rom_name: String,
    #[arg(short = 'L', default_value = "default_library")]
    library_name: String,
    frame_buffer: Option<Vec<u8>>,
}

static mut CURRENT_EMULATOR_STATE: EmulatorState = EmulatorState {
    rom_name: String::new(),
    library_name: String::new(),
    frame_buffer: None,
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
            println!("TODO: Handle ENVIRONMENT_SET_PIXEL_FORMAT when we start drawing the the screen buffer");
            return true;
        }
        _ => println!("libretro_environment_callback Called with command: {}", command)
    }
    false
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

const WIDTH: usize = 640;
const HEIGHT: usize = 480;

fn main() {
    unsafe { CURRENT_EMULATOR_STATE = parse_command_line_arguments()};

    let mut buffer: Vec<u32> = vec![0; WIDTH * HEIGHT];
    let mut window = Window::new("Rust Game", WIDTH, HEIGHT, WindowOptions::default())
        .unwrap_or_else(|e| {
            panic!("{}", e);
        });

    window.limit_update_rate(Some(std::time::Duration::from_micros(16600))); // ~60fps

    let mut x: usize = 0;
    let mut y: usize = 0;

    let mut fps_timer = Instant::now();
    let mut fps_counter: i32 = 0;

    let core = Core::new(unsafe { &CURRENT_EMULATOR_STATE.library_name });
    let core_api = &core.api;

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

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Call the libRetro core every frame
        unsafe {
            (core_api.retro_run)();
        }

        // Clear the previous pixel to black
        buffer[y * WIDTH + x] = 0x00000000;
        fps_counter += 1;
        let elapsed = fps_timer.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let fps = fps_counter as f64 / elapsed.as_secs_f64();
            window.set_title(&format!("Rust Game (FPS: {:.2})", fps));
            fps_counter = 0;
            fps_timer = Instant::now();
        }

        // Move the pixel when the arrow keys are pressed
        if window.is_key_down(Key::Left) && x > 0 {
            x -= 1;
        }
        if window.is_key_down(Key::Right) && x < WIDTH - 1 {
            x += 1;
        }
        if window.is_key_down(Key::Up) && y > 0 {
            y -= 1;
        }
        if window.is_key_down(Key::Down) && y < HEIGHT - 1 {
            y += 1;
        }

        // Set the pixel to blue
        buffer[y * WIDTH + x] = 0x0000FFFF;

       unsafe {
        match &CURRENT_EMULATOR_STATE.frame_buffer {
            Some(buffer) => {
                let slice_u32: &[u32] = unsafe {
                    std::slice::from_raw_parts(buffer.as_ptr() as *const u32, buffer.len())
                };
                // temporary hack
                let mut vec: Vec<u32> = slice_u32.to_vec();
                vec.resize(WIDTH*HEIGHT*4, 0x0000FFFF);
                window.update_with_buffer(&vec, WIDTH, HEIGHT).unwrap();
            }
            None => {
                // Handle the case where frame_buffer is None
                println!("We don't have a buffer to display");
            }
        }
       }
    }
}
