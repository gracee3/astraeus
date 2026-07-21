use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize};

use crate::{CelestialObject, Position, ValidationError};

/// Maximum angular error still classified as exact.
pub const ASPECT_EXACT_TOLERANCE_DEGREES: f64 = 1e-9;

/// Maximum absolute relative speed classified as a relative station.
pub const ASPECT_STATION_TOLERANCE_DEGREES_PER_DAY: f64 = 1e-12;

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

/// Instantaneous motion state relative to an aspect's exact angle.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectPhase {
    Applying,
    Exact,
    Separating,
    Stationary,
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
#[serde(deny_unknown_fields)]
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
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Aspect {
    first: CelestialObject,
    second: CelestialObject,
    kind: AspectKind,
    separation_degrees: f64,
    signed_separation_degrees: f64,
    orb_degrees: f64,
    relative_speed_degrees_per_day: f64,
    phase: AspectPhase,
}

impl Aspect {
    fn from_wire(wire: AspectWire) -> Result<Self, ValidationError> {
        if wire.first >= wire.second {
            return Err(ValidationError::InvalidAspectPair);
        }
        if !wire.separation_degrees.is_finite() || !(0.0..=180.0).contains(&wire.separation_degrees)
        {
            return Err(ValidationError::InvalidAspectSeparation(
                wire.separation_degrees.to_string(),
            ));
        }
        if !wire.signed_separation_degrees.is_finite()
            || wire.signed_separation_degrees <= -180.0
            || wire.signed_separation_degrees > 180.0
        {
            return Err(ValidationError::InvalidSignedAspectSeparation(
                wire.signed_separation_degrees.to_string(),
            ));
        }
        if (wire.signed_separation_degrees.abs() - wire.separation_degrees).abs()
            > ASPECT_EXACT_TOLERANCE_DEGREES
        {
            return Err(ValidationError::InconsistentAspectSeparation);
        }
        let expected_orb = (wire.separation_degrees - wire.kind.angle_degrees()).abs();
        if !wire.orb_degrees.is_finite() || (wire.orb_degrees - expected_orb).abs() > 1e-12 {
            return Err(ValidationError::InconsistentAspectOrb {
                kind: wire.kind,
                expected: expected_orb.to_string(),
                actual: wire.orb_degrees.to_string(),
            });
        }
        if !wire.relative_speed_degrees_per_day.is_finite() {
            return Err(ValidationError::InvalidRelativeSpeed(
                wire.relative_speed_degrees_per_day.to_string(),
            ));
        }
        let expected_phase = classify_phase(
            wire.signed_separation_degrees,
            wire.kind,
            wire.relative_speed_degrees_per_day,
        );
        if wire.phase != expected_phase {
            return Err(ValidationError::InconsistentAspectPhase {
                expected: expected_phase,
                actual: wire.phase,
            });
        }
        Ok(Self {
            first: wire.first,
            second: wire.second,
            kind: wire.kind,
            separation_degrees: wire.separation_degrees,
            signed_separation_degrees: wire.signed_separation_degrees,
            orb_degrees: wire.orb_degrees,
            relative_speed_degrees_per_day: wire.relative_speed_degrees_per_day,
            phase: wire.phase,
        })
    }

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
    /// Oriented separation from the first object to the second in (-180, 180].
    pub fn signed_separation_degrees(self) -> f64 {
        self.signed_separation_degrees
    }
    /// Absolute distance from the aspect's exact angle.
    pub fn orb_degrees(self) -> f64 {
        self.orb_degrees
    }
    pub fn relative_speed_degrees_per_day(self) -> f64 {
        self.relative_speed_degrees_per_day
    }
    pub fn phase(self) -> AspectPhase {
        self.phase
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AspectWire {
    first: CelestialObject,
    second: CelestialObject,
    kind: AspectKind,
    separation_degrees: f64,
    signed_separation_degrees: f64,
    orb_degrees: f64,
    relative_speed_degrees_per_day: f64,
    phase: AspectPhase,
}

impl<'de> Deserialize<'de> for Aspect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AspectWire::deserialize(deserializer)?;
        Self::from_wire(wire).map_err(serde::de::Error::custom)
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
            let signed_separation = signed_separation(
                first_position.longitude_degrees(),
                second_position.longitude_degrees(),
            );
            let separation = signed_separation.abs();
            let relative_speed = second_position.longitude_speed_degrees_per_day()
                - first_position.longitude_speed_degrees_per_day();
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
                    signed_separation_degrees: signed_separation,
                    orb_degrees: orb,
                    relative_speed_degrees_per_day: relative_speed,
                    phase: classify_phase(signed_separation, definition.kind(), relative_speed),
                });
            }
        }
    }
    aspects
}

fn signed_separation(first_longitude: f64, second_longitude: f64) -> f64 {
    let separation = (second_longitude - first_longitude).rem_euclid(360.0);
    if separation > 180.0 {
        separation - 360.0
    } else {
        separation
    }
}

fn classify_phase(signed_separation: f64, kind: AspectKind, relative_speed: f64) -> AspectPhase {
    let angle = kind.angle_degrees();
    let signed_target = if signed_separation < 0.0 {
        -angle
    } else {
        angle
    };
    let deviation = signed_separation - signed_target;
    if deviation.abs() <= ASPECT_EXACT_TOLERANCE_DEGREES {
        AspectPhase::Exact
    } else if relative_speed.abs() <= ASPECT_STATION_TOLERANCE_DEGREES_PER_DAY {
        AspectPhase::Stationary
    } else if deviation * relative_speed < 0.0 {
        AspectPhase::Applying
    } else {
        AspectPhase::Separating
    }
}
