//! The determinism corpus: the digest of every C1 fixture and twin, blessed
//! below. `tools/digest-wasm.sh` runs this same test natively and on
//! wasm32-wasip1 and diffs the `digest <name> <hex>` lines it prints, so the
//! fixtures are embedded, not read (a WASI run preopens no directory).
//! A change that moves a digest re-blesses it in the same commit, saying why.
//! A run digest covers the loaded sketch and the checker's report at the
//! expected coordinates; a solved digest covers the sketch, the `Solution`
//! from the guesses (both algorithms) and the checker's report there. A
//! solved digest is blessed only for a run the checker passes: clean, or for
//! a conflicting twin exactly the violations `c1.rs` derives. The test below
//! holds that itself before it compares a digest.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::solution::{Algorithm, SolverOptions};
use laman_debug::fixture::Fixture;
use laman_debug::runner::{run, solve_run};

/// Name, sketch.json, expected.json, blessed digest.
const CORPUS: [(&str, &str, &str, &str); 10] = [
    (
        "sss-triangle",
        include_str!("../../../tests/fixtures/c1/sss-triangle/sketch.json"),
        include_str!("../../../tests/fixtures/c1/sss-triangle/expected.json"),
        "efcf7703c0f9224421196fb1d3bf648e422d1dfd90a53031fbbef646eb739a23",
    ),
    (
        "sss-triangle-redundant",
        include_str!("../../../tests/fixtures/c1/sss-triangle-redundant/sketch.json"),
        include_str!("../../../tests/fixtures/c1/sss-triangle-redundant/expected.json"),
        "692229b059271668d7a03998f4d9538f3abd416915266d13f42847e62ff28fb7",
    ),
    (
        "sss-triangle-conflicting",
        include_str!("../../../tests/fixtures/c1/sss-triangle-conflicting/sketch.json"),
        include_str!("../../../tests/fixtures/c1/sss-triangle-conflicting/expected.json"),
        "a907e5dff9d0af59311baee7813b33ee4000d388ff79fd759e993c937fb0267d",
    ),
    (
        "belt",
        include_str!("../../../tests/fixtures/c1/belt/sketch.json"),
        include_str!("../../../tests/fixtures/c1/belt/expected.json"),
        "9f27a6f9b1e68c5415d63a7408b4ec887b59b744972a21bb5b06e73f26d38ee6",
    ),
    (
        "belt-redundant",
        include_str!("../../../tests/fixtures/c1/belt-redundant/sketch.json"),
        include_str!("../../../tests/fixtures/c1/belt-redundant/expected.json"),
        "c0ddbb6beb64ece6228732a13a8909fb203b3819f0e8ce8a68697d04899cce2d",
    ),
    (
        "belt-conflicting",
        include_str!("../../../tests/fixtures/c1/belt-conflicting/sketch.json"),
        include_str!("../../../tests/fixtures/c1/belt-conflicting/expected.json"),
        "579c0fe2f71433b9be3e2d4dba5ecfc3bbd09d3f2f76d30d58c95379a6f638e0",
    ),
    (
        "slotted-plate",
        include_str!("../../../tests/fixtures/c1/slotted-plate/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate/expected.json"),
        "3a29d8c6e3c19ebc6409e1c504791710e4ecc81a77ebe49c3f77671edd7a5790",
    ),
    (
        "slotted-plate-redundant",
        include_str!("../../../tests/fixtures/c1/slotted-plate-redundant/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate-redundant/expected.json"),
        "4b5314fb48b3ab3b4eb8d719cf2c68cd5222268ae8fe45f3c282b75d45515560",
    ),
    (
        "slotted-plate-conflicting",
        include_str!("../../../tests/fixtures/c1/slotted-plate-conflicting/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate-conflicting/expected.json"),
        "d981003c496b7f4e763db9c28e0f9345b10bd27a78b4fb400be4e5533a74cc29",
    ),
    (
        "slotted-plate-drag",
        include_str!("../../../tests/fixtures/c1/slotted-plate-drag/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate-drag/expected.json"),
        "0aac49453654d9746b61df5ee59a1a3b29b66342213fcc8447d3ba54a8cdb822",
    ),
];

#[test]
fn corpus_digests_match_the_blessed_ones() {
    let mut wrong = Vec::new();
    for (name, sketch, expected, blessed) in CORPUS {
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        let digest = run(&fixture).digest.to_string();
        println!("\ndigest {name} {digest}");
        if digest != blessed {
            wrong.push(name);
        }
    }
    assert!(
        wrong.is_empty(),
        "digests differ from the blessed ones: {wrong:?}"
    );
}

