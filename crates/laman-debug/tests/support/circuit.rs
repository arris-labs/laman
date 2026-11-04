//! The exact minimum conflict set of a stretched distance, by construction
//! (plans/m7-drag-conflicts step 6): the **fundamental circuit** of that
//! edge in a pinned distance framework, computed from the graph alone and
//! never from laman's solver or residuals.
//!
//! Generically a set of distance edges is independent exactly when it is
//! (2,3)-sparse: every subset of `m` edges spans at least `(m + 3) / 2`
//! points (Laman 1970), decided by the pebble game (Jacobs and Hendrickson
//! 1997). Fixed points are one rigid body held in place, which removes the
//! three trivial motions the 3 stands for: the fixed points are joined by a
//! virtual edge that is always in. The circuit of an extra edge `e` over an
//! independent basis `B` is `e` and every `f` in `B` for which `B - f + e`
//! is independent (a basic fact of matroids). A genuine conflict has this
//! circuit as its only inclusion-minimal member containing `e`, and it is
//! the minimum one.
#![allow(dead_code)]

/// The pebble game: each point holds two pebbles, an accepted edge is
/// covered by one pebble of an end and directed out of it.
struct Game {
    pebbles: Vec<usize>,
    out: Vec<Vec<usize>>,
}

impl Game {
    fn new(points: usize) -> Self {
        Game {
            pebbles: vec![2; points],
            out: vec![Vec::new(); points],
        }
    }

    /// Bring a pebble to `start` from another point than `blocked`, by
    /// reversing the directed path that leads to it.
    fn fetch(&mut self, start: usize, blocked: usize) -> bool {
        let mut seen = vec![false; self.pebbles.len()];
        seen[start] = true;
        seen[blocked] = true;
        let mut path = Vec::new();
        if self.search(start, &mut seen, &mut path) {
            // `path` holds the points from `start` to the pebble's holder.
            let holder = path[path.len() - 1];
            self.pebbles[holder] -= 1;
            self.pebbles[start] += 1;
            for pair in path.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                if let Some(k) = self.out[from].iter().position(|&t| t == to) {
                    self.out[from].remove(k);
                }
                self.out[to].push(from);
            }
            true
        } else {
            false
        }
    }

    fn search(&self, at: usize, seen: &mut [bool], path: &mut Vec<usize>) -> bool {
        path.push(at);
        if at != path[0] && self.pebbles[at] > 0 {
            return true;
        }
        for &next in &self.out[at] {
            if !seen[next] {
                seen[next] = true;
                if self.search(next, seen, path) {
                    return true;
                }
            }
        }
        path.pop();
        false
    }

    /// Accept the edge when four pebbles gather on its ends.
    fn add(&mut self, a: usize, b: usize) -> bool {
        if a == b {
            return false;
        }
        while self.pebbles[a] < 2 {
            if !self.fetch(a, b) {
                return false;
            }
        }
        while self.pebbles[b] < 2 {
            if !self.fetch(b, a) {
                return false;
            }
        }
        self.pebbles[a] -= 1;
        self.out[a].push(b);
        true
    }
}

/// Whether the edges (pairs of point indices) are independent when the
/// points `fixed` lists are held in place.
pub fn independent(edges: &[(usize, usize)], fixed: &[usize]) -> bool {
    let points = edges
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .chain(fixed.iter().copied())
        .max()
        .map_or(0, |m| m + 1);
    let mut game = Game::new(points);
    // The fixed points are one rigid body: a chain of virtual edges.
    for pair in fixed.windows(2) {
        if !game.add(pair[0], pair[1]) {
            return false;
        }
    }
    edges.iter().all(|&(a, b)| game.add(a, b))
}

/// The indices into `basis` of the edges that, with `extra`, form its
/// fundamental circuit. `None` when `basis` is not independent or `extra`
/// is independent of it (no circuit: nothing to conflict).
pub fn fundamental_circuit(
    basis: &[(usize, usize)],
    extra: (usize, usize),
    fixed: &[usize],
) -> Option<Vec<usize>> {
    if !independent(basis, fixed) {
        return None;
    }
    let mut all = basis.to_vec();
    all.push(extra);
    if independent(&all, fixed) {
        return None;
    }
    let mut circuit = Vec::new();
    for skip in 0..basis.len() {
        let mut rest: Vec<(usize, usize)> = basis
            .iter()
            .enumerate()
            .filter(|&(k, _)| k != skip)
            .map(|(_, &e)| e)
            .collect();
        rest.push(extra);
        if independent(&rest, fixed) {
            circuit.push(skip);
        }
    }
    Some(circuit)
}

/// How far a set of distance edges is from singular at the positions `at`:
/// the smallest singular value of its rigidity matrix, rows scaled to unit
/// length so that the number is a pure geometry (an angle's sine, about),
/// over the free points' columns. Near 0 the set is at an alignment (three
/// points almost collinear), where a distance moved a little is no longer
/// realisable: the set is then infeasible at lengths close to the truth.
/// The edges must be independent, or the answer is 0.
#[allow(clippy::needless_range_loop)] // dense matrix indices read clearer than iterators
pub fn margin(at: &[(f64, f64)], edges: &[(usize, usize)], fixed: &[usize]) -> f64 {
    let free: Vec<usize> = (0..at.len()).filter(|k| !fixed.contains(k)).collect();
    let column = |p: usize| free.iter().position(|&k| k == p);
    let m = edges.len();
    let mut rows = vec![vec![0.0_f64; 2 * free.len()]; m];
    for (r, &(a, b)) in edges.iter().enumerate() {
        let (dx, dy) = (at[a].0 - at[b].0, at[a].1 - at[b].1);
        let len = libm::hypot(dx, dy);
        if len == 0.0 {
            return 0.0;
        }
        let (ux, uy) = (dx / len, dy / len);
        if let Some(c) = column(a) {
            rows[r][2 * c] += ux;
            rows[r][2 * c + 1] += uy;
        }
        if let Some(c) = column(b) {
            rows[r][2 * c] -= ux;
            rows[r][2 * c + 1] -= uy;
        }
    }
    // J Jᵀ, symmetric m by m.
    let mut g = vec![vec![0.0_f64; m]; m];
    for i in 0..m {
        for j in 0..m {
            g[i][j] = rows[i].iter().zip(&rows[j]).map(|(x, y)| x * y).sum();
        }
    }
    // Cyclic Jacobi eigenvalues; the sizes are a dozen at most.
    for _ in 0..50 {
        let mut off = 0.0;
        for i in 0..m {
            for j in 0..i {
                off += g[i][j] * g[i][j];
            }
        }
        if off < 1e-24 {
            break;
        }
        for p in 0..m {
            for q in 0..p {
                if g[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (g[q][q] - g[p][p]) / (2.0 * g[p][q]);
                let t = theta.signum() / (theta.abs() + libm::sqrt(theta * theta + 1.0));
                let c = 1.0 / libm::sqrt(t * t + 1.0);
                let s = t * c;
                for k in 0..m {
                    let (a, b) = (g[k][p], g[k][q]);
                    g[k][p] = c * a - s * b;
                    g[k][q] = s * a + c * b;
                }
                for k in 0..m {
                    let (a, b) = (g[p][k], g[q][k]);
                    g[p][k] = c * a - s * b;
                    g[q][k] = s * a + c * b;
                }
            }
        }
    }
    let lowest = (0..m).map(|i| g[i][i]).fold(f64::INFINITY, f64::min);
    libm::sqrt(lowest.max(0.0))
}
