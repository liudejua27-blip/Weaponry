use forgecad_core::lower_forge_visual_geometry_program_v2;
use serde_json::{json, Value};

fn main() {
    let fixtures = [
        ("bracket", include_str!("../../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-bracket.json")),
        ("rotor", include_str!("../../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-rotor.json")),
        ("duct", include_str!("../../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-duct.json")),
    ];
    let results = fixtures
        .into_iter()
        .map(|(fixture_id, raw)| {
            let source: Value = serde_json::from_str(raw).expect("VP203 fixture must be JSON");
            let lowering = lower_forge_visual_geometry_program_v2(&source)
                .expect("VP203 fixture must lower through Rust");
            json!({"fixture_id": fixture_id, "lowering": lowering})
        })
        .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::to_string(&json!({"results": results})).unwrap()
    );
}
