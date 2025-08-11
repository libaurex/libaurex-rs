fn main() {
    cc::Build::new()
        .file("src/miniaudio.c")
        .compile("miniaudio");
}