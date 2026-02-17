use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};
use std::collections::VecDeque;
use crate::aurex::{Player, PlayerCallback};
use crate::enums::{ResamplingQuality, EngineSignal, PlayerError};

// === GLOBAL STATE ===
static PLAYER: OnceLock<Arc<Player>> = OnceLock::new();
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

// Simple event queue - Dart polls this
static EVENT_QUEUE: Mutex<VecDeque<i32>> = Mutex::new(VecDeque::new());

// === CALLBACK ADAPTER ===
struct FFICallback;

impl PlayerCallback for FFICallback {
    fn on_player_event(&self, event: EngineSignal, _player: Arc<Player>) {
        let event_code: i32;
        if event == EngineSignal::MediaEnd {
            event_code = 0;
        } else {
            event_code = -1;
        }
        // Just push to queue, Dart will poll it
        EVENT_QUEUE.lock().unwrap().push_back(event_code);
    }
}

// === FFI FUNCTIONS ===

#[unsafe(no_mangle)]
pub extern "C" fn player_new(resampling_quality: i32) -> i32 {
    let rt = RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().unwrap()
    });

    rt.block_on(async {
        let quality = match resampling_quality {
            0 => Some(ResamplingQuality::Low),
            1 => Some(ResamplingQuality::Medium),
            2 => Some(ResamplingQuality::High),
            3 => Some(ResamplingQuality::VeryHigh),
            _ => None,
        };

        let ffi_callback = Box::new(FFICallback);

        match Player::create(quality, ffi_callback).await {
            Ok(player) => {
                PLAYER.set(player).ok();
                0
            }
            Err(PlayerError::Code(c)) => c,
        }
    })
}

// Poll for events - returns -1 if no events, otherwise returns event code
#[unsafe(no_mangle)]
pub extern "C" fn player_poll_event() -> i32 {
    EVENT_QUEUE.lock().unwrap().pop_front().unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn player_load(file_path: *const c_char) -> i32 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1,
    };

    let path = unsafe {
        match CStr::from_ptr(file_path).to_str() {
            Ok(s) => s,
            Err(_) => return -2,
        }
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        match player.clone().load(path).await {
            Ok(_) => 0,
            Err(PlayerError::Code(c)) => c,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_play() -> i32 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        match player.play().await {
            Ok(_) => 0,
            Err(PlayerError::Code(c)) => c,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_pause() -> i32 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        match player.pause().await {
            Ok(_) => 0,
            Err(PlayerError::Code(c)) => c,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_seek(time_s: f64) -> i32 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        match player.seek(time_s).await {
            Ok(_) => 0,
            Err(PlayerError::Code(c)) => c,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_clear() -> i32 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        match player.clear().await {
            Ok(_) => 0,
            Err(PlayerError::Code(c)) => c,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_get_duration() -> f64 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1.0,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        player.get_duration().await
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_get_progress() -> f64 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return -1.0,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        match player.get_progress().await {
            Ok(v) => v,
            Err(_) => -1.0,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_get_volume() -> f32 {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return 0.0,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        player.get_volume().await
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn player_set_volume(volume: f32) {
    let player = match PLAYER.get() {
        Some(p) => p,
        None => return,
    };

    let rt = RUNTIME.get().unwrap();
    rt.block_on(async {
        player.set_volume(volume).await
    });
}