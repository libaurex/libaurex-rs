fn main() {
    cc::Build::new()
        .file("external/miniaudio/src/miniaudio.c")
        .compile("miniaudio");
}