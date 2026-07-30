use forgecad_core::lower_forge_visual_author_source_v1;
use serde_json::{json, Value};

fn main() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
    ))
    .expect("E005-R1 fixture must be JSON");
    let lowering = lower_forge_visual_author_source_v1(&source)
        .expect("E005-R1 fixture must lower through the Rust-owned compiler");
    println!(
        "{}",
        serde_json::to_string(&json!({"source": source, "lowering": lowering})).unwrap()
    );
}
