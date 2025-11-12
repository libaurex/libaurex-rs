use crate::{
    ffi::data_callback, 
    enums::{PlayerState, CMD, ResamplingQuality},
    singletons::{get_played}
};

use ffmpeg_next;
use miniaudio_aurex::{self as miniaudio, ma_device_config_init};
use soxr::{Soxr, format::{self}, params::{Interpolation, QualitySpec, RuntimeSpec}};

use std::{
    ffi::c_void,
    ptr,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self}
};


#[allow(unused_imports)]
use ffmpeg_next::{self as av, ffi::AVAudioFifo, frame::Audio as AudioFrame, media, sys};

pub struct AudioFifo(pub *mut AVAudioFifo);
unsafe impl Send for AudioFifo {}

pub struct AudioEngine {
    device: miniaudio::ma_device,
    buffer: Arc<Mutex<AudioFifo>>,
    channels: i32,
    sample_rate: Arc<Mutex<i32>>,
    state: Arc<Mutex<PlayerState>>,
    device_config: miniaudio::ma_device_config,
    initialised: bool,
    tx: Option<Sender<CMD>>,
    duration: Arc<Mutex<f64>>, //Total duration in seconds, -1.0 if theres nothing to play
    total_samples: Arc<Mutex<Option<u64>>>, // Total samples in current track
    resampling_quality: ResamplingQuality
}

impl AudioEngine {
    pub fn new(
        resampling_quality: Option<ResamplingQuality>
        
        ) -> Result<Self, i32> {

        let m_resampling_quality = resampling_quality.unwrap_or(ResamplingQuality::High);
        
        let mut device: miniaudio::ma_device = unsafe { std::mem::zeroed() };
        let buffer_ptr =
            unsafe { sys::av_audio_fifo_alloc(sys::AVSampleFormat::AV_SAMPLE_FMT_S32, 2, 1) };
        let buffer = Arc::new(Mutex::new(AudioFifo(buffer_ptr)));

        let mut device_config =
            unsafe { ma_device_config_init(miniaudio::ma_device_type_ma_device_type_playback) };

        device_config.playback.format = miniaudio::ma_format_ma_format_s32;
        device_config.playback.channels = 0;
        device_config.sampleRate = 0;
        device_config.dataCallback = Some(data_callback);
        device_config.pUserData = buffer.lock().unwrap().0 as *mut c_void;

        if cfg!(target_os = "windows") {
            println!("Detected Windows");
            device_config.wasapi.noAutoConvertSRC = miniaudio::MA_TRUE as u8;
            device_config.wasapi.noDefaultQualitySRC = miniaudio::MA_TRUE as u8;
        }

        let device_result =
            unsafe { miniaudio::ma_device_init(ptr::null_mut(), &device_config, &mut device) };

        if device_result != miniaudio::ma_result_MA_SUCCESS {
            println!("Failed to init device.");
        }

        println!(
            "Detected system configuration: {} channels at {} hz",
            device.playback.channels, device.sampleRate
        );

        let engine = AudioEngine {
            device: device,
            buffer: buffer,
            channels: device.playback.channels as i32,
            sample_rate: Arc::new(Mutex::new(device.sampleRate as i32)),
            state: Arc::new(Mutex::new(PlayerState::EMPTY)),
            device_config: device_config,
            initialised: false,
            tx: None,
            duration: Arc::new(Mutex::new(-1.0)),
            total_samples: Arc::new(Mutex::new(None)),
            resampling_quality: m_resampling_quality
        };

        Ok(engine)
    }

    pub fn get_duration(&self) -> f64 {
        *self.duration.lock().unwrap()
    }

    //Loads files. Automatically stops previous playback if any
    pub fn load(&mut self, file: &str) -> Result<(), i32> {
        // Clear any existing playback first
        self.clear()?;

        // Initialize decoder thread if needed
        if !self.initialised {
            let (tx, rx) = mpsc::channel::<CMD>();
            self.tx = Some(tx);
            _ = self.reinit_device(); //The state is mangled after using it to query the system config, hence re-init
            _ = self.spawn_decoder_thread(rx);
            self.initialised = true;
        }

        println!("Loading {}", &file);

        _ = self.tx.as_mut().unwrap().send(CMD::Start(file.to_string(), self.resampling_quality));

        Ok(())
    }

    pub fn get_progress(&self) -> Result<f64, i32> {
        let sample_rate = *self.sample_rate.lock().unwrap() as f64;
        if sample_rate <= 0.0 {
            return Err(-1);
        }
        let played_samples = get_played() as f64;
        Ok(played_samples / sample_rate)
    }

    //Clears the audio buffer
    pub fn clear(&mut self) -> Result<(), i32> {
        // Stop playback if active
        if *self.state.lock().unwrap() == PlayerState::PLAYING {
            self.pause()?;
        }

        // Clear the FIFO buffer
        unsafe {
            sys::av_audio_fifo_reset(self.buffer.lock().unwrap().0);
        }

        *self.state.lock().unwrap() = PlayerState::EMPTY;
        
        println!("Cleared audio buffer");
        Ok(())
    }

    //Plays
    pub fn play(&mut self) -> Result<(), i32> {
        if *self.state.lock().unwrap() != PlayerState::PLAYING {
            if unsafe { miniaudio::ma_device_start(&mut self.device) }
                != miniaudio::ma_result_MA_SUCCESS
            {
                println!("Failed to start device");
            } else {
                *self.state.lock().unwrap() = PlayerState::PLAYING;
            }
        }

        Ok(())
    }

