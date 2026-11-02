//! The determinism corpus: the digest of every C1 fixture and twin, blessed
//! below. `tools/digest-wasm.sh` runs this same test natively and on
//! wasm32-wasip1 and diffs the `digest <name> <hex>` lines it prints, so the
//! fixtures are embedded, not read (a WASI run preopens no directory).
//! A change that moves a digest re-blesses it in the same commit, saying why.
//! A run digest covers the loaded sketch and the checker's report at the
//! expected coordinates; a solved digest covers the sketch, the `Solution`
//! from the guesses (both algorithms) and the checker's report there. A
//! solved digest is blessed only after the solve checks clean (or, for a
//! conflicting twin, names its conflict) in `c1.rs`.
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
        "9c57f391fd5b35d5544686860250558d901d6ac634c6ab74cc7325c34e49c01a",
        "6c75203c3d1ee7e03d6462683bcec2c46942e5fecb2ff4fed655f94e1098c304",
    ),
    (
        "sss-triangle-redundant",
        "b37683d8050579d2e246561c08dfbd22fd813dd9e3d2141b5521d3c5496bf346",
        "60b6c2abf04a25ec899a30b341d67c74604d94bb67f841b142b4ae4c02006f88",
    ),
    (
        "sss-triangle-conflicting",
        "b20b9a0d6383001fbcc8030d308b52efbfd247700e419476ffe63ffe3bab1b90",
        "dc5d273412f79266f4ff9c4123037488c278249a6d2e03d291153c05f48d2cec",
    ),
    (
        "belt",
        "dbcd82faf434144c0deaf6e24fd325ccd306e84a203c59db363912f94550c8d9",
        "678121e6438cfc6a015c67797c22d5c0314c42f55448503b819c064b4d238de2",
    ),
    (
        "belt-redundant",
        "b15b5b2635870ea54ec0757ca9fd4442359749d47922bfe06d288e1a3e885daa",
        "4b6fd5125dc5dd56f84aea0b28b7fc2b9a5aa89079551b74294b418e19beddd3",
    ),
    (
        "belt-conflicting",
        "f31cd6947ba89e3f39f2039f43168e96b671df1165bafc3d6d2103c648877bd4",
        "ce47596c612bd3fe9127c7dc0744384b5bb9be2afa56d91377ea9b208f16aa2b",
    ),
    (
        "slotted-plate",
        "b82d254cee2b99b56d524ee1e65c124f081a31c6d047a77c3e99b9a6afe0c2a3",
        "e76f8ab8dc05dd227979f95e7fc84029a3d8383c53ae1b2d89fc801194f2aae4",
    ),
    (
        "slotted-plate-redundant",
        "806b77a9e625b785b9778605f72fb8adaccfe260ba83372bcab37b8d63abd769",
        "175e9b53a968e356a696f1f2f21fd9f13de957e29bc779a915b8eab2057be703",
    ),
    (
        "slotted-plate-conflicting",
        "ef04cc7e1f77b39f941c2465c3b9085f7b7941b88d76c503e3476689fab89520",
        "a23db42f1e983130ce487465890f880b8f624bbd2f161829dcbbe376fcb74345",
    ),
    (
        "slotted-plate-drag",
        "20dcdeffd28a63e5067e3d6589e90848bb5d0ffe73808118fc8add6ff224bbe2",
        "37bab180a8908f5b99d4b3e9058674c7256efb1d61a6c73ebb807f12e300d6f2",
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
            let digest = solve_run(&fixture, &options).digest.to_string();
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
