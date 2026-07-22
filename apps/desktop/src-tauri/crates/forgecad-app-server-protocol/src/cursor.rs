use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::RpcError;

pub const CURSOR_SCHEMA_VERSION: &str = "ForgeCADAppServerCursor@1";
const CURSOR_PREFIX: &str = "fc1_";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum CursorPhase {
    TurnStarted,
    Item,
    Approval,
    TurnTerminal,
}

/// Structured server-side cursor position.  On the wire this is encoded as an
/// opaque string so clients never infer ordering from its representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppServerCursor {
    pub schema_version: String,
    pub thread_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    pub source_sequence: u64,
    pub phase: CursorPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
}

impl AppServerCursor {
    pub fn new(
        thread_id: impl Into<String>,
        turn_id: Option<String>,
        source_sequence: u64,
        phase: CursorPhase,
        item_id: Option<String>,
    ) -> Self {
        Self {
            schema_version: CURSOR_SCHEMA_VERSION.to_string(),
            thread_id: thread_id.into(),
            turn_id,
            source_sequence,
            phase,
            item_id,
        }
    }

    pub fn validate(&self) -> Result<(), RpcError> {
        if self.schema_version != CURSOR_SCHEMA_VERSION {
            return Err(RpcError::invalid_params(format!(
                "cursor schema_version must be {CURSOR_SCHEMA_VERSION}."
            )));
        }
        if !valid_stable_id(&self.thread_id) {
            return Err(RpcError::invalid_params("cursor thread_id is invalid."));
        }
        if self
            .turn_id
            .as_deref()
            .is_some_and(|value| !valid_stable_id(value))
        {
            return Err(RpcError::invalid_params("cursor turn_id is invalid."));
        }
        if self
            .item_id
            .as_deref()
            .is_some_and(|value| !valid_stable_id(value))
        {
            return Err(RpcError::invalid_params("cursor item_id is invalid."));
        }
        Ok(())
    }

    pub fn compare_position(&self, other: &Self) -> Option<Ordering> {
        if self.thread_id != other.thread_id {
            return None;
        }
        Some((self.source_sequence, self.phase).cmp(&(other.source_sequence, other.phase)))
    }

    pub fn encode(&self) -> Result<String, RpcError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| RpcError::internal(format!("Cursor serialization failed: {error}")))?;
        let mut token = String::with_capacity(CURSOR_PREFIX.len() + bytes.len() * 2);
        token.push_str(CURSOR_PREFIX);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut token, "{byte:02x}").expect("writing to String cannot fail");
        }
        Ok(token)
    }

    pub fn decode(token: &str) -> Result<Self, RpcError> {
        let hex = token.strip_prefix(CURSOR_PREFIX).ok_or_else(|| {
            RpcError::invalid_params("cursor is not a ForgeCAD v1 opaque cursor.")
        })?;
        if hex.is_empty() || hex.len() % 2 != 0 || hex.len() > 4096 {
            return Err(RpcError::invalid_params("cursor encoding is invalid."));
        }
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        for pair in hex.as_bytes().chunks_exact(2) {
            let pair = std::str::from_utf8(pair)
                .map_err(|_| RpcError::invalid_params("cursor encoding is invalid."))?;
            bytes.push(
                u8::from_str_radix(pair, 16)
                    .map_err(|_| RpcError::invalid_params("cursor encoding is invalid."))?,
            );
        }
        let cursor: Self = serde_json::from_slice(&bytes)
            .map_err(|_| RpcError::invalid_params("cursor payload is invalid."))?;
        cursor.validate()?;
        Ok(cursor)
    }
}

pub fn valid_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplayParams {
    pub cursor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AckParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CancelParams {
    pub request_id: String,
    pub cancel_token: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_cursor_round_trips_and_rejects_tampering() {
        let cursor = AppServerCursor::new(
            "thread_1",
            Some("turn_1".into()),
            4,
            CursorPhase::Item,
            Some("item_4".into()),
        );
        let token = cursor.encode().unwrap();
        assert_eq!(AppServerCursor::decode(&token).unwrap(), cursor);
        assert!(AppServerCursor::decode(&(token + "z")).is_err());
    }
}
