use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::*;

const FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/k002-native-agent-protocol.json"
));
const K001_MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/forgecad-app-server-protocol-manifest.json"
));
const K001_TURN_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../packages/concept-spec/fixtures/k001-a004-turn-compatibility.json"
));

fn parse_example<T: DeserializeOwned>(fixture: &Value, name: &str) -> T {
    serde_json::from_value(fixture["examples"][name].clone())
        .unwrap_or_else(|error| panic!("invalid K002 {name} fixture: {error}"))
}

#[test]
fn k002_fixed_fixture_round_trips_every_native_contract() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(
        fixture["schema_version"],
        "K002NativeAgentProtocolFixture@1"
    );
    assert_eq!(fixture["protocol_version"], FORGECAD_PROTOCOL_VERSION);

    let ownership: MigrationOwnership =
        serde_json::from_value(fixture["ownership"].clone()).unwrap();
    ownership.validate().unwrap();

    parse_example::<ThreadCommand>(&fixture, "thread_command")
        .validate()
        .unwrap();
    parse_example::<TurnCommand>(&fixture, "turn_command")
        .validate()
        .unwrap();
    parse_example::<ItemCommand>(&fixture, "item_command")
        .validate()
        .unwrap();
    parse_example::<ApprovalCommand>(&fixture, "approval_command")
        .validate()
        .unwrap();
    parse_example::<ProviderPreflightCommand>(&fixture, "provider_preflight_command")
        .validate()
        .unwrap();
    parse_example::<ProviderPreflightResult>(&fixture, "provider_preflight_result")
        .validate()
        .unwrap();
    parse_example::<ProviderCheckCommand>(&fixture, "provider_check_command")
        .validate()
        .unwrap();
    parse_example::<ProviderCheckResult>(&fixture, "provider_check_result")
        .validate()
        .unwrap();
    parse_example::<ProviderCancelCommand>(&fixture, "provider_cancel_command")
        .validate()
        .unwrap();
    parse_example::<ProviderCancelResult>(&fixture, "provider_cancel_result")
        .validate()
        .unwrap();
    parse_example::<ProductToolExecutionRequest>(&fixture, "product_tool_request")
        .validate()
        .unwrap();
    parse_example::<ProductToolExecutionResult>(&fixture, "product_tool_result")
        .validate()
        .unwrap();
    parse_example::<LifecyclePersistenceCommand>(&fixture, "persistence_command")
        .validate()
        .unwrap();
    parse_example::<LifecyclePersistenceResult>(&fixture, "persistence_result")
        .validate()
        .unwrap();
    let notification = parse_example::<NativeAgentNotification>(&fixture, "native_notification");
    notification.validate().unwrap();
    assert_eq!(notification.method(), NOTIFICATION_ITEM_UPDATED);
}

#[test]
fn k002_method_registry_is_fixed_and_k001_golden_is_unchanged() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let request_methods = fixture["request_methods"].as_array().unwrap();
    for method in [
        METHOD_THREAD_CREATE,
        METHOD_THREAD_LIST,
        METHOD_THREAD_READ,
        METHOD_THREAD_ARCHIVE,
        METHOD_TURN_START,
        METHOD_TURN_READ,
        METHOD_TURN_CANCEL,
        METHOD_ITEM_LIST,
        METHOD_ITEM_READ,
        METHOD_APPROVAL_CREATE,
        METHOD_APPROVAL_READ,
        METHOD_APPROVAL_RESOLVE,
        METHOD_PROVIDER_PREFLIGHT,
        METHOD_PROVIDER_CHECK,
        METHOD_PROVIDER_CANCEL,
        METHOD_PRODUCT_TOOLS_LIST,
        METHOD_PRODUCT_TOOLS_EXECUTE,
        METHOD_LIFECYCLE_PERSISTENCE_EXECUTE,
        METHOD_MIGRATION_OWNERSHIP_READ,
    ] {
        assert!(
            request_methods.iter().any(|value| value == method),
            "{method}"
        );
    }

    let native_notifications = fixture["native_notifications"].as_array().unwrap();
    for method in [
        NOTIFICATION_THREAD_CREATED,
        NOTIFICATION_THREAD_UPDATED,
        NOTIFICATION_THREAD_ARCHIVED,
        NOTIFICATION_TURN_STARTED,
        NOTIFICATION_ITEM_UPDATED,
        NOTIFICATION_APPROVAL_CREATED,
        NOTIFICATION_APPROVAL_RESOLVED,
        NOTIFICATION_TURN_COMPLETED,
        NOTIFICATION_TURN_FAILED,
        NOTIFICATION_TURN_CANCELLED,
    ] {
        assert!(
            native_notifications.iter().any(|value| value == method),
            "{method}"
        );
    }

    let legacy_manifest: Value = serde_json::from_str(K001_MANIFEST).unwrap();
    assert_eq!(
        legacy_manifest["state_owner"],
        "python_compatibility_adapter"
    );
    assert_eq!(
        legacy_manifest["persistent_state_writers"][0],
        "python_fastapi"
    );

    let legacy_turn: Value = serde_json::from_str(K001_TURN_FIXTURE).unwrap();
    let golden_hash = "2981eddeaf38faafc9147211181c44e353666ef790210a12be184530174ebb3e";
    assert_eq!(
        legacy_turn["canonical_golden"]["turn_items_sha256"],
        golden_hash
    );
    assert_eq!(
        fixture["k001_compatibility"]["canonical_turn_items_sha256"],
        golden_hash
    );
}
