//i know it's janky. just for testing 

use std::{env, thread};
use std::time::Duration;
use libaurex::aurex::Player;
use libaurex::aurex::PlayerCallback;
use libaurex::enums::{ResamplingQuality, EngineSignal};


struct Callback;
impl PlayerCallback for Callback {
    fn on_player_event(&self, _event:EngineSignal) {
        println!("Media Ended.")
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No files provided.");
        return;
    }
 
    let player = Player::new(Some(ResamplingQuality::VeryHigh), 
        Box::new(Callback)
    ).await.unwrap();
    _ = player.load(&args[1].clone()).await;
    _ = player.seek(30.0).await;
    _ = player.play().await;
    // thread::sleep(Duration::from_millis(100));
    

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
