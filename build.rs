fn main() {
    cc::Build::new()
        .file("external/miniaudio_aurex/src/miniaudio.c")
        .compile("miniaudio");
}