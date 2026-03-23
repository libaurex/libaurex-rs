use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let ffmpeg_dir = PathBuf::from(&manifest_dir).join("vendor").join("ffmpeg");
        
        let lib_dir = ffmpeg_dir.join("lib");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-env=FFMPEG_DIR={}", ffmpeg_dir.display());

        let bin_dir = ffmpeg_dir.join("bin");
        let out_dir = env::var("OUT_DIR").unwrap();
        
        let target_dir = find_target_dir(Path::new(&out_dir));

        if let Ok(entries) = fs::read_dir(bin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("dll") {
                    let dest = target_dir.join(path.file_name().unwrap());
                    fs::copy(&path, &dest).ok(); 
                }
            }
        }
    }
}

fn find_target_dir(path: &Path) -> PathBuf {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if current.file_name().and_then(|s| s.to_str()) == Some("build") {
            return parent.to_path_buf();
        }
        current = parent;
    }
    path.to_path_buf()
}