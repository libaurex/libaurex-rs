//i know it's janky. just for testing 

use std::collections::VecDeque;
use std::sync::{Arc, Weak};
use std::{env, thread};
use std::time::Duration;
use libaurex::aurex::Player;
use libaurex::aurex::PlayerCallback;
use libaurex::enums::{ResamplingQuality, EngineSignal};
use std::process;
use std::fs;
use std::path::PathBuf;
use std::io;

struct Callback;
impl PlayerCallback for Callback {
    fn on_player_event(&self, _event:EngineSignal, player: Arc<Player>) {
        println!("Media Ended.");

        let player = player.clone();
        tokio::spawn(async move {
            let file = player.with_callback_ctx_mut::<VecDeque<PathBuf>, _, PathBuf>(|files| {
                files.pop_front().expect("Failed to get next path")
            }).await;

            match file {
                Some(m_file) => {
                    let file_str = m_file.to_str().expect("Failed to get next path");
                    player.clone().load(file_str).await;
                    player.play().await;
                },
                None => {
                    process::exit(0)
                }
            }
        });

        
    }
}

fn get_all_paths(dir: &str) -> io::Result<VecDeque<PathBuf>> {
    fs::read_dir(dir)?
        .map(|res| res.map(|e| e.path()))
        .map(|res| res.and_then(|p| p.canonicalize())) 
        .collect()
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

    if &args[1] == "--dir" {
        if (args.len() < 3) {
            println!("No directory provided");
            return;
        }

        match get_all_paths(&args[2]) {
            Ok(mut files) => {
                player.clone().load(files.pop_front().unwrap().to_str().unwrap()).await;
                player.clone().play().await;
                player.clone().set_callback_context(files).await;
            },
            Err(e) => {
                eprintln!("Error: {}", e)
            }
        }
    } else {
        player.clone().set_callback_context(Vec::<PathBuf>::new()).await;
    
        _ = player.clone().load(&args[1].clone()).await;
        // player.set_volume(0.01).await;
        // _ = player.clone().seek(30.0).await;
        _ = player.play().await;
        // thread::sleep(Duration::from_millis(100));
    }


    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
