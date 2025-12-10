use crate::{engine::AudioFifo, singletons::set_decoder_eof, structs::Decoder};
#[allow(unused_imports)]
use ffmpeg_next::{self as av, ffi::AVAudioFifo, frame::Audio as AudioFrame, media, sys};

use std::{ffi::c_void, sync::{Arc, Mutex, atomic::Ordering}};

pub fn decode(
        decoder_handle: Arc<Mutex<Decoder>>,
        sample_rate_handle: Arc<Mutex<i32>>,
        buffer_handle: Arc<Mutex<AudioFifo>>
    ) -> Result<(), i32> {

    let mut m_decoder = decoder_handle.lock().unwrap();
    let mut format_ctx = m_decoder.format_ctx.take().unwrap();
    drop(m_decoder);

    //Decoding loop
    for (stream, packet) in format_ctx.packets() {
        let mut m_decoder = decoder_handle.lock().unwrap();

        //Check if loop needs to be interrupted
        if m_decoder.main_decoder_cancel_flag.load(Ordering::Relaxed) {
            m_decoder.format_ctx = Some(format_ctx);
            println!("Interrupting decoder");
            return Ok::<(), i32>(());
        }

        if stream.index() != m_decoder.audio_stream_index {
            continue;
        }

        m_decoder
            .decoder
            .send_packet(&packet)
            .expect("Failed to send packet to decoder.");
        let mut frame = AudioFrame::empty();

        while m_decoder.decoder.receive_frame(&mut frame).is_ok() {
            let mut resampled_frame = AudioFrame::empty();
            _ = m_decoder.resampler.run(&frame, &mut resampled_frame);

            //Convert ffmpeg's raw bytes into soxr's required array types
            let input_samples: &[[i32; 2]] = bytemuck::cast_slice(resampled_frame.data(0));
            let mut output_buf = vec![
                [0i32; 2];
                (input_samples.len() as usize
                    * *sample_rate_handle.lock().unwrap() as usize)
                    / m_decoder.decoder.rate() as usize
            ];

            let res = m_decoder
                .soxr_resampler
                .process(input_samples, &mut output_buf)
                .unwrap();

            let mut soxr_frame = AudioFrame::new(
                av::format::Sample::I32(av::format::sample::Type::Packed),
                res.output_frames,
                av::ChannelLayout::STEREO,
            );

            //Copy soxr's output to ffmpeg frame bit of a mess cause soxr gives you nice typed arrays but ffmpeg just uses raw bytes
            soxr_frame.set_rate(*sample_rate_handle.lock().unwrap() as u32);

            let data_plane = soxr_frame.data_mut(0);
            let dst_slice: &mut [[i32; 2]] = bytemuck::cast_slice_mut(data_plane);
            dst_slice[..res.output_frames].copy_from_slice(&output_buf[..res.output_frames]);

            unsafe {
                let data_ptr0 = soxr_frame.data_mut(0).as_mut_ptr() as *mut c_void;
                let mut data_ptrs: [*mut c_void; 1] = [data_ptr0];

                let written = sys::av_audio_fifo_write(
                    buffer_handle.lock().unwrap().0,
                    data_ptrs.as_mut_ptr(),
                    soxr_frame.samples() as i32,
                );

                // println!("Got to the end");

                if written < 0 {
                    // Todo
                }
            }
        }
    }

    // Put format_ctx back after using it
    m_decoder = decoder_handle.lock().unwrap();
    m_decoder.format_ctx = Some(format_ctx);

    set_decoder_eof(true);

    Ok(())
}
