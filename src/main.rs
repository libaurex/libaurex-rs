//i know it's janky. just for testing 

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::{env, thread};
use std::time::Duration;
use libaurex::aurex::Player;
use libaurex::enums::{ResamplingQuality, EngineSignal};
use std::fs;
use std::path::{Path, PathBuf};
use std::io;


fn get_all_paths(dir: &str, recursive: bool) -> io::Result<VecDeque<PathBuf>> {
    let audio_extensions = ["mp3", "wav", "flac", "ogg", "m4a", "aac", "wma", "opus"];
    
    let mut paths = VecDeque::new();
    let mut dirs_to_process = VecDeque::new();
    dirs_to_process.push_back(PathBuf::from(dir));
    
    while let Some(current_dir) = dirs_to_process.pop_front() {
        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path().canonicalize()?;
            
            if path.is_dir() && recursive {
                dirs_to_process.push_back(path);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    if audio_extensions.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                        paths.push_back(path);
                    }
                }
            }
        }
    }
    
    Ok(paths)
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("No files provided.");
        return;
    }

    let mut all_files: Arc<Mutex<Option<VecDeque<PathBuf>>>> = Arc::new(Mutex::new(None));

    if &args[1] == "--dir" {
        if args.len() < 3 {
            println!("No directory provided");
            return;
        }

        match get_all_paths(&args[2], args.contains(&String::from("-R"))) {
            Ok(mut files) => {
                all_files = Arc::new(Mutex::new(Some(files)));
            },
            Err(e) => {
                eprintln!("Error: {}", e)
            }
        }
    }

    let files = all_files.clone();

    let player = Player::new(
        Some(ResamplingQuality::VeryHigh),
        Box::new(move |_event, player_arc| {
            println!("Media Ended.");
            let player = player_arc.clone();
            let file = files.lock().unwrap().as_mut().and_then(|list| list.pop_front());

            tokio::spawn(async move {
                match file {
                    Some(file_path) => {
                        if let Some(path_str) = file_path.to_str() {
                            player.clone().load(path_str).await;
                            player.play().await;
                        }
                    },
                    None => {
                        std::process::exit(0);
                    }
                }
            });
        })
    ).unwrap();

    
    if &args[1] == "--dir" {
        let file = all_files.lock().unwrap().as_mut().and_then(|list| list.pop_front());

        player.clone().load(file.unwrap().to_str().unwrap()).await;
        player.play().await;
    } else {
        player.clone().load(&args[1]).await;
        player.play().await;
    }

    loop {
        println!("Progress: {}/{}", player.get_progress().await.unwrap(), player.get_duration().await);
        thread::sleep(Duration::from_secs(1));
    }
}
