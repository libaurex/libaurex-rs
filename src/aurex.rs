//This is an ffi safe public api wrapper

use crate::{
    engine::{AudioEngine},
    enums::ResamplingQuality
};

use std::{
    sync::{Arc, Mutex}
};

pub struct Player {
    engine: Arc<Mutex<AudioEngine>>
}

impl Player {

    pub fn new (
        resampling_quality: Option<ResamplingQuality>
    ) -> Result<Self, i32> {

        let engine = AudioEngine::new(resampling_quality);
        if engine.is_err() {
            return Err(engine.err().unwrap_or(-1));
        }

        Ok(
            Player { engine: engine.unwrap() }
        )
    }

    pub fn get_duration(&self) -> f64 {
        let engine = self.engine.lock().unwrap();
        engine.get_duration()
    }

    pub fn load(&self, file: &str) -> Result<(), i32> {
        AudioEngine::load(self.engine.clone(), file)
    }

    pub fn get_progress(&self) -> Result<f64, i32> {
        let engine = self.engine.lock().unwrap();
        engine.get_progress()
    }

    pub fn clear(&self) -> Result<(), i32> {
        let mut engine = self.engine.lock().unwrap();
        engine.clear()
    }

    pub fn play(&self) -> Result<(), i32> {
        let mut engine = self.engine.lock().unwrap();
        engine.play()
    }

    pub fn pause(&self) -> Result<(), i32> {
        let mut engine = self.engine.lock().unwrap();
        engine.pause()
    }
}