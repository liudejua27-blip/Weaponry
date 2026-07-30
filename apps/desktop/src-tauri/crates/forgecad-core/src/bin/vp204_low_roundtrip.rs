use std::io::{self, Read};

use forgecad_core::{
    apply_forge_visual_geometry_patch_v2, VisualProgramAuthoringSessionV2,
    VisualProgramExecutionReceiptV2, VisualProgramGateOutcomeV2,
};
use serde_json::{json, Value};

fn main() {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .expect("VP204 stdin must be readable");
    let input: Value = serde_json::from_str(&raw).expect("VP204 input must be JSON");
    match input["action"].as_str().expect("VP204 action is required") {
        "patch" => {
            let result = apply_forge_visual_geometry_patch_v2(&input["source"], &input["patch"])
                .expect("VP204 patch must apply");
            println!("{}", serde_json::to_string(&result).unwrap());
        }
        "session" => {
            let initial_receipt: VisualProgramExecutionReceiptV2 =
                serde_json::from_value(input["initial_receipt"].clone())
                    .expect("initial receipt must parse");
            let mut session = VisualProgramAuthoringSessionV2::begin(
                input["session_id"].as_str().unwrap().to_string(),
                input["idempotency_key"].as_str().unwrap().to_string(),
                input["request_sha256"].as_str().unwrap().to_string(),
                input["source"].clone(),
                initial_receipt,
            )
            .expect("VP204 session must begin");
            let initial_gate: VisualProgramGateOutcomeV2 =
                serde_json::from_value(input["initial_gate"].clone())
                    .expect("initial gate must parse");
            session
                .record_gate(initial_gate)
                .expect("initial gate must apply");
            if let Some(patch) = input.get("patch") {
                let patched_receipt: VisualProgramExecutionReceiptV2 =
                    serde_json::from_value(input["patched_receipt"].clone())
                        .expect("patched receipt must parse");
                session
                    .apply_patch(patch, patched_receipt.clone())
                    .expect("one patch must apply");
                if input["replay_patch"].as_bool().unwrap_or(false) {
                    session
                        .apply_patch(patch, patched_receipt)
                        .expect("same patch replay must be idempotent");
                }
                let second_patch_error_code = input.get("second_patch").map(|second_patch| {
                    session
                        .apply_patch(second_patch, session.receipt.clone())
                        .expect_err("a conflicting second patch must fail")
                        .code()
                });
                let patched_gate: VisualProgramGateOutcomeV2 =
                    serde_json::from_value(input["patched_gate"].clone())
                        .expect("patched gate must parse");
                session
                    .record_gate(patched_gate)
                    .expect("patched gate must apply");
                let serialized = serde_json::to_value(&session).unwrap();
                let restored = VisualProgramAuthoringSessionV2::restore(&serialized)
                    .expect("session must restore");
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "session": restored,
                        "same_patch_replay_idempotent": input["replay_patch"].as_bool().unwrap_or(false),
                        "second_patch_error_code": second_patch_error_code,
                    }))
                    .unwrap()
                );
                return;
            }
            let serialized = serde_json::to_value(&session).unwrap();
            let restored = VisualProgramAuthoringSessionV2::restore(&serialized)
                .expect("session must restore");
            println!(
                "{}",
                serde_json::to_string(&json!({"session": restored})).unwrap()
            );
        }
        _ => panic!("VP204 action is unsupported"),
    }
}
