use std::{env, time::Instant};

use libaurex::engine::AudioEngine;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No file provided.");
        return;
    }
 
    let mut engine = AudioEngine::new().unwrap();
    engine.load(&args[1].clone());
    engine.play();
    
    loop {
        
    }
    // _ = engine_old::play_audio(&args[1]);
}
