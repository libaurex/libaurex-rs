use ffmpeg_next::format::context::input::Input;
use ffmpeg_next::codec::decoder::audio::Audio;
use ffmpeg_next::software::resampling::context::Context as Resampler;

use soxr::format::Interleaved;
use soxr::Soxr;

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct Decoder {
    pub format_ctx: Option<Input>,
    pub decoder: Audio,
    pub resampler: Resampler,
    pub soxr_resampler: Soxr<Interleaved<i32, 2>>,
    pub audio_stream_index: usize,
    pub main_decoder_cancel_flag: Arc<AtomicBool>
}

unsafe impl Send for Decoder {}
unsafe impl Sync for Decoder {}
