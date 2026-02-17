//engine.rs

use crate::{
    aurex::{Player, PlayerCallback},
    decoding_loop::decode,
    enums::{CMD, EngineSignal, PlayerState, ResamplingQuality},
    singletons::{self, add_played, get_decoder_eof, get_played, get_volume as f_get_volume, reset_played, set_decoder_eof, set_played, set_total, set_volume as f_set_volume},
    structs::Decoder,
};

use ffmpeg_next::{self};
use soxr::{
    Soxr,
    format::{self},
    params::{Interpolation, RuntimeSpec},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;

use std::{
    any::Any, ffi::c_void, i64, mem::zeroed, sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    }, thread::{self}, time::Duration
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use tokio::sync::{Mutex as async_Mutex};
use tokio::runtime::Handle;

#[allow(unused_imports)]
use ffmpeg_next::{self as av, ffi::AVAudioFifo, frame::Audio as AudioFrame, media, sys};

pub struct AudioFifo(pub *mut AVAudioFifo);
unsafe impl Send for AudioFifo {}

pub struct AudioEngine {
    stream: Option<Stream>,
    buffer: Arc<Mutex<AudioFifo>>,
    channels: i32,
    sample_rate: Arc<Mutex<i32>>,
    state: Arc<Mutex<PlayerState>>,
    initialised: bool,
    tx: Option<Sender<CMD>>,
    duration: Arc<Mutex<f64>>, //Total duration in seconds, -1.0 if theres nothing to play
    total_samples: Arc<Mutex<Option<u64>>>, // Total samples in current track
    resampling_quality: ResamplingQuality,
    signal_receiver: Receiver<EngineSignal>,
    callback: Box<dyn FnMut(EngineSignal, Arc<Player>) -> ()>,
    decoder: Arc<Mutex<Decoder>>,
}

impl AudioEngine {
    pub fn new(
        
        resampling_quality: Option<ResamplingQuality>,
        callback: Box<dyn FnMut(EngineSignal, Arc<Player>) -> ()>,
    
    ) -> Result<Arc<async_Mutex<Self>>, i32> 
    
    {
        
        let m_resampling_quality = resampling_quality.unwrap_or(ResamplingQuality::High);
        singletons::set_decoder_busy(false);

        let host = cpal::default_host();
        let device = host.default_output_device()
            .expect("No output device available");
        let config = device.default_output_config()
            .expect("Failed to get default output config");

        let sample_rate = config.sample_rate() as i32;
        let channels = config.channels() as i32;
        
        let buffer_ptr =
            unsafe { sys::av_audio_fifo_alloc(sys::AVSampleFormat::AV_SAMPLE_FMT_S32, 2, 100) };
        let buffer = Arc::new(Mutex::new(AudioFifo(buffer_ptr)));

        let (signal_tx, signal_rx) = unbounded::<EngineSignal>();

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
            stream: Some(build_stream(&device, config.into(), buffer.clone(), signal_tx, sample_rate.clone()).unwrap()),
            buffer: buffer,
            channels: channels,
            sample_rate: Arc::new(Mutex::new(sample_rate)),
            state: Arc::new(Mutex::new(PlayerState::EMPTY)),
            initialised: false,
            tx: None,
            duration: Arc::new(Mutex::new(-1.0)),
            total_samples: Arc::new(Mutex::new(None)),
            resampling_quality: m_resampling_quality,
            signal_receiver: signal_rx,
            callback: callback,
            decoder: decoder,
        };

        Ok(Arc::new(async_Mutex::new(engine)))
    }

    pub fn get_duration(&self) -> f64 {
        *self.duration.lock().unwrap()
    }

    ///Loads files. Automatically stops previous playback if any. Requires an arc clone of the player object because it's later passed as context to media end callback
    pub async fn load(audio_engine: Arc<async_Mutex<Self>>, file: &str, player: Weak<Player>) -> Result<(), i32> {
        // Clear any existing playback first
        let mut engine = audio_engine.lock().await;
        engine.clear()?;

        // Initialize decoder thread if needed
        if !engine.initialised {
            let (tx, rx) = unbounded::<CMD>();
            engine.tx = Some(tx);
            _ = engine.spawn_decoder_thread(rx.clone());
            _ = AudioEngine::spawn_listening_thread(
                audio_engine.clone(),
                engine.signal_receiver.clone(),
                player
            );
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

    pub fn get_volume(&self) -> f32 {
        f_get_volume()
    }

    pub fn set_volume(&self, volume: f32) {
        f_set_volume(volume);
    }

    //Clears the audio buffer
    pub fn clear(&mut self) -> Result<(), i32> {
        // Stop playback if active
        if *self.state.lock().unwrap() == PlayerState::PLAYING {
            self.pause()?;
        }

        reset_played();

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

        //Check if we have enough samples for playback so it doesnt cause artifacting    
        let mut size = unsafe {
            sys::av_audio_fifo_size(self.buffer.lock().unwrap().0)
        };

        let sample_rate = {self.sample_rate.lock().unwrap().clone()};
        let minimum_samples = sample_rate * 5;

        while size <= minimum_samples && !get_decoder_eof(){
            let buffer = self.buffer.lock().unwrap().0;
            size = unsafe {sys::av_audio_fifo_size(buffer)};
            thread::sleep(Duration::from_millis(10));
        }
        

        if *self.state.lock().unwrap() != PlayerState::PLAYING {
            self.stream.as_ref().unwrap().play().map_err(|_| -1)?;
            *self.state.lock().unwrap() = PlayerState::PLAYING;
        }

        Ok(())
    }

    //Pauses playback
    pub fn pause(&mut self) -> Result<(), i32> {
        if *self.state.lock().unwrap() != PlayerState::PAUSED {
            self.stream.as_ref().unwrap().pause().map_err(|_| -1)?;
            *self.state.lock().unwrap() = PlayerState::PAUSED;
        }

        Ok(())
    }

    ///Spawns a thread to listen for any events that are triggered by the audio engine
    fn spawn_listening_thread(
        engine: Arc<async_Mutex<Self>>,
        receiver: Receiver<EngineSignal>,
        player: Weak<Player>
    ) -> Result<(), i32> {

        tokio::task::spawn_blocking(move || {
            let rt_handle = Handle::current();

            for signal in receiver {
                let maybe_player = player.upgrade();
                if maybe_player.is_none() { break; }
                let player_arc = maybe_player.unwrap();
                rt_handle.block_on(async {
                    match signal {
                        EngineSignal::MediaEnd => {
                            set_decoder_eof(false);
                            let mut m_engine = engine.lock().await;
                            _ = m_engine.pause();
                            _ = m_engine.clear();
                            println!("Player empty and ready. Executing callback");
                            (m_engine.callback)(EngineSignal::MediaEnd, player_arc);
                        },
                        EngineSignal::BufferLow => {
                            if !get_decoder_eof() {  
                                let m_engine = engine.lock().await;
                                if let Some(tx) = &m_engine.tx {
                                    _ = tx.send(CMD::FillBuffer);
                                }
                            }
                        }
                    }
                });
            }
        });


        Ok(())
    }

    pub fn seek(&mut self, time_s: f64) -> Result<(), i32> {

        loop {
            let state = self.state.lock().unwrap();

            if (*state == PlayerState::EMPTY) || (*state == PlayerState::LOADING) {
                println!("Invalid state");
                println!("State: {}", *state);
                thread::sleep(Duration::from_millis(5));
            } else {
                break;
            }
        }

        let is_paused = { 
            dbg!(self.state.lock().unwrap());
            *self.state.lock().unwrap() == PlayerState::PAUSED 
        };

        dbg!(is_paused);
        _ = self.pause();
        
        {
            let decoder = self.decoder.lock().unwrap();
            decoder
                .main_decoder_cancel_flag
                .store(true, Ordering::Relaxed);
        }

        _ = self.clear();

        {
            let mut decoder = self.decoder.lock().unwrap();
                        
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

            decoder
                .main_decoder_cancel_flag
                .store(false, Ordering::Relaxed);
        }

        let tx = self.tx.as_ref().unwrap().clone();
        set_decoder_eof(false);
        _ = tx.send(CMD::Resume);
        set_played((time_s * (*self.sample_rate.lock().unwrap() as f64)) as u64);
        
        if !is_paused {
            _ = self.play();
        }

        Ok(())
    }















    











    // <- DECODING LOGIC ->
    fn spawn_decoder_thread(&mut self, rx: Receiver<CMD>) -> Result<(), i32> {
        let sample_rate_handle = self.sample_rate.clone();
        let buffer_handle = self.buffer.clone();
        let duration_handle = self.duration.clone();
        let total_samples_handle = self.total_samples.clone();
        let state_handle = self.state.clone();

        let decoder_handle = self.decoder.clone();

        thread::spawn(move || {
            let target_buffer_size = (*sample_rate_handle.lock().unwrap() * 10) as i32; // 10 seconds buffered
            let low_water_mark = (*sample_rate_handle.lock().unwrap() * 5) as i32; // refill at 5 seconds

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
                    let silence: Vec<[i32; 2]> = vec![[0, 0]; (m_decoder.decoder.rate()) as usize]; 
                    let mut dummy_output: Vec<[i32; 2]> = vec![[0, 0]; (m_decoder.decoder.rate()) as usize];
                    _ = m_decoder.soxr_resampler.process(&silence, &mut dummy_output);

                    let mut state = state_handle.lock().unwrap();
                    *state = PlayerState::INITIALISED;
                    println!("Initialised decoders");
                    drop(m_decoder);

                    _ = decode(
                        decoder_handle.clone(),
                        sample_rate_handle.clone(),
                        buffer_handle.clone(),
                        target_buffer_size
                    );
                }
                else if let CMD::Resume = cmd {
                    _ = decode(decoder_handle.clone(), sample_rate_handle.clone(), buffer_handle.clone(), target_buffer_size);
                }
                else if let CMD::FillBuffer = cmd {

                    let current_size = unsafe { 
                        sys::av_audio_fifo_size(buffer_handle.lock().unwrap().0) 
                    };
                    
                    if current_size < low_water_mark && !get_decoder_eof() {
                        _ = decode(
                            decoder_handle.clone(),
                            sample_rate_handle.clone(),
                            buffer_handle.clone(),
                            target_buffer_size,
                        );
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
            if !self.buffer.lock().unwrap().0.is_null() {
                sys::av_audio_fifo_free(self.buffer.lock().unwrap().0);
            }
        }
    }
}

unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

fn build_stream(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        buffer: Arc<Mutex<AudioFifo>>,
        signal_tx: Sender<EngineSignal>,
        device_sample_rate: i32,
    ) -> Result<Stream, i32> {
        
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
                unsafe {
                    let buffer_guard = match buffer.lock() {
                        Ok(guard) => guard,
                        Err(_) => {
                            // Lock contention - zero fill
                            data.fill(0);
                            return;
                        }
                    };

                    let fifo = buffer_guard.0;
                    if fifo.is_null() {
                        data.fill(0);
                        return;
                    }

                    let available = sys::av_audio_fifo_size(fifo);
                    let frames_to_read = available.min(data.len() as i32 / 2); // 2 channels

                    if frames_to_read > 0 {
                        let mut data_ptrs = [data.as_mut_ptr() as *mut c_void];
                        let got = sys::av_audio_fifo_read(
                            fifo,
                            data_ptrs.as_mut_ptr(),
                            frames_to_read,
                        );

                        if got > 0 {
                            add_played(got as u64);

                            // Apply volume
                            let vol = f_get_volume();
                            if vol != 1.0 {
                                for sample in &mut data[..((got as usize) * 2)] {
                                    let s = *sample as f32;
                                    *sample = (s * vol).clamp(i32::MIN as f32, i32::MAX as f32) as i32;
                                }
                            }

                            // Zero fill remaining
                            if got < frames_to_read {
                                let start = (got as usize) * 2;
                                data[start..].fill(0);
                            }
                        } else {
                            data.fill(0);
                        }

                        // Check for EOF
                        if get_decoder_eof() && got < (device_sample_rate / 100) { //I dont know whats the logic behind this number, i just kept printing `got` until the audio ended
                            _ = signal_tx.try_send(EngineSignal::MediaEnd);
                        }

                        // Check for low buffer
                        if available < (config.sample_rate as i32 * 5) && !get_decoder_eof() {
                            _ = signal_tx.try_send(EngineSignal::BufferLow);
                        }
                    } else {
                        data.fill(0);
                    }
                }
            },
            |err| {
                eprintln!("Stream error: {}", err);
            },
            None,
        ).map_err(|_| -1)?;

        Ok(stream)
    }

