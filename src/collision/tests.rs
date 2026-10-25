//! Cross-validation: every method must agree with GJK + EPA on random pairs.

use super::probe::{Recorder, Silent};
use super::registry::{self, METHODS};
use super::shape::{Shape, ShapeKind};
use crate::rand::frand01;

/// Contacts closer than this are skipped: touching is ambiguous within float precision.
const GRAZING: f32 = 0.05;
/// Largest accepted distance difference to GJK + EPA, in px.
const TOLERANCE: f32 = 0.01;

fn random_shape(kind: ShapeKind, seed: &mut u32) -> Shape {
    let mut r = |lo: f32, hi: f32| lo + (hi - lo) * frand01(seed);
    let (x, y) = (r(-120.0, 120.0), r(-120.0, 120.0));
    let params: Vec<f32> = match kind {
        ShapeKind::Circle => vec![x, y, r(5.0, 100.0)],
        ShapeKind::Aabb => vec![x, y, r(5.0, 100.0), r(5.0, 100.0)],
        ShapeKind::Obb | ShapeKind::Ellipse => {
            vec![x, y, r(5.0, 100.0), r(5.0, 100.0), r(0.0, 6.3)]
        }
        ShapeKind::Triangle => (0..6)
            .map(|i| if i % 2 == 0 { x } else { y } + r(-90.0, 90.0))
            .collect(),
        ShapeKind::Polygon => (0..16)
            .map(|i| if i % 2 == 0 { x } else { y } + r(-90.0, 90.0))
            .collect(),
        ShapeKind::Capsule => vec![x, y, x + r(-90.0, 90.0), y + r(-90.0, 90.0), r(5.0, 60.0)],
    };
    Shape::from_params(kind, &params).expect("valid parameters")
}

#[test]
fn every_pair_has_a_method() {
    for a in ShapeKind::ALL {
        for b in ShapeKind::ALL {
            assert!(registry::methods_for(a, b).count() >= 2, "{a:?} vs {b:?}");
        }
    }
}

#[test]
fn methods_agree_with_gjk_epa() {
    let reference = METHODS
        .iter()
        .find(|m| m.id == "gjk-epa")
        .expect("reference");
    let mut seed = 0x9e37_79b9;
    let mut failures = Vec::new();
    for ka in ShapeKind::ALL {
        for kb in ShapeKind::ALL {
            let gjk = registry::resolve(reference, ka, kb).expect("GJK applies everywhere");
            for m in registry::methods_for(ka, kb) {
                let mut worst = 0.0f32;
                let mut mismatches = 0;
                for _ in 0..4000 {
                    let (a, b) = (random_shape(ka, &mut seed), random_shape(kb, &mut seed));
                    let want = gjk.run(&a, &b, &mut Silent);
                    let got = m.run(&a, &b, &mut Silent);
                    let d = want.distance.expect("GJK + EPA always measures");
                    if d.abs() < GRAZING {
                        continue;
                    }
                    if got.intersect != want.intersect {
                        mismatches += 1;
                        if mismatches == 1 {
                            failures.push(format!(
                                "{} {ka:?}/{kb:?}: intersect {} vs {} (d {d})\n  A {a:?}\n  B {b:?}",
                                m.method.id, got.intersect, want.intersect
                            ));
                        }
                    }
                    if let Some(g) = got.distance {
                        worst = worst.max((g - d).abs());
                    }
                }
                if worst > TOLERANCE || mismatches > 0 {
                    failures.push(format!(
                        "{} {ka:?}/{kb:?}: {mismatches} mismatches, worst distance error {worst}",
                        m.method.id
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn recorder_and_silent_agree() {
    let mut seed = 7;
    for ka in ShapeKind::ALL {
        for kb in ShapeKind::ALL {
            for m in registry::methods_for(ka, kb) {
                let (a, b) = (random_shape(ka, &mut seed), random_shape(kb, &mut seed));
                let mut rec = Recorder::default();
                assert_eq!(
                    m.run(&a, &b, &mut rec),
                    m.run(&a, &b, &mut Silent),
                    "{}",
                    m.method.id
                );
            }
        }
    }
}
