use std::fmt;
use soxr::{Soxr, params::{QualitySpec, QualityRecipe, QualityFlags}};

#[derive(PartialEq)]
pub enum PlayerState {
    LOADING = 0,
    LOADED,
    PLAYING,
    PAUSED,
    EMPTY,
}

#[derive(PartialEq)]
#[derive(uniffi::Enum)]
pub enum EngineSignal {
    MediaEnd
}

pub enum CMD {
    Start(String, ResamplingQuality),
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


#[derive(Clone, Copy)]
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
