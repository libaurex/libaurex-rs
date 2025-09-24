use std::{ffi::c_void, io::{self, Read}, ptr, sync::mpsc::{self, Receiver, Sender}, thread::{self, Thread}, time::Instant};
use ffmpeg_next::decoder::decoder;
use soxr::{format, Soxr};
use crate::ffi::data_callback;
use miniaudio_aurex::{self as miniaudio, ma_device_config_init, ma_device_start, ma_device_stop};

#[allow(unused_imports)] //IDK why it thinks I'm not using AVAudioFifo
use ffmpeg_next::{
    self as av, ffi::AVAudioFifo, frame::Audio as AudioFrame, media, sys
};

pub enum PlayerState {
    LOADING = 0,
    LOADED,
    PLAYING,
    PAUSED,
    EMPTY
}

enum CMD {
    Start
}

pub struct AudioEngine {
    device: miniaudio::ma_device,
    buffer: *mut AVAudioFifo,
    channels: i32,
    sample_rate: i32,
    decoder: Option<av::codec::decoder::audio::Audio>,
    state: PlayerState,
    format_ctx: Option<av::format::context::Input>,
    device_config: miniaudio::ma_device_config,
    initialised: bool,
    tx: Option<Sender<CMD>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, i32> {
        let mut device: miniaudio::ma_device = unsafe {
            std::mem::zeroed()
        };
        let buffer: *mut AVAudioFifo = unsafe {
            sys::av_audio_fifo_alloc(sys::AVSampleFormat::AV_SAMPLE_FMT_S32, 2, 1)
        }; 
        let mut device_config = unsafe {
            ma_device_config_init(miniaudio::ma_device_type_ma_device_type_playback)
        };

        device_config.playback.format = miniaudio::ma_format_ma_format_s32;
        device_config.playback.channels = 0;
        device_config.sampleRate = 0;
        device_config.dataCallback = Some(data_callback);
        device_config.pUserData = buffer as *mut c_void;

        if cfg!(target_os = "windows") {
            println!("Detected Windows");
            device_config.wasapi.noAutoConvertSRC = miniaudio::MA_TRUE as u8;
            device_config.wasapi.noDefaultQualitySRC = miniaudio::MA_TRUE as u8;
        }

        let device_result = unsafe { miniaudio::ma_device_init(ptr::null_mut(), &device_config, &mut device) };
        
        if device_result != miniaudio::ma_result_MA_SUCCESS {
            println!("Failed to init device.");
        }

        println!("Detected system configuration: {} channels at {} hz", device.playback.channels, device.sampleRate);

        

        let mut engine = AudioEngine { 
                device: device, 
                buffer: buffer,
                channels: device.playback.channels as i32,
                sample_rate: device.sampleRate as i32,
                decoder: None,
                state: PlayerState::EMPTY,
                format_ctx: None,
                device_config: device_config,
                initialised: false,
                tx: None
            };


        Ok(engine)
    }

    pub fn load(&mut self, file: &str) -> Result<(), i32> {
        
        //Doing it here because and initialised object is needed
        if !self.initialised {
            let (tx, rx) = mpsc::channel::<CMD>();
            self.tx = Some(tx);
            self.reinit_device(); //idk why it needs that, it throws some device state error
            self.spawn_decoder_thread(rx);
            self.initialised = true;
        }

        self.state = PlayerState::LOADING;
        let mut format_ctx = av::format::input(&file).expect("Failed to open file.");
        let audio_stream_index = format_ctx.streams()
            .best(media::Type::Audio)
            .expect("No audio stream found.").index();

        let codec_params = format_ctx.streams()
            .nth(audio_stream_index)
            .expect("Stream Disappeared").parameters();
        let codec_ctx = av::codec::context::Context::from_parameters(codec_params).expect("Failed to allocate codec context");
        self.decoder = Some(codec_ctx.decoder()
            .audio().expect("Failed to open decoder."));

        let decoder = self.decoder.as_mut().expect("Decoder not Initialised");
        self.format_ctx = Some(format_ctx);
        self.state = PlayerState::LOADED;
        println!("Loaded {}", &file);

        Ok(())
    }

    pub fn play(&mut self) -> Result<(), i32> {
        let decoder = self.decoder.as_mut().expect("Decoder not initialized");

        let mut resampler = av::software::resampling::Context::get(
            decoder.format(), 
            decoder.channel_layout(), 
            decoder.rate(), 
            av::format::Sample::I32(av::format::sample::Type::Packed), //Basically just changing sample format as soxr is only capable of resampling the rate 
            decoder.channel_layout(), 
            decoder.rate()
        ).expect("Failed to init resampler");

        let mut soxr_resampler = Soxr::<format::Interleaved<i32, 2>>::new(decoder.rate() as f64, self.sample_rate as f64).expect("Failed to setup Soxr");

        let format_ctx = self.format_ctx.as_mut().expect("Format Context not initialised");

        let audio_stream_index = format_ctx.streams()
            .best(media::Type::Audio)
            .expect("No audio stream found.").index();

        for (stream, packet) in format_ctx.packets() {
            if stream.index() != audio_stream_index {
                continue;
            }

            decoder.send_packet(&packet).expect("Failed to send packet to decoder.");
            let mut frame = AudioFrame::empty();

            while decoder.receive_frame(&mut frame).is_ok() {
                let mut resampled_frame = AudioFrame::empty();
                _ = resampler.run(&frame, &mut resampled_frame);

                let input_samples: &[[i32; 2]] = bytemuck::cast_slice(resampled_frame.data(0));
                let mut output_buf = vec![[0i32; 2]; (input_samples.len() as usize * self.sample_rate as usize) / decoder.rate() as usize];

                let res = soxr_resampler.process(input_samples, &mut output_buf).unwrap();

                let mut soxr_frame = AudioFrame::new(
                    av::format::Sample::I32(av::format::sample::Type::Packed),
                    res.output_frames,
                    av::ChannelLayout::STEREO
                );
                soxr_frame.set_rate(self.sample_rate as u32);

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
                        self.buffer,
                        data_ptrs.as_mut_ptr(),
                        soxr_frame.samples() as i32,
                    );

                    if written < 0 {
                        // Todo
                    }
                }
            }
        }

        // self.reinit_device();


        if unsafe { miniaudio::ma_device_start(&mut self.device) } != miniaudio::ma_result_MA_SUCCESS {
            println!("Failed to start device");
        }

        Ok(())
    }

    fn reinit_device(&mut self) -> Result<(), i32> {
        unsafe {
            std::mem::drop(self.device);
            self.device = std::mem::zeroed();
            self.device_config.pUserData = self.buffer as *mut c_void;
            miniaudio::ma_device_init(ptr::null_mut(), &self.device_config, &mut self.device);
        }
        Ok(())
    }

    fn spawn_decoder_thread(&mut self, rx: Receiver<CMD>) -> Result<(), i32> {
        thread::spawn(move || {
            for cmd in rx {
                match cmd {
                    CMD::Start => {
                        
                    }
                }
            }            
        });
        
        Ok(())
    }

}