//This is an ffi safe public api wrapper
use crate::{
    engine::{AudioEngine},
    enums::ResamplingQuality,
    enums::EngineSignal,
    enums::PlayerError
};

use std::{
    any::Any, sync::Arc
};

use tokio::sync::Mutex as async_Mutex;

#[derive(uniffi::Object)]
pub struct Player {
    engine: Arc<async_Mutex<AudioEngine>>
}

#[uniffi::export(callback_interface)]
pub trait PlayerCallback: Send + Sync {
    fn on_player_event(&self, event: EngineSignal, player: Arc<Player>);
}

#[uniffi::export]
impl Player {

    #[uniffi::constructor]
    pub async fn new (

        resampling_quality: Option<ResamplingQuality>,
        callback: Box<dyn PlayerCallback>

    ) -> Result<Arc<Self>, PlayerError> {

        let engine = AudioEngine::new(resampling_quality, callback);
        if engine.is_err() {
            return Err(PlayerError::Code(engine.err().unwrap_or(-1)));
        }

        Ok(
            Arc::new(Player { engine: engine.unwrap() })
        )
    }

    pub async fn get_duration(self: Arc<Self>) -> f64 {
        let engine = self.engine.lock().await;
        engine.get_duration()
    }

    pub async fn load(self: Arc<Self>, file: &str) -> Result<(), PlayerError> {
        let player_clone = Arc::clone(&self);
        _ = AudioEngine::load(self.engine.clone(), file, Arc::downgrade(&player_clone)).await;
        Ok(())
    }

    pub async fn get_progress(self: Arc<Self>) -> Result<f64, PlayerError> {
        let engine = self.engine.lock().await;
        let res = engine.get_progress();
        if res.is_err() {
            return Err(PlayerError::Code(res.err().unwrap_or(-1)));
        }
        Ok(res.unwrap())
    }

    pub async fn clear(self: Arc<Self>) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.clear();
        Ok(())
    }

    pub async fn play(self: Arc<Self>) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.play();
        Ok(())
    }

    pub async fn pause(self: Arc<Self>) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.pause();
        Ok(())
    }

    pub async fn seek(self: Arc<Self>, time_s: f64) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.seek(time_s);

        Ok(())
    }

    pub async fn get_volume(self: Arc<Self>) -> f32 {
        let engine = self.engine.lock().await;
        engine.get_volume()
    }

    pub async fn set_volume(self: Arc<Self>, volume: f32) {
        let engine = self.engine.lock().await;
        let mut m_volume = volume;
        if m_volume > 1.0 {m_volume = 1.0;}
        engine.set_volume(m_volume);
    }
}

impl Player {
    pub async fn set_callback_context<T: Any>(self: Arc<Self>, data: T) {
        let mut engine = self.engine.lock().await;
        engine.set_callback_context(data);
    }

    pub async fn with_callback_ctx_mut<T, F, R>(&self, f: F) -> Option<R>
    where
        T: Any,
        F: FnOnce(&mut T) -> R, 
    {
        let mut engine = self.engine.lock().await;
        let context = engine.get_callback_context::<T>()?;
        let result = f(context);
        Some(result)
    }
}