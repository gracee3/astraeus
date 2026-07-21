use astraeus_core::{
    Ayanamsa, CalculationError, CalculationRequest, CelestialObject, EphemerisAdapter,
    GeographicLocation, HouseSystem, UtcInstant, Zodiac,
};
use astraeus_fixtures::{GoldenFixture, parse_swetest_output};
use astraeus_swiss::SwissEphemerisAdapter;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/swetest-v2.10.03"
);

fn fixture(name: &str) -> GoldenFixture {
    let json = std::fs::read_to_string(format!("{FIXTURES}/{name}.json")).unwrap();
    GoldenFixture::from_json(&json).unwrap()
}

#[test]
fn moshier_matches_tropical_and_sidereal_references() {
    let adapter = SwissEphemerisAdapter::moshier();
    for name in [
        "j2000-greenwich-tropical-placidus",
        "j2000-greenwich-sidereal-lahiri-placidus",
    ] {
        let fixture = fixture(name);
        fixture
            .compare(&adapter.calculate(fixture.request()).unwrap())
            .unwrap();
    }
}

#[test]
fn swiss_mode_rejects_silent_moshier_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let adapter = SwissEphemerisAdapter::swiss_files(directory.path()).unwrap();
    let fixture = fixture("j2000-greenwich-tropical-placidus");
    assert!(matches!(
        adapter.calculate(fixture.request()),
        Err(CalculationError::DataUnavailable(_))
    ));
}

#[test]
fn moshier_rejects_file_only_chiron() {
    let fixture = fixture("j2000-greenwich-tropical-placidus");
    let mut objects = fixture.request().objects().to_vec();
    objects.push(CelestialObject::Chiron);
    let request = astraeus_core::CalculationRequest::new(
        fixture.request().instant(),
        fixture.request().location(),
        objects,
        fixture.request().zodiac(),
        fixture.request().ayanamsa(),
        fixture.request().house_system(),
    )
    .unwrap();
    assert_eq!(
        SwissEphemerisAdapter::moshier()
            .calculate(&request)
            .unwrap_err(),
        CalculationError::UnsupportedObject(CelestialObject::Chiron)
    );
}

#[test]
fn every_public_ayanamsa_maps_to_a_native_mode() {
    let fixture = fixture("j2000-greenwich-tropical-placidus");
    for ayanamsa in [
        Ayanamsa::FaganBradley,
        Ayanamsa::Lahiri,
        Ayanamsa::DeLuce,
        Ayanamsa::Raman,
        Ayanamsa::Krishnamurti,
        Ayanamsa::Yukteshwar,
        Ayanamsa::JnBhasin,
    ] {
        let request = CalculationRequest::new(
            fixture.request().instant(),
            fixture.request().location(),
            vec![CelestialObject::Sun],
            Zodiac::Sidereal,
            Some(ayanamsa),
            fixture.request().house_system(),
        )
        .unwrap();
        SwissEphemerisAdapter::moshier()
            .calculate(&request)
            .unwrap();
    }
}

#[test]
#[ignore = "set ASTRAEUS_SWISS_EPHEMERIS_PATH to a directory containing pinned .se1 files"]
fn swiss_files_match_tropical_and_sidereal_references() {
    let path = std::env::var("ASTRAEUS_SWISS_EPHEMERIS_PATH").unwrap();
    let adapter = SwissEphemerisAdapter::swiss_files(path).unwrap();
    for (zodiac, ayanamsa, file) in [
        (
            Zodiac::Tropical,
            None,
            "j2000-greenwich-swiss-tropical.stdout",
        ),
        (
            Zodiac::Sidereal,
            Some(Ayanamsa::Lahiri),
            "j2000-greenwich-swiss-sidereal-lahiri.stdout",
        ),
    ] {
        let request = CalculationRequest::new(
            UtcInstant::parse_rfc3339("2000-01-01T12:00:00Z").unwrap(),
            GeographicLocation::new(51.4779, 0.0, 46.0).unwrap(),
            vec![
                CelestialObject::Sun,
                CelestialObject::Moon,
                CelestialObject::Chiron,
            ],
            zodiac,
            ayanamsa,
            HouseSystem::Placidus,
        )
        .unwrap();
        let raw = std::fs::read_to_string(format!("{FIXTURES}/{file}")).unwrap();
        let expected = parse_swetest_output(&request, &raw).unwrap();
        let actual = adapter.calculate(&request).unwrap();
        for (object, expected_position) in expected.positions() {
            let actual_position = actual.positions().get(object).unwrap();
            assert!(
                (expected_position.longitude_degrees() - actual_position.longitude_degrees()).abs()
                    <= 1e-6
            );
            assert!(
                (expected_position.latitude_degrees() - actual_position.latitude_degrees()).abs()
                    <= 1e-6
            );
            assert!(
                (expected_position.distance_au() - actual_position.distance_au()).abs() <= 1e-9
            );
            assert!(
                (expected_position.longitude_speed_degrees_per_day()
                    - actual_position.longitude_speed_degrees_per_day())
                .abs()
                    <= 1e-6
            );
        }
        for (expected_cusp, actual_cusp) in expected
            .houses()
            .cusps_degrees()
            .iter()
            .zip(actual.houses().cusps_degrees())
        {
            assert!((expected_cusp - actual_cusp).abs() <= 1e-6);
        }
    }
}
