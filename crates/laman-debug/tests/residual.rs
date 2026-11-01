//! `laman::residual` against finite differences: for every constraint
//! variant (`laman_debug::strategy::configuration`), 1 000 random
//! configurations, generic and near-degenerate, and at each the analytic
//! gradient of every equation must agree with the stencil of
//! `laman_debug::fd`. The arc's intrinsic equation is held the same way.
//!
//! A variant is listed in [`covered`] when its equations are evaluated; the
//! listing test prints every variant and the property runs over the
//! covered ones.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinate, Coordinates, Entity, EntityId};
use laman::residual::{Equation, Unit, equations, intrinsic};
use laman_debug::fd::{Rule, check};
use laman_debug::strategy::configuration::{Configuration, Variant, configuration, variants};
use proptest::test_runner::{Config, TestCaseError, TestRunner};

/// Cases per variant (the roadmap's number); `PROPTEST_CASES` is not read.
const CASES: u32 = 1_000;

/// Whether the variant's equations are evaluated by `laman::residual`. No
/// wildcard: a new variant fails this to compile.
fn covered(variant: Variant) -> bool {
    match variant {
        Variant::Coincident
        | Variant::Horizontal(_)
        | Variant::Vertical(_)
        | Variant::Midpoint
        | Variant::Fix
        | Variant::Distance => true,
        Variant::PointOnLine
        | Variant::PointOnCircle(_)
        | Variant::Parallel
        | Variant::Perpendicular
        | Variant::EqualLength
        | Variant::EqualRadius(..)
        | Variant::TangentLineCircle(..)
        | Variant::TangentCircles(..)
        | Variant::Symmetric
        | Variant::PointLineDistance(_)
        | Variant::Radius(_)
        | Variant::Angle => false,
    }
}

/// How many equations the variant's constraint contributes (DATA-MODEL.md
/// §Constraints, "Equations").
fn equation_count(variant: Variant) -> usize {
    match variant {
        Variant::Coincident | Variant::Midpoint | Variant::Fix | Variant::Symmetric => 2,
        Variant::PointOnLine
        | Variant::PointOnCircle(_)
        | Variant::Horizontal(_)
        | Variant::Vertical(_)
        | Variant::Parallel
        | Variant::Perpendicular
        | Variant::EqualLength
        | Variant::EqualRadius(..)
        | Variant::TangentLineCircle(..)
        | Variant::TangentCircles(..)
        | Variant::Distance
        | Variant::PointLineDistance(_)
        | Variant::Radius(_)
        | Variant::Angle => 1,
    }
}

/// The sketch's parameters in a fixed order: the dense vector the stencil
/// moves, and the key of each position.
fn dense(c: &Configuration) -> (Vec<Param>, Vec<f64>) {
    let at = c.sketch.coordinates();
    (at.keys().copied().collect(), at.values().copied().collect())
}

type Param = (EntityId, Coordinate);

fn coordinates(keys: &[Param], x: &[f64]) -> Coordinates {
    keys.iter().copied().zip(x.iter().copied()).collect()
}

/// An equation's analytic gradient expanded to the dense form, `fd::check`'s.
fn expand(keys: &[Param], eq: &Equation) -> Result<Vec<f64>, String> {
    let mut out = vec![0.0; keys.len()];
    for (key, g) in eq.terms() {
        let i = keys
            .iter()
            .position(|k| k == key)
            .ok_or_else(|| format!("a term over {key:?}, which the sketch does not have"))?;
        out[i] += g;
    }
    Ok(out)
}

fn rule(unit: Unit, scale: f64) -> Rule {
    match unit {
        Unit::Length => Rule::length(scale),
        Unit::Ratio => Rule::ratio(scale),
    }
}

