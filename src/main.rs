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
    _ = engine.load(&args[1].clone());
    _ = engine.play();


    loop {
        thread::sleep(Duration::from_secs(1));
        println!("Progress: {}/{} seconds", engine.get_progress().unwrap(), engine.get_duration());
    }
}