    //Pauses playback
    pub fn pause(&mut self) -> Result<(), i32> {
        if *self.state.lock().unwrap() != PlayerState::PAUSED {
            if unsafe { miniaudio::ma_device_stop(&mut self.device) }
                != miniaudio::ma_result_MA_SUCCESS
            {
                println!("Failed to stop device");
            } else {
                *self.state.lock().unwrap() = PlayerState::PAUSED;
            }
        }

        Ok(())
    }

    // It has this weird quirk, the state gets mangled after querying system rates -> Assertion failed: ma_device_get_state(pDevice) == ma_device_state_starting
    // So re initializing the device fixes that
    fn reinit_device(&mut self) -> Result<(), i32> {
        unsafe {
            self.device = std::mem::zeroed();
            self.device_config.pUserData = Arc::into_raw(self.buffer.clone()) as *mut c_void;
            miniaudio::ma_device_init(ptr::null_mut(), &self.device_config, &mut self.device);
        }
        Ok(())
    }

    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    // <- DECODING LOGIC ->
    fn spawn_decoder_thread(&mut self, rx: Receiver<CMD>) -> Result<(), i32> {
        let sample_rate_handle = self.sample_rate.clone();
        let buffer_handle = self.buffer.clone();
        let duration_handle = self.duration.clone();
        let total_samples_handle = self.total_samples.clone();

        thread::spawn(move || {
            for cmd in rx {
                match cmd {
                    CMD::Start(url, resampling_quality) => {

                        //Standard ffmpeg decoding process
                        let mut format_ctx = av::format::input(&url).expect("Failed to open file.");

                        //Populate duration
                        let sample_rate = *sample_rate_handle.lock().unwrap() as f64;
                        let mut duration = duration_handle.lock().unwrap();
                        let mut total_samples = total_samples_handle.lock().unwrap();
                        *duration = format_ctx.duration() as f64 / f64::from(av::ffi::AV_TIME_BASE);
                        *total_samples = Some((*duration * sample_rate) as u64);

                        let audio_stream_index = format_ctx
                            .streams()
                            .best(media::Type::Audio)
                            .expect("No audio stream found.")
                            .index();

                        let codec_params = format_ctx
                            .streams()
                            .nth(audio_stream_index)
                            .expect("Stream Disappeared")
                            .parameters();
                        let codec_ctx = av::codec::context::Context::from_parameters(codec_params)
                            .expect("Failed to allocate codec context");

                        let mut decoder = codec_ctx
                            .decoder()
                            .audio()
                            .expect("Failed to open decoder.");

                        
                        //This is just for sample size conversion since soxr only does resampling
                        let mut resampler = av::software::resampling::Context::get(
                            decoder.format(),
                            decoder.channel_layout(),
                            decoder.rate(),
                            av::format::Sample::I32(av::format::sample::Type::Packed),
                            decoder.channel_layout(),
                            decoder.rate(),
                        )
                        .expect("Failed to init resampler");

                        //Actual resamppling happens here
                        let runtime = RuntimeSpec::new(0)
                            .with_interpolation(Interpolation::High);

                        let mut soxr_resampler = Soxr::<format::Interleaved<i32, 2>>::new_with_params(
                            decoder.rate() as f64, 
                            sample_rate, 
                            resampling_quality.get_quality_spec().expect("Failed to get quality spec for soxr."),
                            runtime
                        ).expect("Failed to setup soxr");

                        for (stream, packet) in format_ctx.packets() {
                            if stream.index() != audio_stream_index {
                                continue;
                            }

                            decoder
                                .send_packet(&packet)
                                .expect("Failed to send packet to decoder.");
                            let mut frame = AudioFrame::empty();

                            while decoder.receive_frame(&mut frame).is_ok() {
                                let mut resampled_frame = AudioFrame::empty();
                                _ = resampler.run(&frame, &mut resampled_frame);

                                //Convert ffmpeg's raw bytes into soxr's required array types
                                let input_samples: &[[i32; 2]] =
                                    bytemuck::cast_slice(resampled_frame.data(0));
                                let mut output_buf = vec![
                                    [0i32; 2];
                                    (input_samples.len() as usize
                                        * *sample_rate_handle.lock().unwrap()
                                            as usize)
                                        / decoder.rate() as usize
                                ];

                                let res = soxr_resampler
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
                                let dst_slice: &mut [[i32; 2]] =
                                    bytemuck::cast_slice_mut(data_plane);
                                dst_slice[..res.output_frames]
                                    .copy_from_slice(&output_buf[..res.output_frames]);

                                unsafe {
                                    let data_ptr0 =
                                        soxr_frame.data_mut(0).as_mut_ptr() as *mut c_void;
                                    let mut data_ptrs: [*mut c_void; 1] = [data_ptr0];

                                    let written = sys::av_audio_fifo_write(
                                        buffer_handle.lock().unwrap().0,
                                        data_ptrs.as_mut_ptr(),
                                        soxr_frame.samples() as i32,
                                    );

                                    if written < 0 {
                                        // Todo
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.pause();
        unsafe {
            miniaudio::ma_device_uninit(&mut self.device);
            if !self.buffer.lock().unwrap().0.is_null() {
                sys::av_audio_fifo_free(self.buffer.lock().unwrap().0);
            }
        }
    }
}