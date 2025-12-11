//engine.rs

use crate::{
    aurex::PlayerCallback,
    decoding_loop::decode,
    enums::{CMD, EngineSignal, PlayerState, ResamplingQuality},
    ffi::data_callback,
    singletons::{self, get_played, set_decoder_eof, set_total},
    structs::Decoder,
};

use ffmpeg_next::{self};
use miniaudio_aurex::{self as miniaudio, ma_device_config_init};
use soxr::{
    Soxr,
    format::{self},
    params::{Interpolation, RuntimeSpec},
};

use std::{
    ffi::c_void,
    i64,
    mem::zeroed,
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self}, time::Duration,
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use tokio::sync::{Mutex as async_Mutex, oneshot};

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
    resampling_quality: ResamplingQuality,
    signal_receiver: Receiver<EngineSignal>,
    user_data: Arc<Mutex<(Arc<Mutex<AudioFifo>>, Sender<EngineSignal>)>>,
    callback: Box<dyn PlayerCallback>,
    decoder: Arc<Mutex<Decoder>>,
}

impl AudioEngine {
    pub fn new(
        resampling_quality: Option<ResamplingQuality>,
        callback: Box<dyn PlayerCallback>,
    ) -> Result<Arc<async_Mutex<Self>>, i32> {
        let m_resampling_quality = resampling_quality.unwrap_or(ResamplingQuality::High);
        singletons::set_decoder_busy(false);

        let mut device: miniaudio::ma_device = unsafe { std::mem::zeroed() };
        let buffer_ptr =
            unsafe { sys::av_audio_fifo_alloc(sys::AVSampleFormat::AV_SAMPLE_FMT_S32, 2, 128_000) };
        let buffer = Arc::new(Mutex::new(AudioFifo(buffer_ptr)));

        let (signal_tx, signal_rx) = unbounded::<EngineSignal>();
        let m_user_data = (buffer.clone(), signal_tx);
        let user_data = Arc::new(Mutex::new(m_user_data));

        let mut device_config =
            unsafe { ma_device_config_init(miniaudio::ma_device_type_ma_device_type_playback) };

        device_config.playback.format = miniaudio::ma_format_ma_format_s32;
        device_config.playback.channels = 0;
        device_config.sampleRate = 0;
        device_config.dataCallback = Some(data_callback);
        device_config.pUserData = Arc::as_ptr(&user_data) as *mut c_void;

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

        let decoder: Arc<Mutex<Decoder>>;

        unsafe {
            decoder = Arc::new(Mutex::new(Decoder {
                format_ctx: zeroed(),
                decoder: zeroed(),
                resampler: zeroed(),
                soxr_resampler: zeroed(),
                audio_stream_index: zeroed(),
                main_decoder_cancel_flag: Arc::new(AtomicBool::new(false)),
            }));
        }

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
            resampling_quality: m_resampling_quality,
            signal_receiver: signal_rx,
            user_data: user_data,
            callback: callback,
            decoder: decoder,
        };

