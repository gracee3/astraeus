//! Stable, validated reusable chart specifications.

use astraeus_core::{
    AspectDefinitions, CalculationOptions, CalculationRequest, GeographicLocation, UtcInstant,
};
use serde::{Deserialize, Serialize, Serializer};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;

/// Reusable calculation and aspect policy, independent of a subject or time.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartSpecification {
    calculation: CalculationOptions,
    aspects: AspectDefinitions,
}

#[derive(Serialize)]
struct SpecificationRef<'a> {
    schema_version: u32,
    calculation: &'a CalculationOptions,
    aspects: &'a AspectDefinitions,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecificationWire {
    schema_version: u32,
    calculation: CalculationOptions,
    aspects: AspectDefinitions,
}

#[derive(Debug, Error)]
pub enum SpecificationError {
    #[error("invalid chart specification JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported chart specification schema version {0}")]
    UnsupportedSchema(u32),
}

impl ChartSpecification {
    pub fn new(calculation: CalculationOptions, aspects: AspectDefinitions) -> Self {
        Self {
            calculation,
            aspects,
        }
    }

    pub fn from_json(input: &str) -> Result<Self, SpecificationError> {
        let wire: SpecificationWire = serde_json::from_str(input)?;
        if wire.schema_version != SCHEMA_VERSION {
            return Err(SpecificationError::UnsupportedSchema(wire.schema_version));
        }
        Ok(Self::new(wire.calculation, wire.aspects))
    }

    pub fn to_json(&self) -> Result<String, SpecificationError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_pretty_json(&self) -> Result<String, SpecificationError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn calculation(&self) -> &CalculationOptions {
        &self.calculation
    }

    pub fn aspects(&self) -> &AspectDefinitions {
        &self.aspects
    }

    pub fn request(&self, instant: UtcInstant, location: GeographicLocation) -> CalculationRequest {
        CalculationRequest::from_options(instant, location, self.calculation.clone())
    }
}

impl Serialize for ChartSpecification {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SpecificationRef {
            schema_version: SCHEMA_VERSION,
            calculation: &self.calculation,
            aspects: &self.aspects,
        }
        .serialize(serializer)
    }
}
