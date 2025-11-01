# libaurex
libaurex is a cross-platform, low latency, high level audio API written in Rust.

# v0.1.3
## New Features
- The load() function now allows swapping files during playback.
- Added clear() to drain the buffer and clear player.

# Features
- Native backends for each platform for low latency audio playback via miniaudio.
    - WASAPI for Windows.
    - AAUDIO for Android.
    - ALSA for Linux.
    - CoreAudio for iOS and MacOS.

- Supports nearly every codec on the planet via a custom FFMpeg decoder.
- Best in class resampling that's damn near bit perfect with libsoxr.
- Simple as hell API.

# Documentation
- A simple example can be found in the main.rs file.

# Upcoming Features
- A full fledged media player API.
- Streaming support (both disk and HTTPS, right now it loads the entire file in memory, good enough for music but not for let's say, a 12 hour podacast).

# Contributing
Patches welcome. Bug reports too, but make them useful.
If you open an issue, include:
    
    - Your OS + version
    - Backend in use (WASAPI, CoreAudio, ALSA, etc.)
    - Exact sample rate / format / channels you were trying
    - Minimal reproducible code
    - What you expected vs what happened

If you’re sending a PR:

    - Keep code style consistent with the existing code (run cargo fmt)
    - Don’t break other platforms — test or at least stub them out
    - If adding a feature, document it and include a minimal example
    - Avoid adding massive dependencies unless there’s no sane alternative
    - Keep all ffi functions in the ffi module.

If you’re doing something that might involve breaking changes, open an issue first so it can be discussed before you sink hours into it.