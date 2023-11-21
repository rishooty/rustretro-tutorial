use libretro_sys::PixelFormat;
use minifb::Window;
use std::sync::atomic::Ordering;

use crate::{
    libretro::EmulatorState, VideoData, BYTES_PER_PIXEL, PIXEL_FORMAT_CHANNEL, VIDEO_DATA_CHANNEL,
};

pub struct EmulatorPixelFormat(pub PixelFormat);

impl Default for EmulatorPixelFormat {
    fn default() -> Self {
        EmulatorPixelFormat(PixelFormat::ARGB8888)
    }
}

pub unsafe extern "C" fn libretro_set_video_refresh_callback(
    frame_buffer_data: *const libc::c_void,
    width: libc::c_uint,
    height: libc::c_uint,
    pitch: libc::size_t,
) {
    if frame_buffer_data.is_null() {
        println!("frame_buffer_data was null");
        return;
    }
    let bpp = BYTES_PER_PIXEL.load(Ordering::SeqCst) as u32;
    let length_of_frame_buffer = ((pitch as u32) * height) * bpp;

    let buffer_slice = std::slice::from_raw_parts(
        frame_buffer_data as *const u8,
        length_of_frame_buffer as usize,
    );
    let result = convert_pixel_array_from_rgb565_to_xrgb8888(buffer_slice);

    let video_data = VideoData {
        frame_buffer: Vec::from(result),
        width: width as u32,
        height: height as u32,
        pitch: pitch as u32,
    };

    if let Err(e) = VIDEO_DATA_CHANNEL.0.send(video_data) {
        eprintln!("Failed to send video data: {:?}", e);
        // Handle error appropriately
    }
}

pub fn set_up_pixel_format(mut current_state: EmulatorState) -> EmulatorState {
    let pixel_format_receiver = &PIXEL_FORMAT_CHANNEL.1.lock().unwrap();

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

    return current_state;
}

fn convert_pixel_array_from_rgb565_to_xrgb8888(color_array: &[u8]) -> Box<[u32]> {
    let bytes_per_pixel = 2;
    assert_eq!(
        color_array.len() % bytes_per_pixel,
        0,
        "color_array length must be a multiple of 2 (16-bits per pixel)"
    );

    let num_pixels = color_array.len() / bytes_per_pixel;
    let mut result = vec![0u32; num_pixels];

    for i in 0..num_pixels {
        // This Rust code is decoding a 16-bit color value, represented by two bytes of data, into its corresponding red, green, and blue components.
        let first_byte = color_array[bytes_per_pixel * i];
        let second_byte = color_array[(bytes_per_pixel * i) + 1];

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

pub fn render_frame(
    mut current_state: EmulatorState,
    mut window: Window,
) -> (EmulatorState, Window) {
    let video_data_receiver = VIDEO_DATA_CHANNEL.1.lock().unwrap();
    for video_data in video_data_receiver.try_iter() {
        current_state.frame_buffer = Some(video_data.frame_buffer);
        current_state.screen_height = video_data.height;
        current_state.screen_width = video_data.width;
        current_state.screen_pitch = video_data.pitch;
    }

    match &current_state.frame_buffer {
        Some(buffer) => {
            let width =
                (current_state.screen_pitch / current_state.bytes_per_pixel as u32) as usize;
            let height = current_state.screen_height as usize;
            let slice_of_pixel_buffer: &[u32];
            unsafe {
                slice_of_pixel_buffer =
                    std::slice::from_raw_parts(buffer.as_ptr() as *const u32, buffer.len());
                // convert to &[u32] slice reference
            }

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
    return (current_state, window);
}
