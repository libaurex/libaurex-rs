use std::sync::{LazyLock, atomic::{AtomicU64, Ordering}};

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