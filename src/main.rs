use std::env;

use libaurex::engine::play_audio;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No file provided.");
        return;
    }
 
    let exit_code = play_audio(&args[1]); 
    println!("Process exited with code {}", exit_code);
    std::process::exit(exit_code);
}
