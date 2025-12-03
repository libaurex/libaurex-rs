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

#[derive(uniffi::Object)]
pub struct Player {
    engine: Arc<Mutex<AudioEngine>>
}

#[uniffi::export(callback_interface)]
pub trait PlayerCallback: Send + Sync {
    fn on_player_event(&self, event: EngineSignal);
}

#[uniffi::export]
impl Player {

    #[uniffi::constructor]
    pub fn new (

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

    pub fn get_duration(&self) -> f64 {
        let engine = self.engine.lock().unwrap();
        engine.get_duration()
    }

    pub fn load(&self, file: &str) -> Result<(), PlayerError> {
        AudioEngine::load(self.engine.clone(), file);
        Ok(())
    }

    pub fn get_progress(&self) -> Result<f64, PlayerError> {
        let engine = self.engine.lock().unwrap();
        let res = engine.get_progress();
        if res.is_err() {
            return Err(PlayerError::Code(res.err().unwrap_or(-1)));
        }
        Ok(res.unwrap())
    }

    pub fn clear(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().unwrap();
        engine.clear();
        Ok(())
    }

    pub fn play(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().unwrap();
        engine.play();
        Ok(())
    }

    pub fn pause(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().unwrap();
        engine.pause();
        Ok(())
    }
}