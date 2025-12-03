fn main() {
    cc::Build::new()
        .file("miniaudio.c")
        .compile("miniaudio");

}