//! Generic rigidity of bar frameworks in the plane, by the (2,3) pebble game.
//!
//! Laman's theorem (G. Laman, *On graphs and rigidity of plane skeletal
//! structures*, 1970): for points in generic position, a set of bars is
//! independent exactly when every subset of `k ≥ 2` points spans at most
//! `2k − 3` of its bars. The independent sets are those of a matroid, so a
//! greedy pass over the bars in the caller's order finds a maximal
//! independent set; the bars it rejects are the redundant ones. The pass is
//! the pebble game of D. Jacobs and B. Hendrickson, *An algorithm for
//! two-dimensional rigidity percolation: the pebble game* (J. Comput.
//! Phys., 1997), implemented from the paper.
//!
//! Everything here is combinatorial: no floating point, the same answer on
//! every platform.

use std::collections::BTreeMap;

/// Free pebbles a point starts with: its two freedoms in the plane.
const PEBBLES_PER_POINT: u8 = 2;
/// Pebbles that must sit on a bar's two ends for it to be independent:
/// `l + 1` for the (2, `l` = 3) pebble game.
const PEBBLES_TO_ACCEPT: u8 = 4;

/// A bar names a point the framework does not have, or a point twice.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RigidityError {
    /// `bars[bar]` names `point`, but the framework has only `points` points.
    #[error("bar {bar} names point {point}, but the framework has {points} points")]
    PointOutOfRange {
        /// Index of the bar in the caller's slice.
        bar: usize,
        /// The point it names.
        point: usize,
        /// The number of points in the framework.
        points: usize,
    },
    /// `bars[bar]` joins `point` to itself.
    #[error("bar {bar} joins point {point} to itself")]
    SelfLoop {
        /// Index of the bar in the caller's slice.
        bar: usize,
        /// The point at both of its ends.
        point: usize,
    },
    /// The freedom count `2 × points` does not fit in a `usize`.
    #[error("{points} points have more freedoms than a usize holds")]
    TooManyPoints {
        /// The number of points asked for.
        points: usize,
    },
}

/// The generic rigidity of a bar framework.
///
/// "Generic" means for points in general position: a framework this
/// reports rigid can still flex at special positions (three collinear
/// points), never the other way round.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Rigidity {
    /// The number of points in the framework.
    pub points: usize,
    /// Indices of the bars that are independent, ascending. The first
    /// maximal independent set in the caller's order.
    pub independent: Vec<usize>,
    /// Indices of the bars that are redundant given the bars before them,
    /// ascending. Each one could be removed without the framework gaining
    /// a freedom.
    pub redundant: Vec<usize>,
    /// Remaining internal degrees of freedom: the freedoms of the points
    /// minus those the independent bars remove, minus the motions of the
    /// whole framework (2 for one point, 3 for two or more).
    pub dof: usize,
}

impl Rigidity {
    /// Whether the framework is generically rigid (no internal freedom).
    pub fn is_rigid(&self) -> bool {
        self.dof == 0
    }

    /// Whether the framework is rigid with no redundant bar: `2n − 3` bars
    /// for `n ≥ 2` points, a Laman graph.
    pub fn is_minimally_rigid(&self) -> bool {
        self.is_rigid() && self.redundant.is_empty()
    }
}

