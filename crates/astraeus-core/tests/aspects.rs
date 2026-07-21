use std::collections::BTreeMap;

use astraeus_core::{
    AspectDefinition, AspectDefinitions, AspectKind, CelestialObject, Position, ValidationError,
    calculate_aspects,
};

fn position(longitude: f64) -> Position {
    Position::new(longitude, 0.0, 1.0, 0.0).unwrap()
}

#[test]
fn detects_wraparound_and_inclusive_orbs() {
    let positions = BTreeMap::from([
        (CelestialObject::Sun, position(358.0)),
        (CelestialObject::Moon, position(2.0)),
        (CelestialObject::Mars, position(92.0)),
    ]);
    let definitions = AspectDefinitions::new(vec![
        AspectDefinition::new(AspectKind::Conjunction, 4.0).unwrap(),
        AspectDefinition::new(AspectKind::Square, 2.0).unwrap(),
    ])
    .unwrap();

    let aspects = calculate_aspects(&positions, &definitions);
    assert_eq!(aspects.len(), 2);
    assert_eq!(aspects[0].kind(), AspectKind::Conjunction);
    assert_eq!(aspects[0].separation_degrees(), 4.0);
    assert_eq!(aspects[0].orb_degrees(), 4.0);
    assert_eq!(aspects[1].kind(), AspectKind::Square);
    assert_eq!(aspects[1].orb_degrees(), 0.0);
}

#[test]
fn closest_aspect_wins_when_windows_overlap() {
    let positions = BTreeMap::from([
        (CelestialObject::Sun, position(0.0)),
        (CelestialObject::Moon, position(80.0)),
    ]);
    let definitions = AspectDefinitions::new(vec![
        AspectDefinition::new(AspectKind::Sextile, 25.0).unwrap(),
        AspectDefinition::new(AspectKind::Square, 25.0).unwrap(),
    ])
    .unwrap();

    let aspects = calculate_aspects(&positions, &definitions);
    assert_eq!(aspects[0].kind(), AspectKind::Square);
    assert_eq!(aspects[0].orb_degrees(), 10.0);
}

#[test]
fn definitions_reject_invalid_orbs_and_duplicates() {
    assert!(matches!(
        AspectDefinition::new(AspectKind::Trine, f64::NAN),
        Err(ValidationError::InvalidAspectOrb(_))
    ));
    let conjunction = AspectDefinition::new(AspectKind::Conjunction, 8.0).unwrap();
    assert_eq!(
        AspectDefinitions::new(vec![conjunction, conjunction]).unwrap_err(),
        ValidationError::DuplicateAspect(AspectKind::Conjunction)
    );
}

#[test]
fn json_cannot_bypass_definition_validation() {
    assert!(
        serde_json::from_str::<AspectDefinition>(r#"{"kind":"square","orb_degrees":-1.0}"#)
            .is_err()
    );
    assert!(
        serde_json::from_str::<AspectDefinitions>(
            r#"[{"kind":"square","orb_degrees":3.0},{"kind":"square","orb_degrees":4.0}]"#
        )
        .is_err()
    );
}
