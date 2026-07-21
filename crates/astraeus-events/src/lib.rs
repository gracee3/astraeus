//! Exact-time astronomical event solving and ordinary event-chart artifacts.

use astraeus_artifacts::CalculationArtifact;
use astraeus_core::{CelestialObject, EphemerisAdapter, GeographicLocation, UtcInstant};
use astraeus_derived::DerivedChartArtifact;
use astraeus_specifications::ChartSpecification;
use chrono::{Duration, SecondsFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_TIME_TOLERANCE_SECONDS: f64 = 1.0;
pub const DEFAULT_ANGULAR_TOLERANCE_DEGREES: f64 = 1e-5;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventSelection {
    Previous,
    Nearest,
    Next,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReturnFrame {
    ConfiguredZodiac,
    BirthEpochEclipticPrecessionCorrected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LunationKind {
    NewMoon,
    FullMoon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeasonalPoint {
    MarchEquinox,
    JuneSolstice,
    SeptemberEquinox,
    DecemberSolstice,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventDefinition {
    Return {
        object: CelestialObject,
        target_longitude_degrees: f64,
        frame: ReturnFrame,
    },
    Lunation {
        lunation: LunationKind,
    },
    Ingress {
        object: CelestialObject,
        target_longitude_degrees: f64,
    },
    Seasonal {
        point: SeasonalPoint,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventSearch {
    pub reference: UtcInstant,
    pub selection: EventSelection,
    pub window_days: f64,
    pub scan_step_hours: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolverMetadata {
    algorithm: String,
    time_tolerance_seconds: f64,
    angular_tolerance_degrees: f64,
    iterations: u32,
    residual_degrees: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventChartArtifact {
    schema_version: u32,
    definition: EventDefinition,
    selection: EventSelection,
    exact_instant: UtcInstant,
    solver: SolverMetadata,
    chart: DerivedChartArtifact,
}

#[derive(Debug, Error)]
pub enum EventError {
    #[error("search window and scan step must be finite and positive")]
    InvalidSearch,
    #[error("event longitude must be finite and in [0, 360)")]
    InvalidLongitude,
    #[error("chart specification does not request required object {0:?}")]
    MissingObject(CelestialObject),
    #[error("no event root found in the requested window")]
    NoRoot,
    #[error("event solver did not meet the residual target")]
    ResidualTooLarge,
    #[error("calculation failed: {0}")]
    Calculation(String),
    #[error("invalid event time: {0}")]
    Time(String),
    #[error("serialization failed: {0}")]
    Serialization(String),
}

pub fn solve_event(
    adapter: &impl EphemerisAdapter,
    specification: &ChartSpecification,
    location: GeographicLocation,
    definition: EventDefinition,
    search: EventSearch,
) -> Result<EventChartArtifact, EventError> {
    validate(definition, search, specification)?;
    let center = search.reference.as_datetime();
    let window_ms = (search.window_days * 86_400_000.0).round() as i64;
    let step_ms = (search.scan_step_hours * 3_600_000.0).round() as i64;
    let start = center - Duration::milliseconds(window_ms);
    let end = center + Duration::milliseconds(window_ms);
    let mut roots = Vec::new();
    let mut left = start;
    let mut left_value = residual(adapter, specification, location, definition, instant(left)?)?;
    while left < end {
        let right = (left + Duration::milliseconds(step_ms)).min(end);
        let right_value = residual(
            adapter,
            specification,
            location,
            definition,
            instant(right)?,
        )?;
        if left_value.abs() <= DEFAULT_ANGULAR_TOLERANCE_DEGREES {
            roots.push((left, 0, left_value));
        }
        if left_value.signum() != right_value.signum() && (left_value - right_value).abs() < 180.0 {
            roots.push(bisect(
                adapter,
                specification,
                location,
                definition,
                left,
                right,
                left_value,
            )?);
        }
        left = right;
        left_value = right_value;
    }
    roots.sort_by_key(|root| root.0);
    roots.dedup_by(|a, b| (a.0 - b.0).num_seconds().abs() <= 1);
    let chosen = choose(&roots, center, search.selection).ok_or(EventError::NoRoot)?;
    if chosen.2.abs() > DEFAULT_ANGULAR_TOLERANCE_DEGREES {
        return Err(EventError::ResidualTooLarge);
    }
    let exact = instant(chosen.0)?;
    let request = specification.request(exact, location);
    let result = adapter.calculate(&request).map_err(calc)?;
    let calculation = CalculationArtifact::new(request, result).map_err(calc)?;
    let chart = DerivedChartArtifact::new(calculation, specification.clone()).map_err(calc)?;
    Ok(EventChartArtifact {
        schema_version: SCHEMA_VERSION,
        definition,
        selection: search.selection,
        exact_instant: exact,
        solver: SolverMetadata {
            algorithm: "scan_bracket_bisection_v1".into(),
            time_tolerance_seconds: DEFAULT_TIME_TOLERANCE_SECONDS,
            angular_tolerance_degrees: DEFAULT_ANGULAR_TOLERANCE_DEGREES,
            iterations: chosen.1,
            residual_degrees: chosen.2.abs(),
        },
        chart,
    })
}

impl EventChartArtifact {
    pub fn exact_instant(&self) -> UtcInstant {
        self.exact_instant
    }
    pub fn residual_degrees(&self) -> f64 {
        self.solver.residual_degrees
    }
    pub fn chart(&self) -> &DerivedChartArtifact {
        &self.chart
    }
    pub fn content_id(&self) -> Result<String, EventError> {
        Ok(format!(
            "sha256:{:x}",
            Sha256::digest(serde_json::to_vec(self).map_err(serial)?)
        ))
    }
}

fn validate(
    definition: EventDefinition,
    search: EventSearch,
    specification: &ChartSpecification,
) -> Result<(), EventError> {
    if !search.window_days.is_finite()
        || search.window_days <= 0.0
        || !search.scan_step_hours.is_finite()
        || search.scan_step_hours <= 0.0
    {
        return Err(EventError::InvalidSearch);
    }
    for object in required_objects(definition) {
        if !specification.calculation().objects().contains(&object) {
            return Err(EventError::MissingObject(object));
        }
    }
    let longitude = match definition {
        EventDefinition::Return {
            target_longitude_degrees,
            ..
        }
        | EventDefinition::Ingress {
            target_longitude_degrees,
            ..
        } => Some(target_longitude_degrees),
        _ => None,
    };
    if longitude.is_some_and(|value| !value.is_finite() || !(0.0..360.0).contains(&value)) {
        return Err(EventError::InvalidLongitude);
    }
    Ok(())
}

fn required_objects(definition: EventDefinition) -> Vec<CelestialObject> {
    match definition {
        EventDefinition::Return { object, .. } | EventDefinition::Ingress { object, .. } => {
            vec![object]
        }
        EventDefinition::Lunation { .. } => vec![CelestialObject::Sun, CelestialObject::Moon],
        EventDefinition::Seasonal { .. } => vec![CelestialObject::Sun],
    }
}
fn target(definition: EventDefinition) -> f64 {
    match definition {
        EventDefinition::Return {
            target_longitude_degrees,
            ..
        }
        | EventDefinition::Ingress {
            target_longitude_degrees,
            ..
        } => target_longitude_degrees,
        EventDefinition::Lunation {
            lunation: LunationKind::NewMoon,
        } => 0.0,
        EventDefinition::Lunation {
            lunation: LunationKind::FullMoon,
        } => 180.0,
        EventDefinition::Seasonal { point } => match point {
            SeasonalPoint::MarchEquinox => 0.0,
            SeasonalPoint::JuneSolstice => 90.0,
            SeasonalPoint::SeptemberEquinox => 180.0,
            SeasonalPoint::DecemberSolstice => 270.0,
        },
    }
}

fn residual(
    adapter: &impl EphemerisAdapter,
    specification: &ChartSpecification,
    location: GeographicLocation,
    definition: EventDefinition,
    at: UtcInstant,
) -> Result<f64, EventError> {
    let request = specification.request(at, location);
    let result = adapter.calculate(&request).map_err(calc)?;
    let longitude = match definition {
        EventDefinition::Lunation { .. } => (result.positions()[&CelestialObject::Moon]
            .longitude_degrees()
            - result.positions()[&CelestialObject::Sun].longitude_degrees())
        .rem_euclid(360.0),
        EventDefinition::Return { object, .. } | EventDefinition::Ingress { object, .. } => {
            result.positions()[&object].longitude_degrees()
        }
        EventDefinition::Seasonal { .. } => {
            result.positions()[&CelestialObject::Sun].longitude_degrees()
        }
    };
    Ok(signed(longitude - target(definition)))
}

fn bisect(
    adapter: &impl EphemerisAdapter,
    specification: &ChartSpecification,
    location: GeographicLocation,
    definition: EventDefinition,
    mut left: chrono::DateTime<chrono::Utc>,
    mut right: chrono::DateTime<chrono::Utc>,
    mut left_value: f64,
) -> Result<(chrono::DateTime<chrono::Utc>, u32, f64), EventError> {
    let mut iterations = 0;
    while (right - left).num_milliseconds() as f64 > DEFAULT_TIME_TOLERANCE_SECONDS * 1000.0
        && iterations < 80
    {
        let middle = left + Duration::milliseconds((right - left).num_milliseconds() / 2);
        let value = residual(
            adapter,
            specification,
            location,
            definition,
            instant(middle)?,
        )?;
        if value.abs() <= DEFAULT_ANGULAR_TOLERANCE_DEGREES {
            return Ok((middle, iterations + 1, value));
        }
        if left_value.signum() == value.signum() {
            left = middle;
            left_value = value;
        } else {
            right = middle;
        }
        iterations += 1;
    }
    let middle = left + Duration::milliseconds((right - left).num_milliseconds() / 2);
    let value = residual(
        adapter,
        specification,
        location,
        definition,
        instant(middle)?,
    )?;
    Ok((middle, iterations, value))
}

fn choose(
    roots: &[(chrono::DateTime<chrono::Utc>, u32, f64)],
    reference: chrono::DateTime<chrono::Utc>,
    selection: EventSelection,
) -> Option<(chrono::DateTime<chrono::Utc>, u32, f64)> {
    let eligible = roots.iter().copied().filter(|root| match selection {
        EventSelection::Previous => root.0 <= reference,
        EventSelection::Next => root.0 >= reference,
        EventSelection::Nearest => true,
    });
    match selection {
        EventSelection::Previous => eligible.max_by_key(|root| root.0),
        EventSelection::Next => eligible.min_by_key(|root| root.0),
        EventSelection::Nearest => eligible.min_by_key(|root| {
            // Treat sub-second solver noise as equal; exact ties choose earlier.
            (
                (root.0 - reference).num_milliseconds().abs() / 1_000,
                root.0,
            )
        }),
    }
}
fn signed(value: f64) -> f64 {
    let value = value.rem_euclid(360.0);
    if value > 180.0 { value - 360.0 } else { value }
}
fn instant(value: chrono::DateTime<chrono::Utc>) -> Result<UtcInstant, EventError> {
    UtcInstant::parse_rfc3339(&value.to_rfc3339_opts(SecondsFormat::Millis, true))
        .map_err(|error| EventError::Time(error.to_string()))
}
fn calc(error: impl ToString) -> EventError {
    EventError::Calculation(error.to_string())
}
fn serial(error: impl ToString) -> EventError {
    EventError::Serialization(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_nearest_tie_chooses_earlier() {
        let reference = chrono::DateTime::parse_from_rfc3339("2000-01-02T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let earlier = reference - Duration::days(1);
        let later = reference + Duration::days(1);
        assert_eq!(
            choose(
                &[(later, 1, 0.0), (earlier, 1, 0.0)],
                reference,
                EventSelection::Nearest
            )
            .unwrap()
            .0,
            earlier
        );
    }
}
