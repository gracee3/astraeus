//! Versioned Western chart techniques with explicit method policies.

use std::collections::BTreeMap;

use astraeus_artifacts::CalculationArtifact;
use astraeus_core::{
    AngularPosition, Ayanamsa, CalculationRequest, ChartPointId, EphemerisAdapter,
    GeographicLocation, UtcInstant, Zodiac, chart_point_positions,
};
use astraeus_derived::DerivedChartArtifact;
use astraeus_specifications::ChartSpecification;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SCHEMA_VERSION: u32 = 1;
pub const MEAN_TROPICAL_YEAR_DAYS: f64 = 365.2422;
pub const MEAN_SIDEREAL_MONTH_DAYS: f64 = 27.321_582_18;
pub const NAIBOD_DEGREES_PER_YEAR: f64 = 0.985_647_33;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressionMethod {
    Secondary,
    TertiaryI,
    TertiaryII,
    Minor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnglePolicy {
    NatalFixed,
    RecastAtSymbolicInstant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolarArcMethod {
    Naibod,
    TrueSolarArc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcApplication {
    AllPoints,
    AnglesOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositeFramework {
    PointsOnly,
    MidpointAnglesAndCusps,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SyntheticMethod {
    Harmonic {
        number: u16,
    },
    MidpointComposite {
        framework: CompositeFramework,
    },
    SolarArc {
        method: SolarArcMethod,
        application: ArcApplication,
        arc_degrees: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyntheticChartArtifact {
    schema_version: u32,
    method: SyntheticMethod,
    source_content_ids: Vec<String>,
    zodiac: Zodiac,
    ayanamsa: Option<Ayanamsa>,
    points: BTreeMap<ChartPointId, AngularPosition>,
    house_cusps_degrees: Option<[f64; 12]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressedChartArtifact {
    schema_version: u32,
    method: ProgressionMethod,
    angle_policy: AnglePolicy,
    natal_content_id: String,
    target_instant: UtcInstant,
    symbolic_instant: UtcInstant,
    chart: DerivedChartArtifact,
    points: BTreeMap<ChartPointId, AngularPosition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DavisonChartArtifact {
    schema_version: u32,
    first_content_id: String,
    second_content_id: String,
    midpoint_instant: UtcInstant,
    midpoint_location: GeographicLocation,
    chart: DerivedChartArtifact,
}

#[derive(Debug, Error)]
pub enum TechniqueError {
    #[error("target instant precedes the natal instant")]
    TargetPrecedesNatal,
    #[error("harmonic number must be in 2..=360")]
    InvalidHarmonic,
    #[error("source charts must use the same zodiac and ayanamsa")]
    CoordinateFrameMismatch,
    #[error("source charts have no common points")]
    NoCommonPoints,
    #[error("Davison midpoint is undefined for antipodal locations")]
    AntipodalLocations,
    #[error("true solar arc requires a progressed Sun")]
    MissingProgressedSun,
    #[error("invalid technique value: {0}")]
    InvalidValue(String),
    #[error("calculation failed: {0}")]
    Calculation(String),
}

pub fn symbolic_instant(
    natal: UtcInstant,
    target: UtcInstant,
    method: ProgressionMethod,
) -> Result<UtcInstant, TechniqueError> {
    let elapsed = target.as_datetime() - natal.as_datetime();
    if elapsed < Duration::zero() {
        return Err(TechniqueError::TargetPrecedesNatal);
    }
    let real_days = elapsed.num_milliseconds() as f64 / 86_400_000.0;
    let symbolic_days = match method {
        ProgressionMethod::Secondary => real_days / MEAN_TROPICAL_YEAR_DAYS,
        ProgressionMethod::TertiaryI => (real_days / MEAN_SIDEREAL_MONTH_DAYS).floor(),
        ProgressionMethod::TertiaryII => real_days / MEAN_SIDEREAL_MONTH_DAYS,
        ProgressionMethod::Minor => real_days * MEAN_SIDEREAL_MONTH_DAYS / MEAN_TROPICAL_YEAR_DAYS,
    };
    add_days(natal, symbolic_days)
}

pub fn cast_progressed(
    adapter: &impl EphemerisAdapter,
    natal: &DerivedChartArtifact,
    target: UtcInstant,
    method: ProgressionMethod,
    angle_policy: AnglePolicy,
) -> Result<ProgressedChartArtifact, TechniqueError> {
    let natal_request = natal.calculation().request();
    let symbolic = symbolic_instant(natal_request.instant(), target, method)?;
    let chart = cast(
        adapter,
        natal.specification(),
        symbolic,
        natal_request.location(),
    )?;
    let mut effective_points = points(&chart)?;
    if angle_policy == AnglePolicy::NatalFixed {
        let natal_points = points(natal)?;
        for id in angle_ids() {
            if let Some(position) = natal_points.get(&id) {
                effective_points.insert(id, *position);
            }
        }
    }
    Ok(ProgressedChartArtifact {
        schema_version: SCHEMA_VERSION,
        method,
        angle_policy,
        natal_content_id: content_id(natal)?,
        target_instant: target,
        symbolic_instant: symbolic,
        chart,
        points: effective_points,
    })
}

pub fn harmonic(
    source: &DerivedChartArtifact,
    number: u16,
) -> Result<SyntheticChartArtifact, TechniqueError> {
    if !(2..=360).contains(&number) {
        return Err(TechniqueError::InvalidHarmonic);
    }
    let points = points(source)?
        .into_iter()
        .map(|(id, position)| {
            AngularPosition::new(
                (position.longitude_degrees() * f64::from(number)).rem_euclid(360.0),
                position.longitude_speed_degrees_per_day() * f64::from(number),
            )
            .map(|position| (id, position))
            .map_err(invalid)
        })
        .collect::<Result<_, _>>()?;
    synthetic(source, SyntheticMethod::Harmonic { number }, points, None)
}

pub fn midpoint_composite(
    first: &DerivedChartArtifact,
    second: &DerivedChartArtifact,
    framework: CompositeFramework,
) -> Result<SyntheticChartArtifact, TechniqueError> {
    ensure_frame(first, second)?;
    let first_points = points(first)?;
    let second_points = points(second)?;
    let mut combined = BTreeMap::new();
    for (id, a) in first_points {
        if let Some(b) = second_points.get(&id) {
            combined.insert(id, midpoint_position(a, *b)?);
        }
    }
    if combined.is_empty() {
        return Err(TechniqueError::NoCommonPoints);
    }
    let cusps = match framework {
        CompositeFramework::PointsOnly => None,
        CompositeFramework::MidpointAnglesAndCusps => {
            let a = first.calculation().result().houses().cusps_degrees();
            let b = second.calculation().result().houses().cusps_degrees();
            let mut result = [0.0; 12];
            for index in 0..12 {
                result[index] = forward_midpoint(a[index], b[index]);
            }
            Some(result)
        }
    };
    Ok(SyntheticChartArtifact {
        schema_version: SCHEMA_VERSION,
        method: SyntheticMethod::MidpointComposite { framework },
        source_content_ids: vec![content_id(first)?, content_id(second)?],
        zodiac: first.calculation().request().zodiac(),
        ayanamsa: first.calculation().request().ayanamsa(),
        points: combined,
        house_cusps_degrees: cusps,
    })
}

pub fn solar_arc(
    natal: &DerivedChartArtifact,
    progressed: Option<&ProgressedChartArtifact>,
    target: UtcInstant,
    method: SolarArcMethod,
    application: ArcApplication,
) -> Result<SyntheticChartArtifact, TechniqueError> {
    let elapsed = target.as_datetime() - natal.calculation().request().instant().as_datetime();
    if elapsed < Duration::zero() {
        return Err(TechniqueError::TargetPrecedesNatal);
    }
    let arc = match method {
        SolarArcMethod::Naibod => {
            elapsed.num_milliseconds() as f64 / 86_400_000.0 / MEAN_TROPICAL_YEAR_DAYS
                * NAIBOD_DEGREES_PER_YEAR
        }
        SolarArcMethod::TrueSolarArc => {
            let progressed = progressed.ok_or(TechniqueError::MissingProgressedSun)?;
            let natal_sun = points(natal)?
                .get(&ChartPointId::Sun)
                .ok_or(TechniqueError::MissingProgressedSun)?
                .longitude_degrees();
            let progressed_sun = points(&progressed.chart)?
                .get(&ChartPointId::Sun)
                .ok_or(TechniqueError::MissingProgressedSun)?
                .longitude_degrees();
            (progressed_sun - natal_sun).rem_euclid(360.0)
        }
    };
    let mut transformed = BTreeMap::new();
    for (id, position) in points(natal)? {
        let is_angle = matches!(
            id,
            ChartPointId::Ascendant
                | ChartPointId::Midheaven
                | ChartPointId::Descendant
                | ChartPointId::ImumCoeli
                | ChartPointId::Vertex
        );
        let longitude = if application == ArcApplication::AllPoints || is_angle {
            (position.longitude_degrees() + arc).rem_euclid(360.0)
        } else {
            position.longitude_degrees()
        };
        transformed.insert(
            id,
            AngularPosition::new(longitude, position.longitude_speed_degrees_per_day())
                .map_err(invalid)?,
        );
    }
    synthetic(
        natal,
        SyntheticMethod::SolarArc {
            method,
            application,
            arc_degrees: arc,
        },
        transformed,
        None,
    )
}

pub fn cast_davison(
    adapter: &impl EphemerisAdapter,
    first: &DerivedChartArtifact,
    second: &DerivedChartArtifact,
    specification: &ChartSpecification,
) -> Result<DavisonChartArtifact, TechniqueError> {
    ensure_frame(first, second)?;
    let midpoint_instant = midpoint_instant(
        first.calculation().request().instant(),
        second.calculation().request().instant(),
    )?;
    let midpoint_location = spherical_midpoint(
        first.calculation().request().location(),
        second.calculation().request().location(),
    )?;
    let chart = cast(adapter, specification, midpoint_instant, midpoint_location)?;
    Ok(DavisonChartArtifact {
        schema_version: SCHEMA_VERSION,
        first_content_id: content_id(first)?,
        second_content_id: content_id(second)?,
        midpoint_instant,
        midpoint_location,
        chart,
    })
}

impl SyntheticChartArtifact {
    pub fn points(&self) -> &BTreeMap<ChartPointId, AngularPosition> {
        &self.points
    }
    pub fn method(&self) -> &SyntheticMethod {
        &self.method
    }
    pub fn house_cusps_degrees(&self) -> Option<&[f64; 12]> {
        self.house_cusps_degrees.as_ref()
    }
    pub fn content_id(&self) -> Result<String, TechniqueError> {
        Ok(format!(
            "sha256:{:x}",
            Sha256::digest(serde_json::to_vec(self).map_err(invalid)?)
        ))
    }
}

impl ProgressedChartArtifact {
    pub fn symbolic_instant(&self) -> UtcInstant {
        self.symbolic_instant
    }
    pub fn chart(&self) -> &DerivedChartArtifact {
        &self.chart
    }
    pub fn points(&self) -> &BTreeMap<ChartPointId, AngularPosition> {
        &self.points
    }
}

impl DavisonChartArtifact {
    pub fn midpoint_instant(&self) -> UtcInstant {
        self.midpoint_instant
    }
    pub fn midpoint_location(&self) -> GeographicLocation {
        self.midpoint_location
    }
    pub fn chart(&self) -> &DerivedChartArtifact {
        &self.chart
    }
}

fn cast(
    adapter: &impl EphemerisAdapter,
    specification: &ChartSpecification,
    instant: UtcInstant,
    location: GeographicLocation,
) -> Result<DerivedChartArtifact, TechniqueError> {
    let request: CalculationRequest = specification.request(instant, location);
    let result = adapter
        .calculate(&request)
        .map_err(|error| TechniqueError::Calculation(error.to_string()))?;
    let calculation = CalculationArtifact::new(request, result)
        .map_err(|error| TechniqueError::Calculation(error.to_string()))?;
    DerivedChartArtifact::new(calculation, specification.clone())
        .map_err(|error| TechniqueError::Calculation(error.to_string()))
}

fn points(
    chart: &DerivedChartArtifact,
) -> Result<BTreeMap<ChartPointId, AngularPosition>, TechniqueError> {
    chart_point_positions(chart.calculation().result()).map_err(invalid)
}
fn content_id(chart: &DerivedChartArtifact) -> Result<String, TechniqueError> {
    chart.content_id().map_err(invalid)
}
fn invalid(error: impl ToString) -> TechniqueError {
    TechniqueError::InvalidValue(error.to_string())
}
fn synthetic(
    source: &DerivedChartArtifact,
    method: SyntheticMethod,
    points: BTreeMap<ChartPointId, AngularPosition>,
    house_cusps_degrees: Option<[f64; 12]>,
) -> Result<SyntheticChartArtifact, TechniqueError> {
    Ok(SyntheticChartArtifact {
        schema_version: SCHEMA_VERSION,
        method,
        source_content_ids: vec![content_id(source)?],
        zodiac: source.calculation().request().zodiac(),
        ayanamsa: source.calculation().request().ayanamsa(),
        points,
        house_cusps_degrees,
    })
}
fn ensure_frame(
    first: &DerivedChartArtifact,
    second: &DerivedChartArtifact,
) -> Result<(), TechniqueError> {
    let a = first.calculation().request();
    let b = second.calculation().request();
    if a.zodiac() != b.zodiac() || a.ayanamsa() != b.ayanamsa() {
        Err(TechniqueError::CoordinateFrameMismatch)
    } else {
        Ok(())
    }
}

fn angle_ids() -> [ChartPointId; 5] {
    [
        ChartPointId::Ascendant,
        ChartPointId::Midheaven,
        ChartPointId::Descendant,
        ChartPointId::ImumCoeli,
        ChartPointId::Vertex,
    ]
}

fn forward_midpoint(first: f64, second: f64) -> f64 {
    let delta = (second - first).rem_euclid(360.0);
    let signed = if delta > 180.0 { delta - 360.0 } else { delta };
    (first + signed / 2.0).rem_euclid(360.0)
}
fn midpoint_position(
    first: AngularPosition,
    second: AngularPosition,
) -> Result<AngularPosition, TechniqueError> {
    AngularPosition::new(
        forward_midpoint(first.longitude_degrees(), second.longitude_degrees()),
        (first.longitude_speed_degrees_per_day() + second.longitude_speed_degrees_per_day()) / 2.0,
    )
    .map_err(invalid)
}
fn midpoint_instant(first: UtcInstant, second: UtcInstant) -> Result<UtcInstant, TechniqueError> {
    let delta = second.as_datetime() - first.as_datetime();
    parse_datetime(first.as_datetime() + Duration::milliseconds(delta.num_milliseconds() / 2))
}

fn spherical_midpoint(
    first: GeographicLocation,
    second: GeographicLocation,
) -> Result<GeographicLocation, TechniqueError> {
    let vector = |location: GeographicLocation| {
        let lat = location.latitude_degrees().to_radians();
        let lon = location.longitude_degrees().to_radians();
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let a = vector(first);
    let b = vector(second);
    let sum = [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
    let norm = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    if norm < 1e-12 {
        return Err(TechniqueError::AntipodalLocations);
    }
    GeographicLocation::new(
        (sum[2] / norm).asin().to_degrees(),
        (sum[1] / norm).atan2(sum[0] / norm).to_degrees(),
        (first.elevation_meters() + second.elevation_meters()) / 2.0,
    )
    .map_err(invalid)
}

fn add_days(instant: UtcInstant, days: f64) -> Result<UtcInstant, TechniqueError> {
    let milliseconds = (days * 86_400_000.0).round();
    if !milliseconds.is_finite() || milliseconds > i64::MAX as f64 {
        return Err(invalid("symbolic duration is out of range"));
    }
    parse_datetime(instant.as_datetime() + Duration::milliseconds(milliseconds as i64))
}
fn parse_datetime(value: DateTime<Utc>) -> Result<UtcInstant, TechniqueError> {
    UtcInstant::parse_rfc3339(&value.to_rfc3339_opts(SecondsFormat::Millis, true)).map_err(invalid)
}
