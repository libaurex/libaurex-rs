use ffmpeg_next::{ffi::AVAudioFifo, sys};
use std::{ffi::c_void, ptr};

#[unsafe(no_mangle)]
pub extern "C" fn data_callback(
    p_device: *mut miniaudio::ma_device, 
    p_output: *mut c_void, 
    _p_input: *const c_void, 
    frame_count: miniaudio::ma_uint32
) {
    unsafe {
        let fifo = (*p_device).pUserData as *mut AVAudioFifo;
        
        // Create a single-element array of pointers
        let mut data_ptrs = [p_output as *mut c_void];
        let data_ptrs_ptr = data_ptrs.as_mut_ptr() as *mut *mut c_void;

        let got = sys::av_audio_fifo_read(fifo, data_ptrs_ptr, frame_count as i32);
        if got < frame_count as i32 {
            // Handle underrun by zero-filling remaining buffer
            let bytes_per_sample = sys::av_get_bytes_per_sample(
                sys::AVSampleFormat::AV_SAMPLE_FMT_S16
            ) as usize;
            let channels = (*p_device).playback.channels as usize;
            let bytes_per_frame = bytes_per_sample * channels;
            let written_bytes = got as usize * bytes_per_frame;
            let total_bytes = frame_count as usize * bytes_per_frame;
            let remaining_bytes = total_bytes - written_bytes;
            
            ptr::write_bytes(
                p_output.add(written_bytes) as *mut u8,
                0,
                remaining_bytes
            );
        }
    }
}