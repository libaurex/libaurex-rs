use std::{ffi::c_void, io::{self, Read}, ptr};
use soxr::{format, Soxr};
use crate::ffi::data_callback;

#[allow(unused_imports)] //IDK why it thinks I'm not using AVAudioFifo
use ffmpeg_next::{
    self as av, ffi::AVAudioFifo, frame::Audio as AudioFrame, media, sys
};

pub fn play_audio(filepath: &str) -> i32 {
    // Open file
    let mut format_ctx = av::format::input(filepath)
        .expect("Failed to open file.");

    let audio_stream_index = format_ctx
        .streams()
        .best(media::Type::Audio)
        .expect("No audio stream found")
        .index();

    // Set up decoder
    let codec_params = format_ctx
                                        .streams()
                                        .nth(audio_stream_index)
                                        .expect("Stream disappeared")
                                        .parameters();

    let codec_ctx = av::codec::context::Context::from_parameters(codec_params)
        .expect("Failed to allocate codec context.");
    let mut decoder = codec_ctx.decoder()
        .audio()
        .expect("Failed to open decoder.");

    let audio_buffer = unsafe {
        sys::av_audio_fifo_alloc(
            sys::AVSampleFormat::AV_SAMPLE_FMT_S32,  // Interleaved i16
            decoder.channels() as i32,
            1
        )
    };

    //Playback
    let mut device: miniaudio::ma_device = unsafe { std::mem::zeroed() };
    let mut device_config = unsafe { miniaudio::ma_device_config_init(miniaudio::ma_device_type_ma_device_type_playback) };
        
    device_config.playback.format = miniaudio::ma_format_ma_format_s32;
    device_config.playback.channels = decoder.channels() as u32;
    device_config.sampleRate = 0;
    device_config.dataCallback = Some(data_callback);
    device_config.pUserData = audio_buffer as *mut c_void;


    //WINDOWS ONLY
    device_config.wasapi.noAutoConvertSRC = miniaudio::MA_TRUE as u8;
    device_config.wasapi.noDefaultQualitySRC = miniaudio::MA_TRUE as u8;

    let device_result = unsafe { miniaudio::ma_device_init(ptr::null_mut(), &device_config, &mut device) };
    if device_result != miniaudio::ma_result_MA_SUCCESS {
        println!("Failed to init device.");
        return -3;
    }

    let mut resampler = av::software::resampling::Context::get(
        decoder.format(),
        decoder.channel_layout(),
        decoder.rate(),
        av::format::Sample::I32(av::format::sample::Type::Packed), // Explicitly interleaved
        decoder.channel_layout(),
        decoder.rate()
    ).expect("Failed to setup resampler.");

    let mut soxr_resampler = Soxr::<format::Interleaved<i32, 2>>::new(
        decoder.rate().into(),
        device.sampleRate.into()
    ).expect("Failed to initialize soxr");


    for (stream, packet) in format_ctx.packets() {
        if stream.index() != audio_stream_index {
            continue;
        }

        decoder.send_packet(&packet)
            .expect("failed to send packet");

        let mut frame = AudioFrame::empty();
        
        //Decode shit
        while decoder.receive_frame(&mut frame).is_ok() {
            
            let mut resampled_frame = AudioFrame::empty();
            _ = resampler.run(&frame, &mut resampled_frame);

            let input_samples: &[[i32; 2]] = bytemuck::cast_slice(resampled_frame.data(0));
            let mut output_buf = vec![[0i32; 2]; (input_samples.len() as usize * device.sampleRate as usize) / decoder.rate() as usize];

            let res = soxr_resampler.process(input_samples, &mut output_buf).unwrap();

            let mut soxr_frame = AudioFrame::new(
                av::format::Sample::I32(av::format::sample::Type::Packed),
                res.output_frames,
                av::ChannelLayout::STEREO
            );
            soxr_frame.set_rate(device.sampleRate as u32);

            // Copy the processed samples into the FFmpeg frame
            let data_plane = soxr_frame.data_mut(0);
            let dst_slice: &mut [[i32; 2]] = bytemuck::cast_slice_mut(data_plane);
            dst_slice[..res.output_frames].copy_from_slice(&output_buf[..res.output_frames]);

            unsafe {
                // 1) Get a mutable pointer to the first byte of the frame buffer:
                let data_ptr0 = soxr_frame.data_mut(0).as_mut_ptr() as *mut c_void;

                // 2) Build a small array of channel-pointers:
                //Now normally only one pointer would mean mono audio but we are using interleaved pcm so all channels are mashed into one.
                let mut data_ptrs: [*mut c_void; 1] = [data_ptr0];

                let written = sys::av_audio_fifo_write(
                    audio_buffer,
                    data_ptrs.as_mut_ptr(),
                    soxr_frame.samples() as i32,
                );

                if written < 0 {
                    // Todo
                }
            }

        }
    }

    

    if unsafe { miniaudio::ma_device_start(&mut device) } != miniaudio::ma_result_MA_SUCCESS {
        println!("Failed to start playback");
        unsafe { miniaudio::ma_device_uninit(&mut device) };
        return -4;
    }

    println!("Playing at {} khz", device.sampleRate);
    println!("Press enter to quit...");
    let _ = io::stdin().read(&mut [0u8]).unwrap();

    unsafe { 
        miniaudio::ma_device_uninit(&mut device);
        sys::av_audio_fifo_free(audio_buffer);
    };

    0
}
