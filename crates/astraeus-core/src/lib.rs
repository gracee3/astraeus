//! Validated, provider-independent astrology calculation contracts.
//!
//! This crate intentionally contains no native ephemeris implementation. An
//! adapter must either return every requested object or fail the calculation.

mod adapter;
mod aspects;
mod error;
mod types;

pub use adapter::{DeterministicMock, EphemerisAdapter};
pub use aspects::{
    ASPECT_EXACT_TOLERANCE_DEGREES, ASPECT_STATION_TOLERANCE_DEGREES_PER_DAY, Aspect,
    AspectDefinition, AspectDefinitions, AspectKind, AspectPhase, calculate_aspects,
};
pub use error::{CalculationError, ValidationError};
pub use types::{
    Ayanamsa, CalculationOptions, CalculationProvenance, CalculationRequest, CalculationResult,
    CelestialObject, EphemerisSource, GeographicLocation, HouseCusps, HouseSystem, Position,
    UtcInstant, Zodiac,
};
