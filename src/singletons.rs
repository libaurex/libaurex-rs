use std::sync::{
    LazyLock, 
    atomic::{AtomicU64, Ordering, AtomicBool},
};

//Global counter for number of played samples. Used for progress tracking
pub static PLAYED_SAMPLES: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

pub fn reset_played (){
    PLAYED_SAMPLES.store(0, Ordering::Relaxed);
}

pub fn add_played(samples: u64) {
    PLAYED_SAMPLES.fetch_add(samples, Ordering::Relaxed);
}

pub fn get_played() -> u64 {
    PLAYED_SAMPLES.load(Ordering::Relaxed)
}


// Counter for Total Samples
pub static TOTAL_SAMPLES: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(0));

pub fn set_total(samples: u64) {
    TOTAL_SAMPLES.store(samples, Ordering::Relaxed);
}

pub fn get_total() -> u64 {
    TOTAL_SAMPLES.load(Ordering::Relaxed)
}

//Flag for decoder EOF
pub static DECODER_EOF: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
pub fn get_decoder_eof() -> bool {
    DECODER_EOF.load(Ordering::Relaxed)
}
pub fn set_decoder_eof(flag: bool) {
    DECODER_EOF.store(flag, Ordering::Relaxed);
}
