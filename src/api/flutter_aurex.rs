#![allow(unexpected_cfgs)]

use crate::{
    aurex::{Player as InternalPlayer, PlayerCallback as InternalCallback},
    enums::{ResamplingQuality, EngineSignal, PlayerError},
};
use std::sync::{Arc};
use flutter_rust_bridge::frb;



// The opaque wrapper that Flutter sees
#[frb(opaque)]
pub struct Player {
    internal: Arc<InternalPlayer>,
}

// Adapter: Flutter callback -> Internal callback
struct CallbackAdapter {
    flutter_callback: Box<dyn FlutterPlayerCallback>,
}

impl InternalCallback for CallbackAdapter {
    fn on_player_event(&self, event: EngineSignal, player: Arc<InternalPlayer>) {
        // Forward to Flutter callback without the player
        self.flutter_callback.on_player_event(event);
    }
}

// The Flutter-facing callback trait (simplified)
pub trait FlutterPlayerCallback: Send + Sync {
    fn on_player_event(&self, event: EngineSignal);
}

impl Player {
    pub async fn new(
        resampling_quality: Option<ResamplingQuality>,
        callback: Box<dyn FlutterPlayerCallback>
    ) -> Result<Self, PlayerError> {
        // Create the adapter
        let adapter = Box::new(CallbackAdapter {
            flutter_callback: callback,
        });
        
        // Use your existing constructor
        let internal = InternalPlayer::new(resampling_quality, adapter).await?;
        
        Ok(Player { internal })
    }

    pub async fn get_duration(&self) -> f64 {
        self.internal.get_duration().await
    }

    pub async fn load(&self, file: String) -> Result<(), PlayerError> {
        self.internal.clone().load(&file).await
    }

    pub async fn get_progress(&self) -> Result<f64, PlayerError> {
        self.internal.get_progress().await
    }

    pub async fn clear(&self) -> Result<(), PlayerError> {
        self.internal.clear().await
    }

    pub async fn play(&self) -> Result<(), PlayerError> {
        self.internal.play().await
    }

    pub async fn pause(&self) -> Result<(), PlayerError> {
        self.internal.pause().await
    }

    pub async fn seek(&self, time_s: f64) -> Result<(), PlayerError> {
        self.internal.seek(time_s).await
    }

    pub async fn get_volume(&self) -> f32 {
        self.internal.get_volume().await
    }

    pub async fn set_volume(&self, volume: f32) {
        self.internal.set_volume(volume).await
    }
}