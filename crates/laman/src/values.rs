//! Where a parameter's value is read from: a [`Values`] trait over the map
//! the public API carries ([`Coordinates`]) and [`Dense`], a flat table of
//! the same numbers for the paths a frame runs many times
//! (plans/m8-dense-rank step 6).
//!
//! A read never depends on the route it takes: [`Dense::get`] returns what
//! the map it was made from returns, for every key, a key the table lacks
//! reading `None` (`dense_reads_what_the_map_reads`).

use crate::model::{Coordinate, Coordinates};
use crate::residual::Param;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A source of parameter values: `None` for a parameter it lacks, the value
/// as stored (non-finite included) otherwise.
pub(crate) trait Values {
    /// The value of `p`, if this source holds one.
    fn get(&self, p: Param) -> Option<f64>;
}

impl Values for Coordinates {
    fn get(&self, p: Param) -> Option<f64> {
        BTreeMap::get(self, &p).copied()
    }
}

/// How many ids a [`Layout`]'s direct table covers per entity: a table
/// indexed by raw id would allocate by the largest id the caller chose
/// (`from_form` takes any `u32`, and a removal leaves a hole), so ids below
/// `DIRECT_FACTOR` times the entity count are indexed directly and the rest
/// are searched in a sorted vector. Four covers a sketch that has lost three
/// quarters of its entities; the generated families have no holes (ratio 1),
/// and the table costs 12 bytes per covered id at most.
const DIRECT_FACTOR: u64 = 4;

/// The coordinates a parameter has: `X`, `Y` and `R`.
const KINDS: usize = 3;

/// A direct table entry whose parameter is absent.
const ABSENT: u32 = u32::MAX;

/// Where each parameter of a sketch sits in a [`Dense`] table. Shared by
/// every table made from it, so a clone of a table is one `memcpy`.
#[derive(Debug)]
struct Layout {
    /// The slot of `(id, kind)` at `id * KINDS + kind`, for the ids the
    /// direct table covers.
    direct: Vec<u32>,
    /// The ids past the direct table's span: parameter and slot, sorted.
    spill: Vec<(Param, u32)>,
    /// Every parameter, in slot order (the map's key order).
    params: Vec<Param>,
}

fn kind(c: Coordinate) -> usize {
    match c {
        Coordinate::X => 0,
        Coordinate::Y => 1,
        Coordinate::R => 2,
    }
}

impl Layout {
    fn of(keys: &[Param]) -> Self {
        let mut ids: Vec<u32> = keys.iter().map(|p| p.0.0).collect();
        ids.dedup();
        let span = (ids.len() as u64)
            .saturating_mul(DIRECT_FACTOR)
            .min(ids.last().map_or(0, |m| u64::from(*m) + 1));
        let mut direct = vec![ABSENT; (span as usize).saturating_mul(KINDS)];
        let mut spill = Vec::new();
        for (slot, p) in keys.iter().enumerate() {
            let slot = slot as u32;
            if u64::from(p.0.0) < span {
                if let Some(cell) = direct.get_mut(p.0.0 as usize * KINDS + kind(p.1)) {
                    *cell = slot;
                }
            } else {
                spill.push((*p, slot));
            }
        }
        Self {
            direct,
            spill,
            params: keys.to_vec(),
        }
    }

    fn slot(&self, p: Param) -> Option<usize> {
        // In u64: `id * KINDS` overflows a 32-bit usize for the largest ids.
        let at = u64::from(p.0.0) * KINDS as u64 + kind(p.1) as u64;
        if at < self.direct.len() as u64 {
            let slot = *self.direct.get(at as usize)?;
            return (slot != ABSENT).then_some(slot as usize);
        }
        let i = self.spill.binary_search_by_key(&p, |(q, _)| *q).ok()?;
        self.spill.get(i).map(|(_, s)| *s as usize)
    }
}

/// The values of a set of parameters as a flat table: the numbers a
/// [`Coordinates`] holds, read without a tree search. A clone shares the
/// layout. Setting a parameter the table lacks is refused, so the layout
/// never changes under a clone.
#[derive(Clone, Debug)]
pub(crate) struct Dense {
    layout: Arc<Layout>,
    values: Vec<f64>,
}

impl Dense {
    /// The table of `map`: its keys, in the map's order, with its values.
    pub(crate) fn of(map: &Coordinates) -> Self {
        let keys: Vec<Param> = map.keys().copied().collect();
        Self {
            layout: Arc::new(Layout::of(&keys)),
            values: map.values().copied().collect(),
        }
    }

