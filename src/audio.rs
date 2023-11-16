use once_cell::sync::Lazy;
use rodio::buffer::SamplesBuffer;
use rodio::Sink;
use std::sync::{Arc, Mutex};
use crate::AUDIO_DATA_CHANNEL;

const AUDIO_CHANNELS: usize = 2; // left and right
const SAMPLE_RATE: u32 = 48_000; // 48 kHz
const BUFFER_DURATION_MS: u32 = 64; // Roughly 1/64 of a second
const BUFFER_LENGTH: usize = (SAMPLE_RATE as u32 * BUFFER_DURATION_MS / 1000) as usize;
const POOL_SIZE: usize = 20; // Number of buffers in the pool

pub struct AudioBuffer {
    data: Vec<i16>,
}

impl AudioBuffer {
    // Constructor to create a new AudioBuffer with a specified size
    pub fn new(size: usize) -> Self {
        AudioBuffer {
            data: vec![0; size],
        }
    }

    // Method to clear the buffer
    pub fn clear(&mut self) {
        self.data.clear();
    }

    // Method to extend the buffer from a slice
    pub fn extend_from_slice(&mut self, slice: &[i16]) {
        self.data.extend_from_slice(slice);
    }

    pub fn as_ptr(&self) -> *const i16 {
        self.data.as_ptr()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    // Additional methods as needed...
    // For example, methods to manipulate the audio data, etc.
}


static BUFFER_POOL: Lazy<Mutex<Vec<Arc<Mutex<Vec<i16>>>>>> = Lazy::new(|| {
    let mut pool = Vec::new();
    for _ in 0..POOL_SIZE {
        pool.push(Arc::new(Mutex::new(vec![0; BUFFER_LENGTH])));
    }
    Mutex::new(pool)
});

pub unsafe fn play_audio(sink: &Sink, audio_samples: &AudioBuffer, sample_rate: u32) {
    let audio_slice =
        std::slice::from_raw_parts(audio_samples.as_ptr() as *const i16, audio_samples.len());
    let source = SamplesBuffer::new(2, sample_rate, audio_slice);
    sink.append(source);
}

pub unsafe extern "C" fn libretro_set_audio_sample_callback(left: i16, right: i16) {
    println!("libretro_set_audio_sample_callback");
}

pub unsafe extern "C" fn libretro_set_audio_sample_batch_callback(
    audio_data: *const i16,
    frames: libc::size_t,
) -> libc::size_t {
    let buffer_arc: Arc<Mutex<Vec<i16>>>;
    {
        let mut pool = BUFFER_POOL.lock().unwrap();
        buffer_arc = pool
            .pop()
            .unwrap_or_else(|| Arc::new(Mutex::new(vec![0; BUFFER_LENGTH])));
    }

    {
        let mut buffer = AudioBuffer::new(BUFFER_LENGTH);
        let audio_slice = std::slice::from_raw_parts(audio_data, frames * AUDIO_CHANNELS);
        buffer.clear();
        buffer.extend_from_slice(audio_slice);
        let buffer_arc = Arc::new(Mutex::new(buffer));
        if let Err(e) = AUDIO_DATA_CHANNEL.0.send(buffer_arc.clone()) {
            eprintln!("Failed to send audio data: {:?}", e);
        }
    }


    // Now it's safe to push the original buffer_arc back into the pool
    {
        let mut pool = BUFFER_POOL.lock().unwrap();
        pool.push(buffer_arc);
    }

    frames
}