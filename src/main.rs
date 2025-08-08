use std::env;

use crate::ffmpeg_test::{play_audio_ffmpeg};
mod ffmpeg_test;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No file provided.");
        return;
    }
 
    let exit_code = play_audio_ffmpeg(&args[1]); 
    println!("Process exited with code {}", exit_code);
    std::process::exit(exit_code);
}
