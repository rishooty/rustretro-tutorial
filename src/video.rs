use libretro_sys::PixelFormat;
use std::{ptr, sync::atomic::Ordering};

use crate::{VIDEO_DATA_SENDER, VideoData, BYTES_PER_PIXEL};

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

    VIDEO_DATA_SENDER.send(video_data).expect("Failed to send video data");
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
