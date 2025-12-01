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
pub enum EngineSignal {
    MediaEnd
}

pub enum CMD {
    Start(String, ResamplingQuality),
}

#[derive(Clone, Copy)]
pub enum ResamplingQuality {
    Quick = 0,
    Low,
    Medium,
    High,
    VeryHigh
}

impl ResamplingQuality {
    pub fn get_quality_spec(&self) -> Result<QualitySpec, i32> {
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