/// Name, blessed digest of the solve by Levenberg-Marquardt, by DogLeg.
const SOLVED: [(&str, &str, &str); 10] = [
    (
        "sss-triangle",
        "fd6c58a9f0b9c42204a9f7b8df7d006f76b8e2f4cac57ff5b17d49ce0c1c954e",
        "10cdebe3cdb191f37011f378d0fa7d97565c6a935afcbdc57b80183d46f53d90",
    ),
    (
        "sss-triangle-redundant",
        "85cb1a0445c4990713e48e25345aec58c2db060e383db1190c3ab58614d6a8f6",
        "aabc782d2920824792fed187619b2a47bb3a1182b682518bfa17845a03806001",
    ),
    (
        "sss-triangle-conflicting",
        "4c4d4d30eed7c174bb8292ab910748457bf3d60acbf5d6ebb4137a508a888784",
        "94ce90a09019840c83cc2345777d98069a26a2e18315a4af7716e29b30e2355b",
    ),
    (
        "belt",
        "89ea22612d7d37bd56168384654ecc2bdbf73448c79ec2100f7ea9872a595cb2",
        "5f7b4f5290f3dab65fbd4401d2ab9b261d2e592c84feb1229222fb21019190af",
    ),
    (
        "belt-redundant",
        "a4f84fa0e20d5f291cd92ae3ed00fa1a1d13ab6b1779037813216aac05b53124",
        "c0d983232c873cb1bc18ec50fb1414ccbb9e198c294d67d5084facb6649ca9fd",
    ),
    (
        "belt-conflicting",
        "5f6e83eb80ca6ab4d9fb6c81917b80a05ffad18a8bcb4fa91c74c1e0c837bb1a",
        "84098a75a643d8e8eda496b212633bd29fca0de5f7c7b2a886e3f69d2b2ad722",
    ),
    (
        "slotted-plate",
        "e313fce60bc40ce6baad58ba3045acae9bee2714d22c0a30d7dc1c954379c9e7",
        "bdf25b2c79c24061ce86d94e5fa407d947235fdac8cd2efe155b2b4e09224c44",
    ),
    (
        "slotted-plate-redundant",
        "7b4289630e39d9492f46d215e4e65423cc8168021c0769fc08a98d94a59c2365",
        "ede797d52d5b79d56733f7cde0ad90ae7caafd41d9dc5f509adeaac1372a5813",
    ),
    (
        "slotted-plate-conflicting",
        "3c3d7400a99ae4e2c48bfeb65d982d1a747a6df4cdfc7c9fa75c1bb0d00bc827",
        "3634f460017a1f0fd6036637a6fa8716dead94703d682593b28e5d408b9a33a7",
    ),
    (
        "slotted-plate-drag",
        "3c2e2a7f979006106fec7844376658448c2f8e43d09422de6a4d2c9c99602f4b",
        "d43da1e1e38b9bdf6a6778c9e6788e49ba7fa7a70c731ce1f7560045dd6dfaaf",
    ),
];

/// The violations the checker must report at a conflicting twin's best
/// effort, sorted (derived by analysis in `c1.rs`); every other run is clean.
const BEST_EFFORT_VIOLATIONS: [(&str, &[&str]); 3] = [
    (
        "sss-triangle-conflicting",
        &["apex", "base", "left", "right"],
    ),
    (
        "belt-conflicting",
        &[
            "r1",
            "r2",
            "span",
            "top-length",
            "top-on-c1",
            "top-on-c2",
            "top-tangent-c1",
            "top-tangent-c2",
        ],
    ),
    (
        "slotted-plate-conflicting",
        &["same-ends", "slot-r", "slot-r2"],
    ),
];

#[test]
fn solved_digests_match_the_blessed_ones() {
    let mut wrong = Vec::new();
    for (name, sketch, expected, _) in CORPUS {
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        let (_, lm, dog) = SOLVED.iter().find(|row| row.0 == name).unwrap();
        for (algorithm, label, blessed) in [
            (Algorithm::LevenbergMarquardt, "lm", lm),
            (Algorithm::DogLeg, "dogleg", dog),
        ] {
            let mut options = SolverOptions::default();
            options.algorithm = algorithm;
            let solved = solve_run(&fixture, &options);
            // Bless only checker-validated runs: clean, or for a conflicting
            // twin the violations derived by analysis in `c1.rs`.
            let mut flagged: Vec<String> = solved
                .report
                .violations
                .iter()
                .map(|v| v.tag.clone().unwrap_or_default())
                .collect();
            flagged.sort();
            let want = BEST_EFFORT_VIOLATIONS
                .iter()
                .find(|row| row.0 == name)
                .map_or(&[][..], |row| row.1);
            assert_eq!(flagged, want, "{label} {name}: the checker's verdict");
            assert!(solved.report.degenerate.is_empty(), "{label} {name}");
            assert!(solved.report.missing.is_empty(), "{label} {name}");
            let digest = solved.digest.to_string();
            println!("\ndigest solved-{label}-{name} {digest}");
            if digest != *blessed {
                wrong.push(format!("{label} {name}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "solved digests differ from the blessed ones: {wrong:?}"
    );
}
