//This is an ffi safe public api wrapper
use crate::{
    engine::{AudioEngine},
    enums::ResamplingQuality,
    enums::EngineSignal,
    enums::PlayerError
};

use std::{
    sync::{Arc, Mutex}
};

use tokio::sync::Mutex as async_Mutex;

#[derive(uniffi::Object)]
pub struct Player {
    engine: Arc<async_Mutex<AudioEngine>>
}

#[uniffi::export(callback_interface)]
pub trait PlayerCallback: Send + Sync {
    fn on_player_event(&self, event: EngineSignal);
}

#[uniffi::export]
impl Player {

    #[uniffi::constructor]
    pub async fn new (

        resampling_quality: Option<ResamplingQuality>,
        callback: Box<dyn PlayerCallback>

    ) -> Result<Self, PlayerError> {

        let engine = AudioEngine::new(resampling_quality, callback);
        if engine.is_err() {
            return Err(PlayerError::Code(engine.err().unwrap_or(-1)));
        }

        Ok(
            Player { engine: engine.unwrap() }
        )
    }

    pub async fn get_duration(&self) -> f64 {
        let engine = self.engine.lock().await;
        engine.get_duration()
    }

    pub async fn load(&self, file: &str) -> Result<(), PlayerError> {
        AudioEngine::load(self.engine.clone(), file).await;
        Ok(())
    }

    pub async fn get_progress(&self) -> Result<f64, PlayerError> {
        let engine = self.engine.lock().await;
        let res = engine.get_progress();
        if res.is_err() {
            return Err(PlayerError::Code(res.err().unwrap_or(-1)));
        }
        Ok(res.unwrap())
    }

    pub async fn clear(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        engine.clear();
        Ok(())
    }

    pub async fn play(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        engine.play();
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        engine.pause();
        Ok(())
    }

    pub async fn seek(&self, time_s: f64) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        engine.seek(time_s);

        Ok(())
    }
}