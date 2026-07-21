use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};

use crate::{CelestialObject, Position, ValidationError};

/// The five conventional Ptolemaic aspects.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectKind {
    Conjunction,
    Sextile,
    Square,
    Trine,
    Opposition,
}

impl AspectKind {
    pub const fn angle_degrees(self) -> f64 {
        match self {
            Self::Conjunction => 0.0,
            Self::Sextile => 60.0,
            Self::Square => 90.0,
            Self::Trine => 120.0,
            Self::Opposition => 180.0,
        }
    }
}

/// One aspect and its inclusive maximum orb.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct AspectDefinition {
    kind: AspectKind,
    orb_degrees: f64,
}

impl AspectDefinition {
    pub fn new(kind: AspectKind, orb_degrees: f64) -> Result<Self, ValidationError> {
        if !orb_degrees.is_finite() || !(0.0..=180.0).contains(&orb_degrees) {
            return Err(ValidationError::InvalidAspectOrb(orb_degrees.to_string()));
        }
        Ok(Self { kind, orb_degrees })
    }

    pub fn kind(self) -> AspectKind {
        self.kind
    }

    pub fn orb_degrees(self) -> f64 {
        self.orb_degrees
    }
}

#[derive(Deserialize)]
struct AspectDefinitionWire {
    kind: AspectKind,
    orb_degrees: f64,
}

impl<'de> Deserialize<'de> for AspectDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AspectDefinitionWire::deserialize(deserializer)?;
        Self::new(wire.kind, wire.orb_degrees).map_err(serde::de::Error::custom)
    }
}

/// A validated, deterministic set of aspect definitions.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AspectDefinitions(Vec<AspectDefinition>);

impl AspectDefinitions {
    pub fn new(definitions: Vec<AspectDefinition>) -> Result<Self, ValidationError> {
        let mut seen = BTreeSet::new();
        for definition in &definitions {
            if !seen.insert(definition.kind()) {
                return Err(ValidationError::DuplicateAspect(definition.kind()));
            }
        }
        Ok(Self(definitions))
    }

    pub fn ptolemaic(orb_degrees: f64) -> Result<Self, ValidationError> {
        Self::new(
            [
                AspectKind::Conjunction,
                AspectKind::Sextile,
                AspectKind::Square,
                AspectKind::Trine,
                AspectKind::Opposition,
            ]
            .into_iter()
            .map(|kind| AspectDefinition::new(kind, orb_degrees))
            .collect::<Result<_, _>>()?,
        )
    }

    pub fn as_slice(&self) -> &[AspectDefinition] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for AspectDefinitions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let definitions = Vec::<AspectDefinition>::deserialize(deserializer)?;
        Self::new(definitions).map_err(serde::de::Error::custom)
    }
}

/// A detected aspect. Objects are always ordered by [`CelestialObject`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aspect {
    first: CelestialObject,
    second: CelestialObject,
    kind: AspectKind,
    separation_degrees: f64,
    orb_degrees: f64,
}

impl Aspect {
    pub fn first(self) -> CelestialObject {
        self.first
    }
    pub fn second(self) -> CelestialObject {
        self.second
    }
    pub fn kind(self) -> AspectKind {
        self.kind
    }
    pub fn separation_degrees(self) -> f64 {
        self.separation_degrees
    }
    /// Absolute distance from the aspect's exact angle.
    pub fn orb_degrees(self) -> f64 {
        self.orb_degrees
    }
}

/// Detect aspects using ecliptic longitude and inclusive orb boundaries.
///
/// At most one aspect is emitted per object pair. If configured aspect windows
/// overlap, the closest exact angle wins; [`AspectKind`] order breaks exact ties.
pub fn calculate_aspects(
    positions: &BTreeMap<CelestialObject, Position>,
    definitions: &AspectDefinitions,
) -> Vec<Aspect> {
    let entries: Vec<_> = positions.iter().collect();
    let mut aspects = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let (first, first_position) = *entry;
        for entry in &entries[index + 1..] {
            let (second, second_position) = *entry;
            let raw =
                (first_position.longitude_degrees() - second_position.longitude_degrees()).abs();
            let separation = raw.min(360.0 - raw);
            let best = definitions
                .as_slice()
                .iter()
                .map(|definition| {
                    let orb = (separation - definition.kind().angle_degrees()).abs();
                    (definition, orb)
                })
                .filter(|(definition, orb)| *orb <= definition.orb_degrees())
                .min_by(
                    |(left_definition, left_orb), (right_definition, right_orb)| {
                        left_orb
                            .total_cmp(right_orb)
                            .then_with(|| left_definition.kind().cmp(&right_definition.kind()))
                    },
                );
            if let Some((definition, orb)) = best {
                aspects.push(Aspect {
                    first: *first,
                    second: *second,
                    kind: definition.kind(),
                    separation_degrees: separation,
                    orb_degrees: orb,
                });
            }
        }
    }
    aspects
}
