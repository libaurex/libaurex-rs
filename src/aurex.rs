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

//Tech debt here. It does not need to be async but once I had a function that needed async but then I figured out a simpler way. Now I'm leaving it as it is

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

    pub async fn get_duration(&self) -> f64 {
        let engine = self.engine.lock().await;
        engine.get_duration()
    }

    ///An Arc clone is needed to call this function cause the player object is also passed as context to the callback
    pub async fn load(self: Arc<Self>, file: &str) -> Result<(), PlayerError> {
        let player_clone = Arc::clone(&self);
        _ = AudioEngine::load(self.engine.clone(), file, Arc::downgrade(&player_clone)).await;
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
        _ = engine.clear();
        Ok(())
    }

    pub async fn play(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.play();
        Ok(())
    }

    pub async fn pause(&self) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.pause();
        Ok(())
    }

    pub async fn seek(&self, time_s: f64) -> Result<(), PlayerError> {
        let mut engine = self.engine.lock().await;
        _ = engine.seek(time_s);

        Ok(())
    }

    pub async fn get_volume(&self) -> f32 {
        let engine = self.engine.lock().await;
        engine.get_volume()
    }

    pub async fn set_volume(&self, volume: f32) {
        let engine = self.engine.lock().await;
        let mut m_volume = volume;
        if m_volume > 1.0 {m_volume = 1.0;}
        engine.set_volume(m_volume);
    }
}


///Rust only impl block
impl Player {
    pub async fn set_callback_context<T: Any>(&self, data: T) {
        let mut engine = self.engine.lock().await;
        engine.set_callback_context(data);
    }

    ///Function to execute callback logic if previously set
    /// T = Type of the context
    /// F = Function
    /// R = The return type
    ///Example:
    /// ```
    /// let file = player.with_callback_ctx_mut::<VecDeque<PathBuf>, _, PathBuf>(|files| {
    ///     files.pop_front().expect("Failed to get next path")
    /// }).await;
    /// ```
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