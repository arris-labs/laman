//! The rigidity count against hand-counted frameworks and against Laman's
//! condition checked by brute force over every subset of points.

use laman::{RigidityError, rigidity};

#[test]
fn rigidity_triangle_is_minimally_rigid() {
    let r = rigidity(3, &[(0, 1), (1, 2), (2, 0)]).unwrap();
    assert_eq!(r.dof, 0);
    assert!(r.is_minimally_rigid());
}

#[test]
fn rigidity_square_has_one_freedom() {
    let r = rigidity(4, &[(0, 1), (1, 2), (2, 3), (3, 0)]).unwrap();
    assert_eq!(r.dof, 1);
    assert!(r.redundant.is_empty());
}

#[test]
fn rigidity_complete_four_names_its_last_bar_redundant() {
    let bars = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let r = rigidity(4, &bars).unwrap();
    assert_eq!(r.dof, 0);
    assert_eq!(r.independent, vec![0, 1, 2, 3, 4]);
    assert_eq!(r.redundant, vec![5]);
}

#[test]
fn rigidity_bowtie_hinges_at_its_shared_point() {
    // Two triangles sharing point 2: rigid each, one rotation between them.
    let bars = [(0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 2)];
    let r = rigidity(5, &bars).unwrap();
    assert_eq!(r.dof, 1);
    assert!(r.redundant.is_empty());
}

#[test]
fn rigidity_redundancy_is_local() {
    // K4 (one redundant bar) hanging off a pendant point: the excess in K4
    // does not hold the pendant, which keeps one freedom.
    let bars = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3), (3, 4)];
    let r = rigidity(5, &bars).unwrap();
    assert_eq!(r.redundant, vec![5]);
    assert_eq!(r.dof, 1);
}

#[test]
fn rigidity_repeated_bar_is_redundant() {
    let r = rigidity(2, &[(0, 1), (1, 0)]).unwrap();
    assert_eq!(r.dof, 0);
    assert_eq!(r.redundant, vec![1]);
}

#[test]
fn rigidity_counts_free_points() {
    assert_eq!(rigidity(0, &[]).unwrap().dof, 0);
    assert_eq!(rigidity(1, &[]).unwrap().dof, 0);
    assert_eq!(rigidity(2, &[]).unwrap().dof, 1);
    // A triangle and two loose points: 2 × 2 freedoms of the loose points.
    let r = rigidity(5, &[(0, 1), (1, 2), (2, 0)]).unwrap();
    assert_eq!(r.dof, 4);
}

#[test]
fn rigidity_refuses_bad_bars() {
    assert_eq!(
        rigidity(3, &[(0, 1), (1, 3)]),
        Err(RigidityError::PointOutOfRange {
            bar: 1,
            point: 3,
            points: 3
        })
    );
    assert_eq!(
        rigidity(3, &[(2, 2)]),
        Err(RigidityError::SelfLoop { bar: 0, point: 2 })
    );
    assert_eq!(
        rigidity(usize::MAX, &[]),
        Err(RigidityError::TooManyPoints { points: usize::MAX })
    );
}

#[test]
fn rigidity_memory_follows_the_bars() {
    // A huge point count with few bars: the game runs over the named points.
    let n = usize::MAX / 2;
    let r = rigidity(n, &[(0, n - 1), (n - 1, 7), (7, 0)]).unwrap();
    assert_eq!(r.dof, 2 * n - 3 - 3);
}

/// Whether `bars` over `n` points satisfies Laman's sparsity condition:
/// every subset of `k ≥ 2` points spans at most `2k − 3` bars.
fn laman_sparse(n: usize, bars: &[(usize, usize)]) -> bool {
    (0u32..1 << n).all(|subset| {
        let k = subset.count_ones() as usize;
        let inside = bars
            .iter()
            .filter(|(a, b)| subset & (1 << a) != 0 && subset & (1 << b) != 0)
            .count();
        k < 2 || inside <= 2 * k - 3
    })
}

/// A small deterministic generator, so the corpus is the same every run.
struct XorShift(u64);
impl XorShift {
    fn below(&mut self, m: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % m as u64) as usize
    }
}

#[test]
fn rigidity_matches_greedy_laman_sparsity() {
    let mut rng = XorShift(0x5eed_1a3a_2026_1101);
    for _ in 0..1000 {
        let n = 2 + rng.below(6);
        let m = rng.below(3 * n);
        let bars: Vec<(usize, usize)> = (0..m)
            .map(|_| {
                let a = rng.below(n);
                (a, (a + 1 + rng.below(n - 1)) % n)
            })
            .collect();

        // The greedy basis by brute force: keep a bar when the kept set
        // stays sparse with it.
        let mut kept: Vec<(usize, usize)> = Vec::new();
        let mut expected = Vec::new();
        for (i, &bar) in bars.iter().enumerate() {
            kept.push(bar);
            if laman_sparse(n, &kept) {
                expected.push(i);
            } else {
                kept.pop();
            }
        }

        let r = rigidity(n, &bars).unwrap();
        assert_eq!(r.independent, expected, "n = {n}, bars = {bars:?}");
        assert_eq!(r.independent.len() + r.redundant.len(), m);
        assert_eq!(r.dof, 2 * n - expected.len() - 3, "n = {n}, bars = {bars:?}");
    }
}