/// The generic rigidity of `points` points joined by `bars`, each bar a
/// pair of point indices in `0..points`.
///
/// Bars may repeat (a repeat is redundant). Points no bar names are free
/// and count toward [`Rigidity::dof`]. The result depends only on the
/// input, the order of `bars` included: of several redundant bars that
/// could be dropped, the later ones are named.
///
/// # Errors
///
/// [`RigidityError`] for a bar naming a point out of range or the same
/// point twice, and for a point count whose freedoms overflow a `usize`.
///
/// # Example
///
/// ```
/// // A square flexes: one internal freedom. Its diagonal makes it rigid.
/// let square = [(0, 1), (1, 2), (2, 3), (3, 0)];
/// assert_eq!(laman::rigidity(4, &square).unwrap().dof, 1);
/// let braced = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)];
/// assert!(laman::rigidity(4, &braced).unwrap().is_minimally_rigid());
/// ```
pub fn rigidity(points: usize, bars: &[(usize, usize)]) -> Result<Rigidity, RigidityError> {
    let freedoms = points
        .checked_mul(usize::from(PEBBLES_PER_POINT))
        .ok_or(RigidityError::TooManyPoints { points })?;

    // The game runs over the points the bars name only, numbered in
    // ascending order, so its memory follows the bars and not `points`.
    let mut slot: BTreeMap<usize, usize> = BTreeMap::new();
    for (bar, &(a, b)) in bars.iter().enumerate() {
        for point in [a, b] {
            if point >= points {
                return Err(RigidityError::PointOutOfRange { bar, point, points });
            }
        }
        if a == b {
            return Err(RigidityError::SelfLoop { bar, point: a });
        }
        slot.insert(a, 0);
        slot.insert(b, 0);
    }
    for (i, s) in slot.values_mut().enumerate() {
        *s = i;
    }

    let mut game = PebbleGame::new(slot.len());
    let mut independent = Vec::new();
    let mut redundant = Vec::new();
    for (bar, (a, b)) in bars.iter().enumerate() {
        let (Some(&u), Some(&v)) = (slot.get(a), slot.get(b)) else {
            // Every endpoint was given a slot above.
            continue;
        };
        if game.insert(u, v) {
            independent.push(bar);
        } else {
            redundant.push(bar);
        }
    }

    let trivial = match points {
        0 => 0,
        1 => 2,
        _ => 3,
    };
    // An independent set holds at most 2n − 3 bars, so this never saturates
    // for n ≥ 2; it is written saturating so no input can make it panic.
    let dof = freedoms
        .saturating_sub(independent.len())
        .saturating_sub(trivial);
    Ok(Rigidity {
        points,
        independent,
        redundant,
        dof,
    })
}

/// The (2,3) pebble game over points `0..n`.
///
/// Each point holds up to two free pebbles; each accepted bar is covered by
/// one pebble of one of its ends and directed away from it (`out[x]` holds
/// the far ends of the bars `x` covers). Ordered containers and a fixed
/// search order keep every run identical.
struct PebbleGame {
    pebbles: Vec<u8>,
    out: Vec<Vec<usize>>,
}

impl PebbleGame {
    fn new(n: usize) -> Self {
        Self {
            pebbles: vec![PEBBLES_PER_POINT; n],
            out: vec![Vec::new(); n],
        }
    }

    /// Inserts the bar `u`–`v` if it is independent of the bars accepted so
    /// far; returns whether it was.
    fn insert(&mut self, u: usize, v: usize) -> bool {
        while self.pebbles[u] < PEBBLES_PER_POINT && self.fetch(u, v) {}
        while self.pebbles[v] < PEBBLES_PER_POINT && self.fetch(v, u) {}
        if self.pebbles[u] + self.pebbles[v] < PEBBLES_TO_ACCEPT {
            return false;
        }
        self.pebbles[u] -= 1;
        self.out[u].push(v);
        true
    }

    /// Brings one free pebble to `root` along a directed path that avoids
    /// `held` (whose pebbles are being kept), reversing the path's bars.
    /// Returns false when no free pebble is reachable.
    fn fetch(&mut self, root: usize, held: usize) -> bool {
        let n = self.pebbles.len();
        let mut parent: Vec<Option<usize>> = vec![None; n];
        let mut seen = vec![false; n];
        seen[root] = true;
        seen[held] = true;
        let mut stack = vec![root];
        let mut found = None;
        'search: while let Some(x) = stack.pop() {
            for &y in &self.out[x] {
                if seen[y] {
                    continue;
                }
                seen[y] = true;
                parent[y] = Some(x);
                if self.pebbles[y] > 0 {
                    found = Some(y);
                    break 'search;
                }
                stack.push(y);
            }
        }
        let Some(found) = found else {
            return false;
        };
        let mut y = found;
        while let Some(x) = parent[y] {
            self.reverse(x, y);
            y = x;
        }
        self.pebbles[found] -= 1;
        self.pebbles[root] += 1;
        true
    }

    /// Turns the bar `x → y` into `y → x`: `y`'s pebble covers it now.
    fn reverse(&mut self, x: usize, y: usize) {
        if let Some(i) = self.out[x].iter().position(|&h| h == y) {
            self.out[x].remove(i);
            self.out[y].push(x);
        }
    }
}
