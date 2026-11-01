//! The determinism corpus: the digest of every C1 fixture and twin, blessed
//! below. `tools/digest-wasm.sh` runs this same test natively and on
//! wasm32-wasip1 and diffs the `digest <name> <hex>` lines it prints, so the
//! fixtures are embedded, not read (a WASI run preopens no directory).
//! A change that moves a digest re-blesses it in the same commit, saying why.
//! In M0 a digest covers the loaded sketch and the checker's report at the
//! expected coordinates; M2 feeds it a `Solution`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman_debug::fixture::Fixture;
use laman_debug::runner::run;

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
        println!("digest {name} {digest}");
        if digest != blessed {
            wrong.push(name);
        }
    }
    assert!(
        wrong.is_empty(),
        "digests differ from the blessed ones: {wrong:?}"
    );
}