/// Every equation of the configuration's constraint, against the stencil.
fn agrees(c: &Configuration) -> Result<(), String> {
    let (keys, x) = dense(c);
    let eqs = equations(&c.sketch, &coordinates(&keys, &x), c.constraint)
        .ok_or("the constraint has no equations")?;
    if eqs.len() != equation_count(c.variant) {
        return Err(format!(
            "{} equations, expected {}",
            eqs.len(),
            equation_count(c.variant)
        ));
    }
    let scale = c.local_scale();
    for (n, eq) in eqs.as_slice().iter().enumerate() {
        if eq.is_degenerate() || !eq.value().is_finite() {
            return Err(format!("equation {n} is degenerate or not finite"));
        }
        let gradient = expand(&keys, eq)?;
        let f = |x: &[f64]| {
            equations(&c.sketch, &coordinates(&keys, x), c.constraint)
                .and_then(|e| e.as_slice().get(n).map(Equation::value))
                .unwrap_or(f64::NAN)
        };
        check(f, &x, &gradient, rule(eq.unit(), scale))
            .map_err(|e| format!("equation {n}, local scale {scale:e}: {e}"))?;
    }
    Ok(())
}

/// The intrinsic equation of every arc of the configuration.
fn intrinsic_agrees(c: &Configuration) -> Result<(), String> {
    let (keys, x) = dense(c);
    for (id, entity) in c.sketch.entities() {
        let Entity::Arc { c: centre, s, e } = entity else {
            continue;
        };
        let eq = intrinsic(&c.sketch, &coordinates(&keys, &x), id).ok_or("no equation")?;
        // The equation reads |s − c| and |e − c| and nothing shorter.
        let dist = |p: EntityId| {
            let at = |co| {
                let i = keys.iter().position(|k| *k == (p, co)).unwrap();
                let j = keys.iter().position(|k| *k == (centre.id(), co)).unwrap();
                x[i] - x[j]
            };
            laman::model::tolerance::LENGTH.max(libm::hypot(at(Coordinate::X), at(Coordinate::Y)))
        };
        let scale = dist(s.id()).min(dist(e.id())).min(c.size);
        if c.size / scale > 1.0e6 {
            return Err(format!(
                "intrinsic of {id}: size/scale = {:e}, outside the rule's soundness",
                c.size / scale
            ));
        }
        let gradient = expand(&keys, &eq)?;
        let f = |x: &[f64]| {
            intrinsic(&c.sketch, &coordinates(&keys, x), id)
                .map(|e| e.value())
                .unwrap_or(f64::NAN)
        };
        check(f, &x, &gradient, rule(eq.unit(), scale))
            .map_err(|e| format!("intrinsic of {id}: {e}"))?;
    }
    Ok(())
}

fn run(variant: Variant, property: fn(&Configuration) -> Result<(), String>) {
    let mut runner = TestRunner::new(Config {
        cases: CASES,
        failure_persistence: None,
        ..Config::default()
    });
    let result = runner.run(&configuration(variant), |c| {
        property(&c)
            .map_err(|e| TestCaseError::fail(format!("{:?} (size {:e}): {e}", c.mode, c.size)))
    });
    if let Err(e) = result {
        panic!("{}: {e}", variant.name());
    }
}

#[test]
fn the_variants_are_listed() {
    let all = variants();
    for v in &all {
        println!(
            "{:<48} equations: {}  {}",
            v.name(),
            equation_count(*v),
            if covered(*v) { "evaluated" } else { "pending" }
        );
    }
    assert!(all.len() >= 30, "{} variants", all.len());
}

#[test]
fn every_covered_variant_agrees_with_finite_differences() {
    let mut ran = 0;
    for v in variants().into_iter().filter(|v| covered(*v)) {
        run(v, agrees);
        ran += 1;
    }
    assert!(ran > 0);
}

#[test]
fn an_arcs_intrinsic_equation_agrees_with_finite_differences() {
    // Every variant that draws an arc.
    for v in variants() {
        let name = v.name();
        if name.contains("Arc") {
            run(v, intrinsic_agrees);
        }
    }
}
