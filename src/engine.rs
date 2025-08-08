use std::{ffi::{c_char, CStr}, io::{self, Read}, os::raw::c_void, ptr};

#[unsafe(no_mangle)]
pub extern "C" fn data_callback(
    p_device: *mut miniaudio::ma_device, 
    p_output: *mut c_void, 
    p_input: *const c_void, 
    frame_count: miniaudio::ma_uint32) -> () {
    
    unsafe {
        if p_device.is_null() {
            return;
        }

        let p_decoder = (*p_device).pUserData as *mut miniaudio::ma_decoder;
        if p_decoder.is_null() {
            return;
        }

        miniaudio::ma_decoder_read_pcm_frames(p_decoder, p_output, frame_count.into(), ptr::null_mut());

        let _ = p_input;

    }
}

#[unsafe(no_mangle)]
pub extern "C" fn play_audio(filepath: *const c_char) -> i32 {
    unsafe {

        if CStr::from_ptr(filepath).to_bytes().len() == 0 {
            print!("No file provided.");
            return -1;
        }

        let mut decoder: miniaudio::ma_decoder = std::mem::zeroed();
        let mut device: miniaudio::ma_device = std::mem::zeroed();

        let result = miniaudio::ma_decoder_init_file(filepath, ptr::null_mut(), &mut decoder);

        if result != miniaudio::ma_result_MA_SUCCESS {
            println!("Could not load file.");
            return -2;
        }

        let mut device_config = miniaudio::ma_device_config_init(miniaudio::ma_device_type_ma_device_type_playback);
        device_config.playback.format = decoder.outputFormat;
        device_config.playback.channels = decoder.outputChannels;
        device_config.sampleRate = decoder.outputSampleRate;
        device_config.dataCallback = Some(data_callback);
        device_config.pUserData = &mut decoder as *mut _ as *mut c_void;

        let device_result = miniaudio::ma_device_init(ptr::null_mut(), &device_config, &mut device);
        if device_result != miniaudio::ma_result_MA_SUCCESS {
            println!("Failed to init device.");
            miniaudio::ma_decoder_uninit(&mut decoder);
            return -3;
        }

        if miniaudio::ma_device_start(&mut device) != miniaudio::ma_result_MA_SUCCESS {
            println!("Failed to start playback");
            miniaudio::ma_device_uninit(&mut device);
            miniaudio::ma_decoder_uninit(&mut decoder);
            return -4;
        }

        println!("Press enter to quit...");
        let _ = io::stdin().read(&mut [0u8]).unwrap();

        miniaudio::ma_device_uninit(&mut device);
        miniaudio::ma_decoder_uninit(&mut decoder);
    }

    0
}