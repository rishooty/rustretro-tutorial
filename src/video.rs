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

pub fn render_frame(current_state: EmulatorState, mut window: Window) -> (EmulatorState, Window) {
    let video_data_receiver = VIDEO_DATA_CHANNEL.1.lock().unwrap();
    for video_data in video_data_receiver.try_iter() {
        let source_width = video_data.width as usize;
        let source_height = video_data.height as usize;
        let pitch = video_data.pitch as usize; // number of bytes per row

        let window_size = window.get_size();
        let scale_x = window_size.0 / source_width;
        let scale_y = window_size.1 / source_height;
        let scale = scale_y.min(scale_x); // maintain aspect ratio

        let target_width = source_width * scale;
        let target_height = source_height * scale;

        // Calculate padding for centering the image
        let bpp = BYTES_PER_PIXEL.load(Ordering::SeqCst) as usize;
        let padding_x = (window_size.0 - target_width) / bpp;
        let padding_y = (window_size.1 - target_height) / bpp;

        // Prepare the buffer that will be sent to the window
        let mut window_buffer = vec![0; window_size.0 * window_size.1];
        for y in 0..source_height {
            let source_start = y * pitch / bpp; // divide by 2 because the pitch is based on 2 bytes per pixel
            let dest_start = (y * scale + padding_y) * window_size.0 + padding_x;

            // Copy each row, taking into account the pitch and scaling
            for x in 0..source_width {
                let dest_index = dest_start + x * scale;
                let source_index = source_start + x;

                // Copy the pixel `scale` times in both X and Y dimensions
                for dx in 0..scale {
                    for dy in 0..scale {
                        let window_index = (dest_index + dy * window_size.0 + dx) as usize;
                        let source_pixel = video_data
                            .frame_buffer
                            .get(source_index)
                            .copied()
                            .unwrap_or(0);
                        window_buffer[window_index] = source_pixel;
                    }
                }
            }
        }

        // Update the window
        window
            .update_with_buffer(&window_buffer, window_size.0, window_size.1)
            .unwrap();
    }

    return (current_state, window);
}
