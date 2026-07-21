//! Serialized Swiss Ephemeris adapter with explicit source enforcement.

use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    os::raw::c_char,
    path::{Path, PathBuf},
    sync::Mutex,
};

use astraeus_core::{
    Ayanamsa, CalculationError, CalculationProvenance, CalculationRequest, CalculationResult,
    CelestialObject, EphemerisAdapter, EphemerisSource, HouseCusps, HouseSystem, Position, Zodiac,
};
use chrono::{Datelike, Timelike};
use sweph_sys as sys;

static SWISS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EphemerisMode {
    Moshier,
    SwissFiles(PathBuf),
}

#[derive(Clone, Debug)]
pub struct SwissEphemerisAdapter {
    mode: EphemerisMode,
    data_revision: Option<String>,
}

impl SwissEphemerisAdapter {
    pub fn moshier() -> Self {
        Self {
            mode: EphemerisMode::Moshier,
            data_revision: None,
        }
    }

    pub fn swiss_files(path: impl AsRef<Path>) -> Result<Self, CalculationError> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(CalculationError::DataUnavailable(format!(
                "Swiss Ephemeris path is not a directory: {}",
                path.display()
            )));
        }
        path_to_c_string(path)?;
        Ok(Self {
            mode: EphemerisMode::SwissFiles(path.to_owned()),
            data_revision: None,
        })
    }

    pub fn swiss_files_with_revision(
        path: impl AsRef<Path>,
        data_revision: impl Into<String>,
    ) -> Result<Self, CalculationError> {
        let mut adapter = Self::swiss_files(path)?;
        let revision = data_revision.into();
        CalculationProvenance::new(
            "Swiss Ephemeris",
            "2.10.03",
            EphemerisSource::SwissFiles,
            Some(revision.clone()),
        )?;
        adapter.data_revision = Some(revision);
        Ok(adapter)
    }

    pub fn mode(&self) -> &EphemerisMode {
        &self.mode
    }

    fn calculate_locked(
        &self,
        request: &CalculationRequest,
    ) -> Result<CalculationResult, CalculationError> {
        let source_flag = match &self.mode {
            EphemerisMode::Moshier => sys::SEFLG_MOSEPH,
            EphemerisMode::SwissFiles(path) => {
                let path = path_to_c_string(path)?;
                // SAFETY: the CString remains alive for the call; the global adapter lock is held.
                unsafe { sys::swe_set_ephe_path(path.as_ptr()) };
                sys::SEFLG_SWIEPH
            }
        };
        let sidereal_flag = if request.zodiac() == Zodiac::Sidereal {
            let mode = ayanamsa_code(request.ayanamsa().expect("validated sidereal request"));
            // SAFETY: numeric mappings come from the vendored official header; lock is held.
            unsafe { sys::swe_set_sid_mode(mode, 0.0, 0.0) };
            sys::SEFLG_SIDEREAL
        } else {
            0
        };
        let flags = source_flag | sidereal_flag | sys::SEFLG_SPEED;
        let instant = request.instant().as_datetime();
        let hour = f64::from(instant.hour())
            + f64::from(instant.minute()) / 60.0
            + (f64::from(instant.second()) + f64::from(instant.nanosecond()) / 1e9) / 3600.0;
        // SAFETY: scalar inputs are validated and the global adapter lock is held.
        let jd = unsafe {
            sys::swe_julday(
                instant.year(),
                instant.month() as i32,
                instant.day() as i32,
                hour,
                sys::SE_GREG_CAL,
            )
        };

        let mut positions = BTreeMap::new();
        for object in request.objects() {
            if self.mode == EphemerisMode::Moshier && *object == CelestialObject::Chiron {
                return Err(CalculationError::UnsupportedObject(*object));
            }
            let mut output = [0.0; 6];
            let mut error = [0 as c_char; sys::SE_MAX_STNAME];
            // SAFETY: output/error buffers satisfy the C API; lock is held.
            let returned_flags = unsafe {
                sys::swe_calc_ut(
                    jd,
                    object_code(*object),
                    flags,
                    output.as_mut_ptr(),
                    error.as_mut_ptr(),
                )
            };
            if returned_flags < 0 {
                return Err(CalculationError::ObjectCalculation {
                    object: *object,
                    message: error_message(&error),
                });
            }
            let actual_source = returned_flags & sys::SEFLG_EPHMASK;
            if actual_source != source_flag {
                return Err(CalculationError::DataUnavailable(format!(
                    "requested {} for {object:?}, but Swiss Ephemeris returned {}",
                    source_name(source_flag),
                    source_name(actual_source)
                )));
            }
            positions.insert(
                *object,
                Position::new(output[0], output[1], output[2], output[3])?,
            );
        }

        let mut cusps = [0.0; 13];
        let mut angles = [0.0; 10];
        // SAFETY: buffers have the sizes required for twelve-house systems; lock is held.
        let house_status = unsafe {
            sys::swe_houses_ex(
                jd,
                source_flag | sidereal_flag,
                request.location().latitude_degrees(),
                request.location().longitude_degrees(),
                house_code(request.house_system()),
                cusps.as_mut_ptr(),
                angles.as_mut_ptr(),
            )
        };
        if house_status < 0 {
            return Err(CalculationError::Provider(format!(
                "{:?} houses could not be calculated at latitude {}",
                request.house_system(),
                request.location().latitude_degrees()
            )));
        }
        let houses = HouseCusps::new(cusps[1..13].to_vec(), angles[0], angles[1])?;
        let source = match self.mode {
            EphemerisMode::Moshier => EphemerisSource::Moshier,
            EphemerisMode::SwissFiles(_) => EphemerisSource::SwissFiles,
        };
        let provenance = CalculationProvenance::new(
            "Swiss Ephemeris",
            native_version(),
            source,
            self.data_revision.clone(),
        )?;
        CalculationResult::new(request, positions, houses, provenance)
    }
}

