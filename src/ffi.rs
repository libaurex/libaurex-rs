use ffmpeg_next::sys;
use std::{ffi::c_void, i32, ptr, sync::Arc, thread, time::Duration};
use miniaudio_aurex as miniaudio;

use crate::{
    engine::AudioFifo,
    singletons::{add_played, get_decoder_eof, get_volume},
};

use std::sync::Mutex;
use crossbeam_channel::Sender;
use crate::enums::EngineSignal;

#[unsafe(no_mangle)]
pub extern "C" fn data_callback(
    p_device: *mut miniaudio::ma_device,
    p_output: *mut c_void,
    _p_input: *const c_void,
    frame_count: miniaudio::ma_uint32,
) {
    unsafe {
        // Defensive checks
        if p_device.is_null() || p_output.is_null() {
            return;
        }

        // bytes and sizing
        let bytes_per_sample = sys::av_get_bytes_per_sample(sys::AVSampleFormat::AV_SAMPLE_FMT_S32) as usize;
        let channels = (*p_device).playback.channels as usize;
        let bytes_per_frame = bytes_per_sample.saturating_mul(channels);
        let total_bytes = (frame_count as usize).saturating_mul(bytes_per_frame);

        // pUserData is a pointer to the inner Mutex<(Arc<Mutex<AudioFifo>>, Sender<EngineSignal>)>
        // This pointer was set using Arc::as_ptr(...) in engine.rs so we must not call Arc::from_raw here.
        let user_data_ptr = (*p_device).pUserData as *const Mutex<(Arc<Mutex<AudioFifo>>, Sender<EngineSignal>)>;
        if user_data_ptr.is_null() {
            // must not crash in callback; zero-fill and bail
            ptr::write_bytes(p_output as *mut u8, 0, total_bytes);
            return;
        }

        //the owning Arc is kept alive by AudioEngine. Use the raw pointer as a reference.
        let user_data_ref: &Mutex<(Arc<Mutex<AudioFifo>>, Sender<EngineSignal>)> = &*user_data_ptr;

        match user_data_ref.try_lock() {
            Ok(guard) => {
                // clone inner handles quickly then drop guard to minimize lock hold time
                let buffer_arc = guard.0.clone();
                let _signal_tx = guard.1.clone();
                drop(guard);

                match buffer_arc.try_lock() {
                    Ok(buffer_guard) => {
                        let fifo = buffer_guard.0;
                        let available = sys::av_audio_fifo_size(fifo);

                        // Prevent buffer underrun/clicking at startup.
                        if available < (frame_count as i32) && !get_decoder_eof() {
                            ptr::write_bytes(p_output as *mut u8, 0, total_bytes);
                            return;
                        }

                        let mut data_ptrs = [p_output as *mut c_void];

                        let frames_to_read = available.min(frame_count as i32);

                        let got = if frames_to_read > 0 {
                            sys::av_audio_fifo_read(
                                fifo,
                                data_ptrs.as_mut_ptr(),
                                frames_to_read,
                            )
                        } else {
                            0
                        };

                        
                        if got > 0 {
                            // Increment global played samples tracker
                            add_played(got as u64);

                            //Apply gain if necessary
                            let vol = get_volume();
                            if vol != 1.0 {
                                let sample_count = (got as usize) * channels;
                                let samples = p_output as *mut i32;

                                for i in 0..sample_count {
                                    let s = *samples.add(i) as f32;
                                    *samples.add(i) = (s * vol)
                                        .clamp(i32::MIN as f32, i32::MAX as f32) as i32;
                                }
                            }
                        }

                        if get_decoder_eof() && (got == 0) {
                            //Media Ended
                            _ = _signal_tx.send(EngineSignal::MediaEnd);
                        }

                        // Zero-fill any remaining frames
                        if got < frame_count as i32 {
                            let written_bytes = (got as usize).saturating_mul(bytes_per_frame);
                            let remaining_bytes = total_bytes.saturating_sub(written_bytes);
                            ptr::write_bytes(
                                (p_output as *mut u8).add(written_bytes),
                                0,
                                remaining_bytes,
                            );
                        }
                    }
                    Err(_) => {
                        // Buffer lock contention - zero-fill entire buffer to avoid glitches
                        ptr::write_bytes(p_output as *mut u8, 0, total_bytes);
                    }
                }
            }
            Err(_) => {
                // Lock contention on outer user_data - zero-fill entire buffer to avoid glitches
                ptr::write_bytes(p_output as *mut u8, 0, total_bytes);
            }
        }
    }
}
