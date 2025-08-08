use std::{ffi::c_void, io::{self, Read}, ptr};

use ffmpeg_next::{
    self as av, ffi::{AVAudioFifo}, frame::Audio as AudioFrame, media, sys
    };

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
        
        // Correctly cast the array pointer
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

pub fn play_audio_ffmpeg(filepath: &str) -> i32 {
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

    //Set up resampler
    // Allocate FIFO for interleaved i16 (match resampler output)
    let audio_buffer = unsafe {
        sys::av_audio_fifo_alloc(
            sys::AVSampleFormat::AV_SAMPLE_FMT_S32,  // Interleaved i16
            decoder.channels() as i32,
            1
        )
    };
    let mut resampler = av::software::resampling::Context::get(
        decoder.format(),
        decoder.channel_layout(),
        decoder.rate(),
        av::format::Sample::I32(av::format::sample::Type::Packed), // Explicitly interleaved
        decoder.channel_layout(),
        decoder.rate()
    ).expect("Failed to setup resampler.");

    for (stream, packet) in format_ctx.packets() {
        if stream.index() != audio_stream_index {
            continue;
        }

        decoder.send_packet(&packet)
            .expect("failed to send packet");

        let mut frame = AudioFrame::empty();
        while decoder.receive_frame(&mut frame).is_ok() {
            
            let mut resampled_frame = AudioFrame::empty();
            _ = resampler.run(&frame, &mut resampled_frame);

            unsafe {
                // 1) Get a mutable pointer to the first byte of your frame buffer:
                let data_ptr0 = resampled_frame.data_mut(0).as_mut_ptr() as *mut c_void;

                // 2) Build a small array of channel-pointers:
                //    (mono → one pointer; stereo → two pointers, etc.)
                let mut data_ptrs: [*mut c_void; 1] = [data_ptr0];

                // 3) Pass the array’s pointer (i.e. *mut *mut c_void) to FFmpeg:
                let written = sys::av_audio_fifo_write(
                    audio_buffer,
                    data_ptrs.as_mut_ptr(),
                    resampled_frame.samples() as i32,
                );

                if written < 0 {
                    // handle error...
                }
            }

        }
    }

    //Playback
    let mut device: miniaudio::ma_device = unsafe { std::mem::zeroed() };
    let mut device_config = unsafe { miniaudio::ma_device_config_init(miniaudio::ma_device_type_ma_device_type_playback) };
        
    device_config.playback.format = miniaudio::ma_format_ma_format_s32;
    device_config.playback.channels = decoder.channels() as u32;
    device_config.sampleRate = decoder.rate();
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

    if unsafe { miniaudio::ma_device_start(&mut device) } != miniaudio::ma_result_MA_SUCCESS {
        println!("Failed to start playback");
        unsafe { miniaudio::ma_device_uninit(&mut device) };
        return -4;
    }

    println!("Press enter to quit...");
    let _ = io::stdin().read(&mut [0u8]).unwrap();

    unsafe { 
        miniaudio::ma_device_uninit(&mut device);
        sys::av_audio_fifo_free(audio_buffer);
    };

    0
}