impl EphemerisAdapter for SwissEphemerisAdapter {
    fn calculate(
        &self,
        request: &CalculationRequest,
    ) -> Result<CalculationResult, CalculationError> {
        let _guard = SWISS_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.calculate_locked(request)
    }
}

fn path_to_c_string(path: &Path) -> Result<CString, CalculationError> {
    let path = path.to_str().ok_or_else(|| {
        CalculationError::DataUnavailable("Swiss Ephemeris path is not valid UTF-8".into())
    })?;
    CString::new(path.as_bytes()).map_err(|_| {
        CalculationError::DataUnavailable("Swiss Ephemeris path contains a NUL byte".into())
    })
}

fn error_message(buffer: &[c_char]) -> String {
    // SAFETY: the zero-initialized fixed buffer is always NUL terminated.
    unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned()
}

fn native_version() -> String {
    let mut buffer = [0 as c_char; sys::SE_MAX_STNAME];
    // SAFETY: the output buffer is valid and the global adapter lock is held.
    unsafe { sys::swe_version(buffer.as_mut_ptr()) };
    error_message(&buffer)
}

fn source_name(flag: i32) -> &'static str {
    match flag {
        sys::SEFLG_SWIEPH => "Swiss files",
        sys::SEFLG_MOSEPH => "Moshier",
        sys::SEFLG_JPLEPH => "JPL",
        _ => "an unknown source",
    }
}

fn object_code(object: CelestialObject) -> i32 {
    match object {
        CelestialObject::Sun => sys::SE_SUN,
        CelestialObject::Moon => sys::SE_MOON,
        CelestialObject::Mercury => sys::SE_MERCURY,
        CelestialObject::Venus => sys::SE_VENUS,
        CelestialObject::Mars => sys::SE_MARS,
        CelestialObject::Jupiter => sys::SE_JUPITER,
        CelestialObject::Saturn => sys::SE_SATURN,
        CelestialObject::Uranus => sys::SE_URANUS,
        CelestialObject::Neptune => sys::SE_NEPTUNE,
        CelestialObject::Pluto => sys::SE_PLUTO,
        CelestialObject::MeanNode => sys::SE_MEAN_NODE,
        CelestialObject::TrueNode => sys::SE_TRUE_NODE,
        CelestialObject::Chiron => sys::SE_CHIRON,
    }
}

fn house_code(system: HouseSystem) -> i32 {
    match system {
        HouseSystem::Placidus => b'P' as i32,
        HouseSystem::Koch => b'K' as i32,
        HouseSystem::Porphyry => b'O' as i32,
        HouseSystem::Regiomontanus => b'R' as i32,
        HouseSystem::Campanus => b'C' as i32,
        HouseSystem::Equal => b'A' as i32,
        HouseSystem::WholeSign => b'W' as i32,
    }
}

fn ayanamsa_code(ayanamsa: Ayanamsa) -> i32 {
    match ayanamsa {
        Ayanamsa::FaganBradley => sys::SE_SIDM_FAGAN_BRADLEY,
        Ayanamsa::Lahiri => sys::SE_SIDM_LAHIRI,
        Ayanamsa::DeLuce => sys::SE_SIDM_DELUCE,
        Ayanamsa::Raman => sys::SE_SIDM_RAMAN,
        Ayanamsa::Krishnamurti => sys::SE_SIDM_KRISHNAMURTI,
        Ayanamsa::Yukteshwar => 7,
        Ayanamsa::JnBhasin => 8,
    }
}
