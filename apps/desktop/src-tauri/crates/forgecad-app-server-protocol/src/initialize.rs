use serde::{Deserialize, Serialize};

use crate::{RpcError, CAPABILITY_UNSUPPORTED, PROTOCOL_VERSION_UNSUPPORTED};

pub const FORGECAD_PROTOCOL_VERSION: &str = "forgecad.app-server/1";
pub const INITIALIZE_PARAMS_SCHEMA_VERSION: &str = "ForgeCADInitializeParams@1";
pub const INITIALIZE_RESULT_SCHEMA_VERSION: &str = "ForgeCADInitializeResult@1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ClientTransport {
    Tauri,
    BrowserLoopbackCompatibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
    pub transport: ClientTransport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProtocolCapabilities {
    pub notifications: bool,
    pub cursor_replay: bool,
    pub cancellation: bool,
    pub notification_ack: bool,
    pub binary_body_base64: bool,
}

impl ProtocolCapabilities {
    pub const REQUIRED: Self = Self {
        notifications: true,
        cursor_replay: true,
        cancellation: true,
        notification_ack: true,
        binary_body_base64: true,
    };

    pub fn validate_required(&self) -> Result<(), RpcError> {
        if self != &Self::REQUIRED {
            return Err(RpcError::new(
                CAPABILITY_UNSUPPORTED,
                "CAPABILITY_UNSUPPORTED",
                "ForgeCAD protocol v1 requires notifications, replay, cancellation, acknowledgements, and base64 binary bodies.",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InitializeParams {
    pub schema_version: String,
    pub supported_protocol_versions: Vec<String>,
    pub client_info: ClientInfo,
    pub capabilities: ProtocolCapabilities,
}

impl InitializeParams {
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.schema_version != INITIALIZE_PARAMS_SCHEMA_VERSION {
            return Err(RpcError::invalid_params(format!(
                "schema_version must be {INITIALIZE_PARAMS_SCHEMA_VERSION}."
            )));
        }
        if self.client_info.name.is_empty() || self.client_info.version.is_empty() {
            return Err(RpcError::invalid_params(
                "client_info name and version must be non-empty.",
            ));
        }
        self.capabilities.validate_required()?;
        select_protocol_version(&self.supported_protocol_versions)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InitializedParams {
    pub protocol_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProtocolLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_in_flight_requests: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_event_queue: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_frame_bytes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStateOwner {
    PythonCompatibilityAdapter,
    RustAppServer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MigrationState {
    pub state_owner: MigrationStateOwner,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InitializeResult {
    pub schema_version: String,
    pub protocol_version: String,
    pub connection_id: String,
    pub server_info: ServerInfo,
    pub capabilities: ProtocolCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<ProtocolLimits>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_state: Option<MigrationState>,
}

pub fn select_protocol_version(supported: &[String]) -> Result<&'static str, RpcError> {
    if supported
        .iter()
        .any(|value| value == FORGECAD_PROTOCOL_VERSION)
    {
        return Ok(FORGECAD_PROTOCOL_VERSION);
    }
    Err(RpcError::new(
        PROTOCOL_VERSION_UNSUPPORTED,
        "PROTOCOL_VERSION_UNSUPPORTED",
        "The client and server do not share a ForgeCAD app-server protocol version.",
        false,
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn initialize_fixture_matches_typescript_contract() {
        let fixture = json!({
            "schema_version": "ForgeCADInitializeParams@1",
            "supported_protocol_versions": ["forgecad.app-server/1"],
            "client_info": {
                "name": "forgecad-desktop",
                "version": "0.1.0",
                "transport": "tauri"
            },
            "capabilities": {
                "notifications": true,
                "cursor_replay": true,
                "cancellation": true,
                "notification_ack": true,
                "binary_body_base64": true
            }
        });
        let params: InitializeParams = serde_json::from_value(fixture.clone()).unwrap();
        params.validate().unwrap();
        assert_eq!(serde_json::to_value(params).unwrap(), fixture);
    }

    #[test]
    fn missing_protocol_or_required_capability_is_stable_error() {
        assert_eq!(
            select_protocol_version(&["future/9".into()])
                .unwrap_err()
                .code,
            PROTOCOL_VERSION_UNSUPPORTED
        );
        let mut capabilities = ProtocolCapabilities::REQUIRED;
        capabilities.notification_ack = false;
        assert_eq!(
            capabilities.validate_required().unwrap_err().code,
            CAPABILITY_UNSUPPORTED
        );
    }
}
