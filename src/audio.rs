// This implementation is based on the guide provided by [RetroGameDeveloper/RetroReversing].
// Original guide can be found at [https://www.retroreversing.com/CreateALibRetroFrontEndInRust].
// Copyright (c) 2023 Nicholas Ricciuti
//
// The `audio` module handles audio processing and playback for the emulator.
// It uses the `rodio` crate for audio output and integrates with the libretro API for audio data.

use once_cell::sync::Lazy;
use rodio::buffer::SamplesBuffer;
use rodio::Sink;
use std::sync::{Arc, Mutex};
use crate::AUDIO_DATA_CHANNEL;

// Constants for audio processing.
const AUDIO_CHANNELS: usize = 2; // Stereo audio with left and right channels.
const SAMPLE_RATE: u32 = 48_000; // Sample rate in Hertz (48 kHz).
const BUFFER_DURATION_MS: u32 = 64; // Duration of each audio buffer in milliseconds.
const BUFFER_LENGTH: usize = (SAMPLE_RATE as u32 * BUFFER_DURATION_MS / 1000) as usize; // Number of samples in each buffer.
const POOL_SIZE: usize = 20; // Number of buffers in the audio buffer pool.

// Represents an audio buffer containing raw audio samples.
pub struct AudioBuffer {
    data: Vec<i16>, // Vector to store the 16-bit audio samples.
}

impl AudioBuffer {
    // Constructs a new `AudioBuffer` with a specified size.
    pub fn new(size: usize) -> Self {
        AudioBuffer { data: vec![0; size] }
    }

    // Clears the buffer, removing all audio samples.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    // Extends the buffer with audio samples from a slice.
    pub fn extend_from_slice(&mut self, slice: &[i16]) {
        self.data.extend_from_slice(slice);
    }

    // Returns a pointer to the audio data.
    pub fn as_ptr(&self) -> *const i16 {
        self.data.as_ptr()
    }

    // Returns the length of the audio data in samples.
    pub fn len(&self) -> usize {
        self.data.len()
    }
}

// Global buffer pool for managing audio buffers.
static BUFFER_POOL: Lazy<Mutex<Vec<Arc<Mutex<Vec<i16>>>>>> = Lazy::new(|| {
    let mut pool = Vec::new();
    for _ in 0..POOL_SIZE {
        pool.push(Arc::new(Mutex::new(vec![0; BUFFER_LENGTH])));
    }
    Mutex::new(pool)
});

// Plays audio using the `rodio` library.
pub unsafe fn play_audio(sink: &Sink, audio_samples: &AudioBuffer, sample_rate: u32) {
    let audio_slice = std::slice::from_raw_parts(audio_samples.as_ptr() as *const i16, audio_samples.len());
    let source = SamplesBuffer::new(AUDIO_CHANNELS.try_into().unwrap(), sample_rate, audio_slice);
    sink.append(source);
}

// Callback function for the libretro API to handle individual audio samples.
pub unsafe extern "C" fn libretro_set_audio_sample_callback(left: i16, right: i16) {
    println!("libretro_set_audio_sample_callback");
}

// Callback function for the libretro API to handle batches of audio samples.
pub unsafe extern "C" fn libretro_set_audio_sample_batch_callback(
    audio_data: *const i16,
    frames: libc::size_t,
) -> libc::size_t {
    let buffer_arc: Arc<Mutex<Vec<i16>>>;
    {
        let mut pool = BUFFER_POOL.lock().unwrap();
        buffer_arc = pool.pop().unwrap_or_else(|| Arc::new(Mutex::new(vec![0; BUFFER_LENGTH])));
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

    // Reuse and return buffers to the pool after processing.
    {
        let mut pool = BUFFER_POOL.lock().unwrap();
        pool.push(buffer_arc);
    }

    frames
}
