//! The finite-difference rule's own tests (`laman_debug::fd`): on closed
//! forms whose gradients are known, at sketch sizes `1e-6`, `1` and `1e3`
//! and local scales of the sketch's size and a millionth of it, anywhere in
//! the sketch, the exact gradient passes, and a negated term, a dropped
//! term and a `1e-4` relative error each fail.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman_debug::fd::{Disagreement, Rule, check};
use proptest::prelude::*;
use proptest::sample::select;

/// One closed form at one configuration: its parameters, its exact
/// gradient there, and the rule it is held by.
struct Case {
    x: Vec<f64>,
    gradient: Vec<f64>,
    rule: Rule,
}

fn cross(u: [f64; 2], v: [f64; 2]) -> f64 {
    u[0] * v[1] - u[1] * v[0]
}

fn norm(u: [f64; 2]) -> f64 {
    (u[0] * u[0] + u[1] * u[1]).sqrt()
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn pt(x: &[f64], i: usize) -> [f64; 2] {
    [x[2 * i], x[2 * i + 1]]
}

fn unit(angle: f64) -> [f64; 2] {
    [libm::cos(angle), libm::sin(angle)]
}

fn at(o: [f64; 2], d: [f64; 2], s: f64) -> [f64; 2] {
    [o[0] + s * d[0], o[1] + s * d[1]]
}

/// `|b − a|` over `[ax, ay, bx, by]`: a length, local scale `|b − a|`.
fn distance(x: &[f64]) -> f64 {
    norm(sub(pt(x, 1), pt(x, 0)))
}

fn distance_case(a: [f64; 2], b: [f64; 2]) -> Case {
    let u = sub(b, a);
    let n = norm(u);
    let g = [u[0] / n, u[1] / n];
    Case {
        x: vec![a[0], a[1], b[0], b[1]],
        gradient: vec![-g[0], -g[1], g[0], g[1]],
        rule: Rule::length(n),
    }
}

/// `cross(u, p − a)/|u|`, `u = b − a`, over `[ax, ay, bx, by, px, py]`:
/// the signed distance of `p` from line `a → b`, a length, local scale `|u|`.
fn line_distance(x: &[f64]) -> f64 {
    let (a, b, p) = (pt(x, 0), pt(x, 1), pt(x, 2));
    let u = sub(b, a);
    cross(u, sub(p, a)) / norm(u)
}

fn line_distance_case(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> Case {
    let (u, w) = (sub(b, a), sub(p, a));
    let n = norm(u);
    let f = cross(u, w) / n;
    let dp = [-u[1] / n, u[0] / n];
    let du = [
        w[1] / n - f * u[0] / (n * n),
        -w[0] / n - f * u[1] / (n * n),
    ];
    Case {
        x: vec![a[0], a[1], b[0], b[1], p[0], p[1]],
        gradient: vec![-du[0] - dp[0], -du[1] - dp[1], du[0], du[1], dp[0], dp[1]],
        rule: Rule::length(n),
    }
}

/// `cross(u, v)/(|u||v|)`, `u = b − a`, `v = d − c`, over the four points:
/// the sine of the angle between two lines, a ratio, local scale the
/// shorter line.
fn sine(x: &[f64]) -> f64 {
    let u = sub(pt(x, 1), pt(x, 0));
    let v = sub(pt(x, 3), pt(x, 2));
    cross(u, v) / (norm(u) * norm(v))
}

fn sine_case(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Case {
    let (u, v) = (sub(b, a), sub(d, c));
    let (nu, nv) = (norm(u), norm(v));
    let s = cross(u, v) / (nu * nv);
    let du = [
        v[1] / (nu * nv) - s * u[0] / (nu * nu),
        -v[0] / (nu * nv) - s * u[1] / (nu * nu),
    ];
    let dv = [
        -u[1] / (nu * nv) - s * v[0] / (nv * nv),
        u[0] / (nu * nv) - s * v[1] / (nv * nv),
    ];
    Case {
        x: [a, b, c, d].iter().flatten().copied().collect(),
        gradient: vec![-du[0], -du[1], du[0], du[1], -dv[0], -dv[1], dv[0], dv[1]],
        rule: Rule::ratio(nu.min(nv)),
    }
}

/// The three planted errors, each on the largest term: negated, dropped,
/// and off by `1e-4` of itself.
fn planted(gradient: &[f64]) -> [Vec<f64>; 3] {
    let i = (0..gradient.len())
        .max_by(|&i, &j| gradient[i].abs().total_cmp(&gradient[j].abs()))
        .unwrap();
    let mut negated = gradient.to_vec();
    negated[i] = -negated[i];
    let mut dropped = gradient.to_vec();
    dropped[i] = 0.0;
    let mut scaled = gradient.to_vec();
    scaled[i] *= 1.0 + 1.0e-4;
    [negated, dropped, scaled]
}

fn hold(f: fn(&[f64]) -> f64, case: &Case) {
    let exact = check(f, &case.x, &case.gradient, case.rule);
    assert!(
        exact.is_ok(),
        "exact gradient refused: {exact:?}; x = {:?}",
        case.x
    );
    for (name, wrong) in ["negated", "dropped", "1e-4"]
        .iter()
        .zip(planted(&case.gradient))
    {
        let refused = check(f, &case.x, &wrong, case.rule);
        assert!(
            matches!(refused, Err(Disagreement::Term { .. })),
            "{name} term accepted: x = {:?}",
            case.x
        );
    }
}

/// A sketch's size, its local scale (the size, or a millionth of it), and
/// an origin anywhere in a box of that size.
fn frame() -> impl Strategy<Value = (f64, f64, [f64; 2])> {
    (
        select(vec![1.0e-6, 1.0, 1.0e3]),
        select(vec![1.0, 1.0e-6]),
        -1.0..1.0_f64,
        -1.0..1.0_f64,
        0.5..1.0_f64,
    )
        .prop_map(|(size, ratio, ox, oy, k)| (size, size * ratio * k, [ox * size, oy * size]))
}

fn angle() -> impl Strategy<Value = f64> {
    -std::f64::consts::PI..std::f64::consts::PI
}

proptest! {
    #[test]
    fn point_pair_distance((_, len, o) in frame(), th in angle()) {
        let b = at(o, unit(th), len);
        hold(distance, &distance_case(o, b));
    }

    /// `p` anywhere in the sketch; `1e-9` of the size off the line; or
    /// square off `a`, where the line's terms vanish to first order while
    /// their curvature stays `|p − a|/|u|`: the stencil's truncation then
    /// meets the floor of the bound, not the gradient's size.
    #[test]
    fn point_to_line_distance(
        (size, len, o) in frame(),
        th in angle(),
        ph in angle(),
        t in -1.0..1.0_f64,
        mode in 0..3_u8,
    ) {
        let d = unit(th);
        let b = at(o, d, len);
        let n = [-d[1], d[0]];
        let p = match mode {
            0 => at(o, unit(ph), t.abs().max(0.1) * size),
            1 => at(at(o, d, t * size), n, 1.0e-9 * size),
            _ => at(o, n, t.abs().max(0.1) * size),
        };
        hold(line_distance, &line_distance_case(o, b, p));
    }

    /// The second line at any angle at least `0.1` rad from perpendicular,
    /// or within `1e-6` rad of parallel. Near perpendicular every term of
    /// the gradient vanishes (`cos θ` factors them all) and a planted
    /// error with them, below what any rule can see.
    #[test]
    fn sine_between_lines(
        (size, len, o) in frame(),
        th in angle(),
        ph in angle(),
        delta in -1.47..1.47_f64,
        flip in any::<bool>(),
        oc in -1.0..1.0_f64,
        near in any::<bool>(),
    ) {
        let b = at(o, unit(th), len);
        let c = at(o, unit(ph), oc.abs() * size);
        let turn = if flip { std::f64::consts::PI } else { 0.0 };
        let tilt = th + turn + if near { 1.0e-6 * oc } else { delta };
        let d = at(c, unit(tilt), size);
        hold(sine, &sine_case(o, b, c, d));
    }
}

#[test]
fn closed_forms_at_named_points() {
    // A unit line at the origin, p at (0.3, 2): distance 2.
    hold(
        line_distance,
        &line_distance_case([0.0, 0.0], [1.0, 0.0], [0.3, 2.0]),
    );
    // A line 1e-6 long at (0.7, -0.4), the sketch's size 1: X/ℓ ≈ 1e6.
    let a = [0.7, -0.4];
    hold(
        line_distance,
        &line_distance_case(a, at(a, unit(0.3), 1.0e-6), [-0.9, 0.8]),
    );
    // The whole sketch at 1e-6 and at 1e3.
    for s in [1.0e-6, 1.0e3] {
        let (a, b, p) = ([0.2 * s, 0.1 * s], [0.9 * s, 0.5 * s], [-0.3 * s, 0.6 * s]);
        hold(line_distance, &line_distance_case(a, b, p));
        hold(distance, &distance_case(a, b));
    }
}

#[test]
fn malformed_inputs_are_refused_not_panicked() {
    let x = [3.0, 4.0];
    let f = |v: &[f64]| norm([v[0], v[1]]);
    assert!(matches!(
        check(f, &x, &[0.6], Rule::length(5.0)),
        Err(Disagreement::Shape {
            point: 2,
            gradient: 1
        })
    ));
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            check(f, &x, &[0.6, 0.8], Rule::length(scale)),
            Err(Disagreement::Input(_))
        ));
    }
    assert!(matches!(
        check(f, &x, &[f64::NAN, 0.8], Rule::length(5.0)),
        Err(Disagreement::Input(_))
    ));
    // A function that is NaN near the point fails rather than passes.
    let nan = |v: &[f64]| if v[0] > 3.0 { f64::NAN } else { v[0] };
    assert!(matches!(
        check(nan, &x, &[1.0, 0.0], Rule::length(5.0)),
        Err(Disagreement::Term { index: 0, .. })
    ));
}
