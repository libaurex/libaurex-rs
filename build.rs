use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FFMPEG_VERSION: &str = "7.1.1";
const FFMPEG_RELEASE_URL: &str = "https://github.com/libaurex/libaurex-rs/releases/download/dev/ffmpeg.zip";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "windows" {
        return;
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let vendor_dir = PathBuf::from(&manifest_dir).join("vendor");
    let ffmpeg_dir = vendor_dir.join("ffmpeg");
    let lib_dir = ffmpeg_dir.join("lib");
    let stamp = vendor_dir.join(format!(".ffmpeg-{}", FFMPEG_VERSION));

    if !stamp.exists() {
        eprintln!("FFmpeg libs missing. Grabbing them now...");
        
        if vendor_dir.exists() {
            let _ = fs::remove_dir_all(&vendor_dir);
        }
        fs::create_dir_all(&vendor_dir).unwrap();

        let zip_path = vendor_dir.join("ffmpeg.zip");
        download_file(FFMPEG_RELEASE_URL, &zip_path);
        extract_zip(&zip_path, &vendor_dir);
        
        let _ = fs::remove_file(&zip_path);

        // Sanity check: Did the extraction actually put files where we expect?
        let ffmpeg_libs = [
            "avcodec", "avdevice", "avfilter", "avformat", 
            "avutil", "swresample", "swscale"
        ];

        for lib in &ffmpeg_libs {
            let lib_file = lib_dir.join(format!("{}.lib", lib));
            if !lib_file.exists() {
                panic!(
                    "Verification failed: Expected library file not found at {}. Check your zip structure.",
                    lib_file.display()
                );
            }
        }

        fs::write(&stamp, FFMPEG_VERSION).expect("Failed to write stamp file");
        eprintln!("FFmpeg verification passed.");
    }

    println!("cargo:rustc-env=FFMPEG_DIR={}", ffmpeg_dir.display());
    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    for lib in &[
        "avcodec", "avdevice", "avfilter", "avformat", 
        "avutil", "swresample", "swscale"
    ] {
        println!("cargo:rustc-link-lib=static={}", lib);
    }

    let system_libs = [
        "bcrypt", "ws2_32", "secur32", "user32", "ole32", "crypt32", "gdi32",
        "d3d11", "d3d12", "dxva2", "d3dcompiler", "dxguid", "mfplat", "mfuuid",
        "mf", "mfreadwrite", "strmiids", "uuid", "oleaut32",
    ];
    for lib in system_libs {
        println!("cargo:rustc-link-lib={}", lib);
    }
}

fn download_file(url: &str, dest: &Path) {
    let cmd = format!("Invoke-WebRequest -Uri '{}' -OutFile '{}'", url, dest.display());
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .status()
        .expect("Powershell failed to start. Seriously?");

    if !status.success() {
        panic!("Failed to download FFmpeg. Check your internet or the URL.");
    }
}

fn extract_zip(zip_path: &Path, dest: &Path) {
    let cmd = format!(
        "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
        zip_path.display(),
        dest.display()
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .status()
        .expect("Powershell failed during extraction.");

    if !status.success() {
        panic!("Extraction failed. The zip might be a lie.");
    }
}