use std::fmt;
use soxr::{params::{QualitySpec, QualityRecipe, QualityFlags}};
use tokio::sync::oneshot;

#[derive(PartialEq)]
pub enum PlayerState {
    LOADING = 0,
    LOADED,
    PLAYING,
    PAUSED,
    EMPTY,
    INITIALISED
}

#[derive(PartialEq)]
#[derive(uniffi::Enum)]
pub enum EngineSignal {
    MediaEnd
}

pub enum CMD {
    Start(String, ResamplingQuality),
    Seek {
        time_s: f64,
        done: oneshot::Sender<()>
    },
    Resume
}

impl PartialEq for CMD {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            // Compare Start(String, ResamplingQuality)
            (
                CMD::Start(a_url, a_quality),
                CMD::Start(b_url, b_quality)
            ) => a_url == b_url && a_quality == b_quality,

            // Compare Seek { time_s, ... }
            (
                CMD::Seek { time_s: a, .. },
                CMD::Seek { time_s: b, .. }
            ) => a == b,

            //Compare resume
            (
                CMD::Resume,
                CMD::Resume
            ) => true,

            // Anything else isn't equal
            _ => false,
        }
    }
}

#[derive(uniffi::Error, Debug)]
pub enum PlayerError {
    Code(i32),
}


impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlayerError::Code(c) => write!(f, "PlayerError code {}", c),
        }
    }
}

impl std::error::Error for PlayerError {}


#[derive(Clone, Copy,PartialEq)]
#[derive(uniffi::Enum)]
pub enum ResamplingQuality {
    Quick = 0,
    Low,
    Medium,
    High,
    VeryHigh
}

impl ResamplingQuality {
    pub fn get_quality_spec(&self) -> Result<QualitySpec, PlayerError> {
        let mut recipe = QualityRecipe::default();
        match self {
            Self::Quick => {
                recipe = QualityRecipe::Quick;
            },
            Self::Low => {
                recipe = QualityRecipe::Low;
            },
            Self::Medium => {
                recipe = QualityRecipe::Medium;
            },
            Self::High => {
                recipe = QualityRecipe::high();
            },
            Self::VeryHigh => {
                recipe = QualityRecipe::very_high();
            }
        }

        Ok( QualitySpec::configure(
                recipe, 
                soxr::params::Rolloff::Small,
                QualityFlags::HighPrecisionClock | QualityFlags::DoublePrecision
        ))
    }
}
