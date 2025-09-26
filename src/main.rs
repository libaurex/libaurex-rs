use std::{env, time::Instant};
use std::thread;
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

    // thread::sleep(Duration::from_secs(10));


    // std::mem::drop(engine);
    // println!("Changing");
    // engine = AudioEngine::new().unwrap();
    // engine.load(&args[2].clone());
    // engine.play();

    loop {
        
    }
    // _ = engine_old::play_audio(&args[1]);
}
