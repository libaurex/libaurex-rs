//i know it's janky. just for testing 

use std::{env, thread};
use std::time::Duration;
use libaurex::aurex::Player;
use libaurex::aurex::PlayerCallback;
use libaurex::enums::{ResamplingQuality, EngineSignal};


struct Callback;
impl PlayerCallback for Callback {
    fn on_player_event(&self, event:EngineSignal) {
        println!("Media Ended.")
    }
}


fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No files provided.");
        return;
    }
 
    let player = Player::new(Some(ResamplingQuality::VeryHigh), 
        Box::new(Callback)
    ).unwrap();
    _ = player.load(&args[1].clone());
    _ = player.play();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
