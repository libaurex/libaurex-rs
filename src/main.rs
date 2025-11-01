//i know it's janky. just for testing 

use std::{env, thread};
use std::time::Duration;
use libaurex::engine::AudioEngine;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No files provided.");
        return;
    }
 
    let mut engine = AudioEngine::new().unwrap();
    engine.load(&args[1].clone());
    engine.play();

    thread::sleep(Duration::from_secs(10));

    engine.load(&args[2].clone());
    engine.play();

    thread::sleep(Duration::from_secs(10));

    engine.load(&args[3].clone());
    engine.play();

    loop {
        thread::sleep(Duration::from_secs(1));
    }
    // _ = engine_old::play_audio(&args[1]);
}