    /// Replaces `p`'s value; `false` (and nothing changes) when the table
    /// has no such parameter.
    pub(crate) fn set(&mut self, p: Param, v: f64) -> bool {
        match self.layout.slot(p).and_then(|s| self.values.get_mut(s)) {
            Some(cell) => {
                *cell = v;
                true
            }
            None => false,
        }
    }

    /// The table as the map the public API carries.
    pub(crate) fn to_coordinates(&self) -> Coordinates {
        self.layout
            .params
            .iter()
            .copied()
            .zip(self.values.iter().copied())
            .collect()
    }
}

impl Values for Dense {
    fn get(&self, p: Param) -> Option<f64> {
        self.layout
            .slot(p)
            .and_then(|s| self.values.get(s))
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntityId, Sketch};
    use proptest::prelude::*;

    fn id() -> impl Strategy<Value = u32> {
        prop_oneof![
            Just(0_u32),
            Just(u32::MAX),
            Just(u32::MAX - 1),
            0..40_u32,
            any::<u32>(),
        ]
    }

    fn coordinate() -> impl Strategy<Value = Coordinate> {
        prop_oneof![
            Just(Coordinate::X),
            Just(Coordinate::Y),
            Just(Coordinate::R)
        ]
    }

    fn value() -> impl Strategy<Value = f64> {
        prop_oneof![
            -1e300..1e300_f64,
            Just(f64::NAN),
            Just(f64::INFINITY),
            Just(f64::NEG_INFINITY),
            Just(-0.0),
            Just(f64::MAX),
        ]
    }

    proptest! {
        // A table reads what the map it was made from reads, bit for bit,
        // on every key the map has and on keys it lacks, whichever route an
        // id takes.
        #[test]
        fn dense_reads_what_the_map_reads(
            entries in proptest::collection::vec(((id(), coordinate()), value()), 0..60),
            probes in proptest::collection::vec((id(), coordinate()), 0..60),
        ) {
            let map: Coordinates = entries
                .into_iter()
                .map(|((i, c), v)| ((EntityId(i), c), v))
                .collect();
            let dense = Dense::of(&map);
            let keys = map.keys().copied().chain(probes.into_iter().map(|(i, c)| (EntityId(i), c)));
            for key in keys {
                let want = Values::get(&map, key).map(f64::to_bits);
                prop_assert_eq!(Values::get(&dense, key).map(f64::to_bits), want);
            }
            prop_assert_eq!(dense.to_coordinates().len(), map.len());
        }
    }

    #[test]
    fn dense_set_changes_only_a_parameter_it_has() {
        let mut map = Coordinates::new();
        map.insert((EntityId(0), Coordinate::X), 1.0);
        map.insert((EntityId(u32::MAX), Coordinate::Y), 2.0);
        let mut dense = Dense::of(&map);
        let copy = dense.clone();
        assert!(dense.set((EntityId(u32::MAX), Coordinate::Y), 5.0));
        assert!(!dense.set((EntityId(0), Coordinate::Y), 5.0));
        assert_eq!(
            Values::get(&dense, (EntityId(u32::MAX), Coordinate::Y)),
            Some(5.0)
        );
        assert_eq!(
            Values::get(&copy, (EntityId(u32::MAX), Coordinate::Y)),
            Some(2.0)
        );
        assert_eq!(Values::get(&dense, (EntityId(0), Coordinate::Y)), None);
    }

    #[test]
    fn a_sketch_with_one_huge_id_allocates_by_its_entity_count() {
        let mut sk = Sketch::new();
        sk.point_with_id(EntityId(0), 1.0, 2.0).unwrap();
        sk.point_with_id(EntityId(u32::MAX), 3.0, 4.0).unwrap();
        let map = sk.coordinates();
        let dense = Dense::of(&map);
        assert!(dense.layout.direct.len() <= 2 * DIRECT_FACTOR as usize * KINDS);
        assert_eq!(dense.layout.spill.len(), 2);
        for (key, v) in &map {
            assert_eq!(Values::get(&dense, *key), Some(*v));
        }
    }

    #[test]
    fn holes_after_a_removal_read_as_the_map_does() {
        let mut sk = Sketch::new();
        let points: Vec<_> = (0..20)
            .map(|i| sk.point(f64::from(i), 0.0).unwrap())
            .collect();
        for p in points.iter().step_by(2) {
            sk.remove(p.id()).unwrap();
        }
        let map = sk.coordinates();
        let dense = Dense::of(&map);
        for i in 0..25 {
            for c in [Coordinate::X, Coordinate::Y, Coordinate::R] {
                let key = (EntityId(i), c);
                assert_eq!(Values::get(&dense, key), Values::get(&map, key));
            }
        }
    }
}
