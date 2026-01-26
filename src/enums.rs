use std::fmt;
use soxr::{params::{QualitySpec, QualityRecipe, QualityFlags}};
use tokio::sync::oneshot;

#[derive(PartialEq, Debug)]
pub enum PlayerState {
    LOADING = 0,
    LOADED,
    PLAYING,
    PAUSED,
    EMPTY,
    INITIALISED
}

impl fmt::Display for PlayerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PlayerState::LOADING => "loading",
            PlayerState::LOADED => "loaded",
            PlayerState::PLAYING => "playing",
            PlayerState::PAUSED => "paused",
            PlayerState::EMPTY => "empty",
            PlayerState::INITIALISED => "initialised",
        };
        write!(f, "{}", s)
    }
}

#[derive(PartialEq)]
#[derive(uniffi::Enum)]
pub enum EngineSignal {
    MediaEnd,
    BufferLow
}
#[derive(PartialEq)]
pub enum CMD {
    Start(String, ResamplingQuality),
    Resume,
    FillBuffer
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
