use forgecad_core::lower_forge_visual_program_v2;
use serde_json::Value;

fn main() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../../../packages/concept-spec/fixtures/forge-visual-program-v2-minimal.json"
    ))
    .expect("VP201 fixture must be valid JSON");
    let lowering = lower_forge_visual_program_v2(&source)
        .expect("VP201 fixture must pass Rust validation and lowering");
    println!(
        "{}",
        serde_json::to_string(&lowering).expect("VP201 lowering must serialize")
    );
}
