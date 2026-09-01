//! Rust-side boundary for Weaponry's fixed Blender knife prototype.
//!
//! The first shipped prototype is intentionally one closed vertical-slice
//! operation. It covers the high/low/UV/bake recipe in one typed job; later
//! per-stage operations must be added as explicit protocol successors, not by
//! opening a generic script or add-on bridge.

pub mod knife;

pub use knife::{
    canonical_json_bytes, canonical_json_hash, validate_request, validate_response,
    KnifeBlenderInstall, KnifeBlenderWorker, KnifeBudgets, KnifeInputGlb, KnifePolicies,
    KnifeWorkerError, KnifeWorkerErrorEnvelope, KnifeWorkerRequest, KnifeWorkerResponse,
    KnifeWorkerRun, KNIFE_BLENDER_EXECUTABLE_SHA256, KNIFE_BLENDER_REVISION, KNIFE_BLENDER_VERSION,
    KNIFE_DEPENDENCY_LOCK_SHA256, KNIFE_ENTRYPOINT_RELATIVE_PATH, KNIFE_INPUT_RELATIVE_PATH,
    KNIFE_MAX_INPUT_BYTES, KNIFE_MAX_MEMORY_BYTES, KNIFE_MAX_OBJECTS, KNIFE_MAX_OUTPUT_BYTES,
    KNIFE_MAX_REQUEST_BYTES, KNIFE_MAX_RUNTIME_MS, KNIFE_MAX_STDOUT_BYTES, KNIFE_MAX_TRIANGLES,
    KNIFE_OPERATION, KNIFE_PACKAGED_BLENDER_RELATIVE_PATH, KNIFE_PACKAGED_ENTRYPOINT_RELATIVE_PATH,
    KNIFE_PACKAGED_MANIFEST_RELATIVE_PATH, KNIFE_PACKAGED_MANIFEST_SCHEMA,
    KNIFE_PACKAGED_SOURCE_MANIFEST_RELATIVE_PATH, KNIFE_RECIPE_ID, KNIFE_RECIPE_SHA256,
    KNIFE_REPOSITORY_BLENDER_RELATIVE_PATH, KNIFE_REQUEST_SCHEMA, KNIFE_RESPONSE_SCHEMA,
    KNIFE_RESULT_SCHEMA, KNIFE_SOURCE_MANIFEST_RELATIVE_PATH, KNIFE_SOURCE_MANIFEST_SCHEMA,
    KNIFE_TEXTURE_SIZE, KNIFE_WORKER_ID, KNIFE_WORKER_PROTOCOL, KNIFE_WORKER_VERSION,
};
