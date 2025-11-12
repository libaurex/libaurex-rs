use ffmpeg_next::{sys};
use std::{ffi::c_void, ptr, sync::Arc};
use miniaudio_aurex as miniaudio;

use crate::engine::{
    AudioFifo, add_played,
};


#[unsafe(no_mangle)]
pub extern "C" fn data_callback(
    p_device: *mut miniaudio::ma_device,
    p_output: *mut c_void,
    _p_input: *const c_void,
    frame_count: miniaudio::ma_uint32,
) {
    unsafe {
        // Reconstruct Arc without changing refcount permanently
        let arc_ptr = (*p_device).pUserData as *const std::sync::Mutex<AudioFifo>;
        let arc = Arc::from_raw(arc_ptr);
        let arc_clone = arc.clone();
        let _ = Arc::into_raw(arc); // Restore original ownership
        
        let bytes_per_sample = sys::av_get_bytes_per_sample(
            sys::AVSampleFormat::AV_SAMPLE_FMT_S32
        ) as usize;
        let channels = (*p_device).playback.channels as usize;
        let bytes_per_frame = bytes_per_sample * channels;
        let total_bytes = frame_count as usize * bytes_per_frame;
        
        match arc_clone.try_lock() {
            Ok(guard) => {
                let fifo = guard.0;
                let available = sys::av_audio_fifo_size(fifo);
                let mut data_ptrs = [p_output as *mut c_void];
                
                let frames_to_read = available.min(frame_count as i32);
                
                let got = if frames_to_read > 0 {
                    sys::av_audio_fifo_read(
                        fifo,
                        data_ptrs.as_mut_ptr(),
                        frames_to_read
                    )
                } else {
                    0
                };

                //Increment global played samples tracker
                if got > 0 {
                    add_played(got as u64);
                }
                
                // Zero-fill any remaining frames
                if got < frame_count as i32 {
                    let written_bytes = got as usize * bytes_per_frame;
                    let remaining_bytes = total_bytes.saturating_sub(written_bytes);
                    ptr::write_bytes(
                        (p_output as *mut u8).add(written_bytes),
                        0,
                        remaining_bytes,
                    );
                }
            }
            Err(_) => {
                // Lock contention - zero-fill entire buffer to avoid glitches
                ptr::write_bytes(p_output as *mut u8, 0, total_bytes);
            }
        }
    }
}