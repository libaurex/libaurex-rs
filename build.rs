fn main() {
    #[cfg(target_os = "windows")]
    {
        let system_libs = [
            "bcrypt", "ws2_32", "secur32", "user32", "ole32", "crypt32", "gdi32",
            "d3d11", "d3d12", "dxva2", "d3dcompiler", "dxguid", "mfplat", "mfuuid",
            "mf", "mfreadwrite", "strmiids", "uuid", "oleaut32",
        ];
        for lib in system_libs {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }
}
