//! The measuring frame of ADR-0012 D3, shared by the profile geometry and
//! the intersection kernel (ADR-0016 D8): coordinates translated to an
//! origin and scaled by exact powers of two to an extent near 1, so no
//! product of two coordinates overflows or underflows at any finite input
//! and a small figure far from the origin keeps its digits.

/// Coordinates translated to an origin and scaled to an extent in `[½, 1)`
/// by exact powers of two.
///
/// Two stages, so that neither the translation nor a product overflows:
/// the points are first scaled so the largest coordinate is below 1
/// (`coarse`), then translated, then scaled again so the largest offset is
/// (`fine`). Scaling by a power of two is exact away from the subnormals,
/// so the measures are those of the caller's coordinates.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    coarse: i32,
    origin: [f64; 2],
    fine: i32,
}

impl Frame {
    /// The frame of `points`, translated to `origin`, with `radius` (a
    /// circle's, or 0) counted in its extent.
    pub(crate) fn new(points: &[[f64; 2]], origin: [f64; 2], radius: f64) -> Self {
        let largest = points
            .iter()
            .chain(std::iter::once(&origin))
            .flat_map(|p| p.iter())
            .fold(0.0_f64, |m, x| m.max(x.abs()));
        let coarse = exponent(largest);
        let origin = scale(origin, -coarse);
        let mut frame = Frame {
            coarse,
            origin,
            fine: 0,
        };
        let offset = points
            .iter()
            .map(|p| frame.point(*p))
            .flat_map(|p| p.into_iter())
            .fold(libm::scalbn(radius, -coarse), |m, x| m.max(x.abs()));
        frame.fine = exponent(offset);
        frame
    }

    /// `p` in the frame.
    pub(crate) fn point(&self, p: [f64; 2]) -> [f64; 2] {
        let p = scale(p, -self.coarse);
        scale([p[0] - self.origin[0], p[1] - self.origin[1]], -self.fine)
    }

    /// A point of the frame in the caller's coordinates.
    pub(crate) fn caller(&self, p: [f64; 2]) -> [f64; 2] {
        let p = scale(p, self.fine);
        scale([p[0] + self.origin[0], p[1] + self.origin[1]], self.coarse)
    }

    /// A caller's length in the frame.
    pub(crate) fn length(&self, l: f64) -> f64 {
        libm::scalbn(l, -(self.coarse + self.fine))
    }

    /// The power of two a frame length is multiplied by to be the caller's.
    pub(crate) fn exponent(&self) -> i32 {
        self.coarse + self.fine
    }
}

/// The `e` with `m = f·2^e`, `f ∈ [½, 1)`; 0 for 0 (or a non-finite `m`).
fn exponent(m: f64) -> i32 {
    if m > 0.0 && m.is_finite() {
        libm::frexp(m).1
    } else {
        0
    }
}

fn scale(p: [f64; 2], e: i32) -> [f64; 2] {
    [libm::scalbn(p[0], e), libm::scalbn(p[1], e)]
}

pub(crate) fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub(crate) fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

pub(crate) fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
