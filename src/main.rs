//i know it's janky. just for testing 

use std::{env, thread};
use std::time::Duration;
use libaurex::aurex::Player;
use libaurex::enums::{ResamplingQuality, EngineSignal};


fn notify_end(event: EngineSignal) {
    if event == EngineSignal::MediaEnd {
        println!("Media has ended.")
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No files provided.");
        return;
    }
 
    let player = Player::new(Some(ResamplingQuality::VeryHigh), notify_end).unwrap();
    _ = player.load(&args[1].clone());
    _ = player.play();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
