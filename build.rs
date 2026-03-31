use std::env;
use std::path::PathBuf;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let ffmpeg_dir = PathBuf::from(&manifest_dir).join("vendor").join("ffmpeg");
        let lib_dir = ffmpeg_dir.join("lib");

        println!("cargo:rustc-env=FFMPEG_DIR={}", ffmpeg_dir.display());
        println!("cargo:rustc-env=FFMPEG_STATIC=1");
        println!("cargo:rustc-env=FFMPEG_NO_PKG_CONFIG=1");
        println!("cargo:rustc-link-search=native={}", lib_dir.display());

        let ffmpeg_libs = [
            "avcodec",
            "avdevice",
            "avfilter",
            "avformat",
            "avutil",
            "swresample",
            "swscale",
        ];

        for lib in &ffmpeg_libs {
            println!("cargo:rustc-link-lib=static={}", lib);
        }

        // Core Windows deps
        println!("cargo:rustc-link-lib=bcrypt");
        println!("cargo:rustc-link-lib=ws2_32");
        println!("cargo:rustc-link-lib=secur32");
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=ole32");

        // schannel (--enable-schannel)
        println!("cargo:rustc-link-lib=crypt32");

        // d3d11va, d3d12va, dxva2
        println!("cargo:rustc-link-lib=d3d11");
        println!("cargo:rustc-link-lib=d3d12");
        println!("cargo:rustc-link-lib=dxva2");
        println!("cargo:rustc-link-lib=d3dcompiler");
        println!("cargo:rustc-link-lib=dxguid");

        // Media Foundation (--enable-mediafoundation)
        println!("cargo:rustc-link-lib=mfplat");
        println!("cargo:rustc-link-lib=mfuuid");
        println!("cargo:rustc-link-lib=mf");
        println!("cargo:rustc-link-lib=mfreadwrite");

        // avdevice needs these
        println!("cargo:rustc-link-lib=strmiids");
        println!("cargo:rustc-link-lib=uuid");
        println!("cargo:rustc-link-lib=oleaut32");
        println!("cargo:rustc-link-lib=gdi32");
    }
}