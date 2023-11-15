use once_cell::sync::Lazy;
use rodio::buffer::SamplesBuffer;
use rodio::Sink;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use crate::CURRENT_STATE;

const AUDIO_CHANNELS: usize = 2; // left and right
const SAMPLE_RATE: u32 = 44_100; // 44.1 kHz
const BUFFER_DURATION_MS: u32 = 16; // Roughly 1/60th of a second
const BUFFER_LENGTH: usize = (SAMPLE_RATE as u32 * BUFFER_DURATION_MS / 1000) as usize; // Number of samples per buffer
const POOL_SIZE: usize = 10; // Number of buffers in the pool

static BUFFER_POOL: Lazy<Mutex<Vec<Arc<Mutex<Vec<i16>>>>>> = Lazy::new(|| {
    let mut pool = Vec::new();
    for _ in 0..POOL_SIZE {
        pool.push(Arc::new(Mutex::new(vec![0; BUFFER_LENGTH])));
    }
    Mutex::new(pool)
});

pub unsafe fn play_audio(sink: &Sink, audio_samples: &Vec<i16>, sample_rate: u32) {
    if sink.empty() {
        let audio_slice =
            std::slice::from_raw_parts(audio_samples.as_ptr() as *const i16, audio_samples.len());
        let source = SamplesBuffer::new(2, sample_rate, audio_slice);
        sink.append(source);
        sink.play();
        sink.sleep_until_end();
    }
}

pub unsafe extern "C" fn libretro_set_audio_sample_callback(left: i16, right: i16) {
    println!("libretro_set_audio_sample_callback");
}

pub unsafe extern "C" fn libretro_set_audio_sample_batch_callback(
    audio_data: *const i16,
    frames: libc::size_t,
) -> libc::size_t {
    let mut pool = BUFFER_POOL.lock().unwrap();
    let buffer_arc = pool
        .pop()
        .unwrap_or_else(|| Arc::new(Mutex::new(vec![0; BUFFER_LENGTH])));

    {
        let mut buffer = buffer_arc.lock().unwrap();
        let audio_slice = std::slice::from_raw_parts(audio_data, frames * AUDIO_CHANNELS);
        buffer.clear();
        buffer.extend_from_slice(audio_slice);
    }

    {
        let mut state = CURRENT_STATE.lock().unwrap();
        // Clone buffer_arc before assigning it
        state.audio_data = Some(buffer_arc.clone());
    }
    // Now it's safe to push the original buffer_arc back into the pool
    pool.push(buffer_arc);

    frames
}

pub fn send_audio_to_thread(sender: &Sender<Arc<Mutex<Vec<i16>>>>) {
    let buffer_arc_clone = {
        let state = CURRENT_STATE.lock().unwrap();
        state.audio_data.clone() // Clone the Arc itself, not the data
    };

    if let Some(buffer_arc) = buffer_arc_clone {
        sender.send(buffer_arc).unwrap();
    }
}
