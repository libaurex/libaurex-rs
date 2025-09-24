use ffmpeg_next::{ffi::AVAudioFifo, sys};
use std::{ffi::c_void, ptr, sync::Arc};
use miniaudio_aurex as miniaudio;
use crate::engine::AudioFifo;

#[unsafe(no_mangle)]
pub extern "C" fn data_callback(
    p_device: *mut miniaudio::ma_device,
    p_output: *mut c_void,
    _p_input: *const c_void,
    frame_count: miniaudio::ma_uint32,
) {
    unsafe {
        // reconstruct Arc without changing refcount permanently:
        let arc_ptr = (*p_device).pUserData as *const std::sync::Mutex<AudioFifo>;
        let arc = Arc::from_raw(arc_ptr);         // temporarily owns one ref
        // clone to bump refcount for our short usage, then restore original raw
        let arc_clone = arc.clone();
        let _ = Arc::into_raw(arc);               // restore raw so original refcount unaffected

        // non-blocking lock: if busy, consider underrun
        match arc_clone.try_lock() {
            Ok(mut guard) => {
                let fifo = guard.0;
                let available = sys::av_audio_fifo_size(fifo);
                let mut data_ptrs = [p_output as *mut c_void];
                let got = if available >= frame_count as i32 {
                    sys::av_audio_fifo_read(fifo, data_ptrs.as_mut_ptr(), frame_count as i32)
                } else {
                    // read whatever is available (may be 0)
                    if available > 0 {
                        sys::av_audio_fifo_read(fifo, data_ptrs.as_mut_ptr(), available)
                    } else {
                        0
                    }
                };

                if got < frame_count as i32 {
                    // zero-fill the rest
                    let bytes_per_sample = sys::av_get_bytes_per_sample(
                        sys::AVSampleFormat::AV_SAMPLE_FMT_S16
                    ) as usize;
                    let channels = (*p_device).playback.channels as usize;
                    let bytes_per_frame = bytes_per_sample * channels;
                    let written_bytes = got as usize * bytes_per_frame;
                    let total_bytes = frame_count as usize * bytes_per_frame;
                    let remaining_bytes = total_bytes.saturating_sub(written_bytes);

                    ptr::write_bytes(
                        (p_output as *mut u8).add(written_bytes),
                        0,
                        remaining_bytes,
                    );
                }
            }
            Err(_) => {
                // lock busy: consider it's an underrun — zero-fill whole buffer
                let bytes_per_sample = sys::av_get_bytes_per_sample(
                    sys::AVSampleFormat::AV_SAMPLE_FMT_S16
                ) as usize;
                let channels = (*p_device).playback.channels as usize;
                let bytes_per_frame = bytes_per_sample * channels;
                let total_bytes = frame_count as usize * bytes_per_frame;
                ptr::write_bytes(p_output as *mut u8, 0, total_bytes);
            }
        }
        // arc_clone drops here, decreasing refcount as expected
    }
}
