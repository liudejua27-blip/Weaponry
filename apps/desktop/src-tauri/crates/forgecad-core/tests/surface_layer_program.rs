use forgecad_core::{semantic_sha256, SurfaceAdornmentProgram, SurfaceLayerProgram};
use serde_json::json;

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/surface-layer-program-fixture.json"
));

fn fixture() -> SurfaceLayerProgram {
    serde_json::from_str(FIXTURE).expect("fixture must conform to the Rust-owned contract")
}

#[test]
fn fixture_lowers_only_exact_normal_relief_to_existing_a005() {
    let program = fixture();
    program.validate().unwrap();
    assert_eq!(program.canonical_sha256().unwrap().len(), 64);

    let first = program.lower().unwrap();
    let second = program.lower().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.schema_version, "SurfaceLayerLowering@1");
    assert_eq!(first.adornments.len(), 1);
    assert!(first.adornments.iter().all(|lowered| {
        lowered.schema_version == "SurfaceAdornmentProgram@1"
            && lowered.execution == "texture_bake"
            && lowered.generator == "a005_v1"
            && lowered.non_functional_only
    }));
    assert_eq!(first.adornments[0].kind, "normal_relief");
    assert_eq!(first.adornments[0].motif, "parallel_groove");
    assert!(first
        .adornments
        .iter()
        .all(|lowered| lowered.validate().is_ok()));

    assert_eq!(first.retained_layers.vector_paths.len(), 1);
    assert_eq!(first.retained_layers.decal_layers.len(), 1);
    assert_eq!(first.retained_layers.roughness_masks.len(), 1);
    assert_eq!(first.retained_layers.emissive_masks.len(), 1);
    assert_eq!(first.retained_layers.quality_profile, "production_concept");
    assert_eq!(
        first.retained_layers_sha256,
        semantic_sha256(&first.retained_layers).unwrap()
    );
}

#[test]
fn rejects_unbounded_coordinates_duplicate_ids_and_non_a005_execution() {
    let mut invalid_coordinate = fixture();
    invalid_coordinate.vector_paths[0].commands[0].points[0] = [1.1, 0.5];
    assert_eq!(
        invalid_coordinate.validate().unwrap_err().code(),
        "SURFACE_LAYER_PROGRAM_INVALID"
    );

    let mut duplicate_id = fixture();
    duplicate_id
        .decal_layers
        .push(duplicate_id.decal_layers[0].clone());
    assert_eq!(
        duplicate_id.validate().unwrap_err().code(),
        "SURFACE_LAYER_PROGRAM_INVALID"
    );

    let mut executable_text = fixture();
    executable_text.execution = "javascript".into();
    assert_eq!(
        executable_text.lower().unwrap_err().code(),
        "SURFACE_LAYER_PROGRAM_INVALID"
    );
}

#[test]
fn serde_rejects_svg_url_path_and_script_fields() {
    let mut value = serde_json::to_value(fixture()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.insert("svg".into(), json!("<path d='M 0 0'/>"));
    object.insert(
        "source_url".into(),
        json!("https://invalid.example/texture.png"),
    );
    object.insert("file_path".into(), json!("/tmp/texture.png"));
    object.insert("script".into(), json!("run()"));
    assert!(serde_json::from_value::<SurfaceLayerProgram>(value).is_err());
}

#[test]
fn compatibility_helper_returns_only_a005_programs() {
    let lowered: Vec<SurfaceAdornmentProgram> = fixture().lower_to_surface_adornments().unwrap();
    assert_eq!(lowered.len(), 1);
    assert!(lowered
        .iter()
        .all(|program| program.execution == "texture_bake"));
}

#[test]
fn target_zone_compatibility_mapping_cannot_diverge() {
    let mut program = fixture();
    program.material_zone_id = "zone_c106_other".into();
    assert_eq!(
        program.validate().unwrap_err().code(),
        "SURFACE_LAYER_PROGRAM_INVALID"
    );
}
