use forgecad_core::expand_and_lower_forge_visual_composition_v2;
use serde_json::Value;

fn main() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../../../../../packages/concept-spec/fixtures/forge-visual-composition-v2-repeat.json"
    ))
    .expect("VP202 fixture must be valid JSON");
    let result = expand_and_lower_forge_visual_composition_v2(&source)
        .expect("VP202 fixture must expand and lower through VP201");
    println!(
        "{}",
        serde_json::to_string(&result).expect("VP202 result must serialize")
    );
}