        Ok(Arc::new(async_Mutex::new(engine)))
    }

    pub fn get_duration(&self) -> f64 {
        *self.duration.lock().unwrap()
    }

    //Loads files. Automatically stops previous playback if any
    pub async fn load(audio_engine: Arc<async_Mutex<Self>>, file: &str) -> Result<(), i32> {
        // Clear any existing playback first
        let mut engine = audio_engine.lock().await;
        engine.clear()?;

        // Initialize decoder thread if needed
        if !engine.initialised {
            let (tx, rx) = unbounded::<CMD>();
            engine.tx = Some(tx);
            _ = engine.reinit_device(); //The state is mangled after using it to query the system config, hence re-init
            _ = engine.spawn_decoder_thread(rx.clone());
            _ = AudioEngine::spawn_listening_thread(
                audio_engine.clone(),
                engine.signal_receiver.clone(),
            );
            _ = engine.spawn_seeker_thread(rx.clone());
            engine.initialised = true;
        }

        println!("Loading {}", &file);
        let resampling_quality = engine.resampling_quality;
        _ = engine
            .tx
            .as_mut()
            .unwrap()
            .send(CMD::Start(file.to_string(), resampling_quality));

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
            self.device_config.pUserData = Arc::as_ptr(&self.user_data) as *mut c_void;
            miniaudio::ma_device_init(ptr::null_mut(), &self.device_config, &mut self.device);
        }
        Ok(())
    }

    fn spawn_listening_thread(
        engine: Arc<async_Mutex<Self>>,
        receiver: Receiver<EngineSignal>,
    ) -> Result<(), i32> {
        let res = thread::spawn(async move || {
            for signal in receiver {
                match signal {
                    EngineSignal::MediaEnd => {
                        set_decoder_eof(false);
                        let mut m_engine = engine.lock().await;
                        _ = m_engine.pause();
                        _ = m_engine.clear();
                        println!("Player empty and ready. Executing callback");
                        m_engine.callback.on_player_event(EngineSignal::MediaEnd);
                    }
                }
            }
        });

        _ = res.join();

        Ok(())
    }

    pub async fn seek(&mut self, time_s: f64) -> Result<(), i32> {

        loop {
            if *self.state.lock().unwrap() != PlayerState::INITIALISED {
                println!("Not initialised");
                thread::sleep(Duration::from_millis(5));
            } else {
                break;
            }
        }
        _ = self.pause();

        {
            let decoder = self.decoder.lock().unwrap();
            decoder
                .main_decoder_cancel_flag
                .store(true, Ordering::Relaxed);
        }

        _ = self.clear();

        let (tx_done, rx_done) = oneshot::channel();
        _ = self.tx.clone().unwrap().send(CMD::Seek {
            time_s: time_s,
            done: tx_done,
        });
        _ = rx_done.await;

        {
            let decoder = self.decoder.lock().unwrap();
            decoder
                .main_decoder_cancel_flag
                .store(false, Ordering::Relaxed);
        }

        let tx = self.tx.as_ref().unwrap().clone();
        _ = tx.send(CMD::Resume);
        _ = self.play();

        Ok(())
    }




























    // <- DECODING LOGIC ->
    fn spawn_seeker_thread(&mut self, rx: Receiver<CMD>) -> Result<(), i32> {
        let decoder_handle = self.decoder.clone();

        thread::spawn(move || {
            for cmd in rx {
                if let CMD::Seek { time_s, done } = cmd {
                    let mut decoder = decoder_handle.lock().unwrap();
                    
                    let target_ts = (time_s * 1_000_000.0) as i64;

                    _ = decoder
                        .format_ctx
                        .as_mut()
                        .unwrap()
                        .seek(target_ts, i64::MIN..i64::MAX);
                    decoder.decoder.flush();
                    let mut dump = AudioFrame::empty();
                    _ = decoder.resampler.flush(&mut dump);
                    _ = decoder.soxr_resampler.clear();
                    _ = done.send(());
                }
            }
        });

        Ok(())
    }

    fn spawn_decoder_thread(&mut self, rx: Receiver<CMD>) -> Result<(), i32> {
        let sample_rate_handle = self.sample_rate.clone();
        let buffer_handle = self.buffer.clone();
        let duration_handle = self.duration.clone();
        let total_samples_handle = self.total_samples.clone();
        let state_handle = self.state.clone();

        let decoder_handle = self.decoder.clone();

        thread::spawn(move || {
            for cmd in rx {
                if let CMD::Start(url, resampling_quality) = cmd {
                    let mut m_decoder = decoder_handle.lock().unwrap();
                    m_decoder.format_ctx =
                        Some(av::format::input(&url).expect("Failed to open file."));

                    //Populate duration
                    let sample_rate = *sample_rate_handle.lock().unwrap() as f64;
                    let mut duration = duration_handle.lock().unwrap();
                    let mut total_samples = total_samples_handle.lock().unwrap();
                    *duration = m_decoder.format_ctx.as_mut().unwrap().duration() as f64
                        / f64::from(av::ffi::AV_TIME_BASE);
                    *total_samples = Some((*duration * sample_rate) as u64);
                    set_total(total_samples.unwrap());

                    let audio_stream_index = m_decoder
                        .format_ctx
                        .as_mut()
                        .unwrap()
                        .streams()
                        .best(media::Type::Audio)
                        .expect("No audio stream found.")
                        .index();

                    m_decoder.audio_stream_index = audio_stream_index;
                    let codec_params = m_decoder
                        .format_ctx
                        .as_mut()
                        .unwrap()
                        .streams()
                        .nth(audio_stream_index)
                        .expect("Stream Disappeared")
                        .parameters();

                    let codec_ctx = av::codec::context::Context::from_parameters(codec_params)
                        .expect("Failed to allocate codec context");

                    m_decoder.decoder = codec_ctx
                        .decoder()
                        .audio()
                        .expect("Failed to open decoder.");

                    //This is just for sample size conversion since soxr only does resampling
                    m_decoder.resampler = av::software::resampling::Context::get(
                        m_decoder.decoder.format(),
                        m_decoder.decoder.channel_layout(),
                        m_decoder.decoder.rate(),
                        av::format::Sample::I32(av::format::sample::Type::Packed),
                        m_decoder.decoder.channel_layout(),
                        m_decoder.decoder.rate(),
                    )
                    .expect("Failed to init resampler");

                    //Actual resamppling happens here
                    let soxr_runtime = RuntimeSpec::new(0).with_interpolation(Interpolation::High);

                    m_decoder.soxr_resampler =
                        Soxr::<format::Interleaved<i32, 2>>::new_with_params(
                            m_decoder.decoder.rate() as f64,
                            sample_rate,
                            resampling_quality
                                .get_quality_spec()
                                .expect("Failed to get quality spec for soxr."),
                            soxr_runtime,
                        )
                        .expect("Failed to setup soxr");

                    //Prime the resampler. At higher quality levels there's artifacting at the start due to lack of previous data
                    let silence: Vec<[i32; 2]> = vec![[0, 0]; 1024]; 
                    let mut dummy_output: Vec<[i32; 2]> = vec![[0, 0]; 2048];
                    _ = m_decoder.soxr_resampler.process(&silence, &mut dummy_output);

                    let mut state = state_handle.lock().unwrap();
                    *state = PlayerState::INITIALISED;
                    println!("Initialised decoders");
                    drop(m_decoder);

                    _ = decode(
                        decoder_handle.clone(),
                        sample_rate_handle.clone(),
                        buffer_handle.clone(),
                    );
                }
                else if let CMD::Resume = cmd {
                    _ = decode(decoder_handle.clone(), sample_rate_handle.clone(), buffer_handle.clone());
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

unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}
