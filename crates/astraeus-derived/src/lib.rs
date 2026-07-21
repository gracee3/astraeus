//! Validated, content-addressed derived chart artifacts.

use astraeus_artifacts::CalculationArtifact;
use astraeus_core::{Aspect, calculate_aspects};
use astraeus_specifications::ChartSpecification;
use serde::{Deserialize, Serialize, Serializer};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedChartArtifact {
    calculation: CalculationArtifact,
    specification: ChartSpecification,
    aspects: Vec<Aspect>,
}

#[derive(Serialize)]
struct DerivedRef<'a> {
    schema_version: u32,
    calculation: &'a CalculationArtifact,
    specification: &'a ChartSpecification,
    aspects: &'a [Aspect],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DerivedWire {
    schema_version: u32,
    calculation: CalculationArtifact,
    specification: ChartSpecification,
    aspects: Vec<Aspect>,
}

#[derive(Debug, Error)]
pub enum DerivedArtifactError {
    #[error("invalid derived chart artifact JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported derived chart artifact schema version {0}")]
    UnsupportedSchema(u32),
    #[error("chart specification calculation options do not match the calculation request")]
    CalculationPolicyMismatch,
    #[error("serialized aspects do not match aspects derived from the calculation")]
    AspectMismatch,
}

impl DerivedChartArtifact {
    pub fn new(
        calculation: CalculationArtifact,
        specification: ChartSpecification,
    ) -> Result<Self, DerivedArtifactError> {
        if calculation.request().options() != *specification.calculation() {
            return Err(DerivedArtifactError::CalculationPolicyMismatch);
        }
        let aspects = calculate_aspects(calculation.result().positions(), specification.aspects());
        Ok(Self {
            calculation,
            specification,
            aspects,
        })
    }

    pub fn from_json(input: &str) -> Result<Self, DerivedArtifactError> {
        let wire: DerivedWire = serde_json::from_str(input)?;
        Self::from_wire(wire)
    }

    fn from_wire(wire: DerivedWire) -> Result<Self, DerivedArtifactError> {
        if wire.schema_version != SCHEMA_VERSION {
            return Err(DerivedArtifactError::UnsupportedSchema(wire.schema_version));
        }
        let derived = Self::new(wire.calculation, wire.specification)?;
        if derived.aspects != wire.aspects {
            return Err(DerivedArtifactError::AspectMismatch);
        }
        Ok(derived)
    }

    pub fn calculation(&self) -> &CalculationArtifact {
        &self.calculation
    }

    pub fn specification(&self) -> &ChartSpecification {
        &self.specification
    }

    pub fn aspects(&self) -> &[Aspect] {
        &self.aspects
    }

    pub fn to_json(&self) -> Result<String, DerivedArtifactError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_pretty_json(&self) -> Result<String, DerivedArtifactError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn content_sha256(&self) -> Result<String, DerivedArtifactError> {
        Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(self)?)))
    }

    pub fn content_id(&self) -> Result<String, DerivedArtifactError> {
        Ok(format!("sha256:{}", self.content_sha256()?))
    }
}

impl<'de> Deserialize<'de> for DerivedChartArtifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = DerivedWire::deserialize(deserializer)?;
        Self::from_wire(wire).map_err(serde::de::Error::custom)
    }
}

impl Serialize for DerivedChartArtifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DerivedRef {
            schema_version: SCHEMA_VERSION,
            calculation: &self.calculation,
            specification: &self.specification,
            aspects: &self.aspects,
        }
        .serialize(serializer)
    }
}
