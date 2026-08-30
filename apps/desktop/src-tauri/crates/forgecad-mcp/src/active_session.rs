//! Default Knife MCP session state and initialize validation.
//!
//! This module owns only the in-memory state of one default MCP stdio
//! session.  It deliberately does not know about a Runtime, Store, CAS,
//! backend/cohort checks, manifest composition, or JSON-RPC response
//! envelopes.  The MCP adapter can therefore keep transport and backend
//! policy at its boundary while delegating the state transition here.
//!
//! A session is single-use: an invalid initialize attempt moves it to
//! `Failed`, and a successful attempt moves it to `Ready`.  There is no reset
//! transition because a new MCP connection must create a new session.

use serde_json::Value;

/// Lifecycle of one default Knife MCP session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum SessionState {
    /// No valid initialize request has been accepted yet.
    #[default]
    New,
    /// The protocol and required client metadata were negotiated.
    Ready,
    /// Initialization failed and the connection must be restarted.
    Failed,
}

impl SessionState {
    pub(crate) fn is_new(self) -> bool {
        self == Self::New
    }

    pub(crate) fn is_ready(self) -> bool {
        self == Self::Ready
    }

    pub(crate) fn is_failed(self) -> bool {
        self == Self::Failed
    }
}

/// Validation and transition failures for the MCP initialize exchange.
///
/// The variants intentionally retain the same distinctions used by the
/// adapter's existing JSON-RPC error mapping.  In particular, an invalid
/// initialize request fails the session, while `AlreadyInitialized` leaves
/// the existing state unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InitializeError {
    /// The session has already reached `Ready` or `Failed`.
    AlreadyInitialized,
    /// `params` was absent or was not a JSON object.
    InvalidParams,
    /// `protocolVersion` was absent, not a string, or unsupported.
    UnsupportedProtocol {
        requested: String,
        supported: Vec<String>,
    },
    /// Both required initialize metadata objects must be present.
    MissingCapabilitiesOrClientInfo,
}

/// The typed portion of a validated initialize request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitializeRequest {
    protocol_version: String,
}

impl InitializeRequest {
    pub(crate) fn protocol_version(&self) -> &str {
        &self.protocol_version
    }
}

/// Validate initialize parameters without changing session state.
///
/// `supported_protocols` is supplied by the caller so the MCP module does
/// not duplicate the Contracts/Runtime protocol list.  The function is pure:
/// callers may use it to inspect or test a request before deciding whether to
/// mutate a session.
pub(crate) fn validate_initialize_params(
    params: Option<&Value>,
    supported_protocols: &[&str],
) -> Result<InitializeRequest, InitializeError> {
    let Some(params) = params.and_then(Value::as_object) else {
        return Err(InitializeError::InvalidParams);
    };

    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !supported_protocols.contains(&requested) {
        return Err(InitializeError::UnsupportedProtocol {
            requested: requested.to_owned(),
            supported: supported_protocols
                .iter()
                .map(|version| (*version).to_owned())
                .collect(),
        });
    }

    if params
        .get("capabilities")
        .and_then(Value::as_object)
        .is_none()
        || params
            .get("clientInfo")
            .and_then(Value::as_object)
            .is_none()
    {
        return Err(InitializeError::MissingCapabilitiesOrClientInfo);
    }

    Ok(InitializeRequest {
        protocol_version: requested.to_owned(),
    })
}

/// In-memory state for one default Knife MCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Session {
    state: SessionState,
    negotiated_protocol_version: Option<String>,
    write_tools_enabled: bool,
    preflight_read: bool,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            state: SessionState::New,
            negotiated_protocol_version: None,
            write_tools_enabled: false,
            preflight_read: false,
        }
    }
}

impl Session {
    /// Construct a fresh session.  The default profile is Knife; no
    /// compatibility-profile state is represented here.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn state(&self) -> SessionState {
        self.state
    }

    pub(crate) fn negotiated_protocol_version(&self) -> Option<&str> {
        self.negotiated_protocol_version.as_deref()
    }

    pub(crate) fn write_tools_enabled(&self) -> bool {
        self.write_tools_enabled
    }

    pub(crate) fn preflight_read(&self) -> bool {
        self.preflight_read
    }

    /// Apply the initialize validation and, when valid, commit the one-way
    /// `New → Ready` transition.
    ///
    /// Backend eligibility (authenticated IPC, build cohort, environment
    /// opt-in) is computed by the caller and passed as a boolean.  This keeps
    /// the state module free of backend and Runtime dependencies.
    pub(crate) fn try_initialize(
        &mut self,
        params: Option<&Value>,
        supported_protocols: &[&str],
        write_tools_enabled: bool,
    ) -> Result<String, InitializeError> {
        if !self.state.is_new() {
            return Err(InitializeError::AlreadyInitialized);
        }

        let request = match validate_initialize_params(params, supported_protocols) {
            Ok(request) => request,
            Err(error) => {
                self.mark_failed();
                return Err(error);
            }
        };

        let protocol_version = request.protocol_version().to_owned();
        self.negotiated_protocol_version = Some(protocol_version.clone());
        self.write_tools_enabled = write_tools_enabled;
        self.state = SessionState::Ready;
        Ok(protocol_version)
    }

    /// Mark initialization as failed, including a request without an id.
    pub(crate) fn mark_failed(&mut self) {
        self.state = SessionState::Failed;
    }

    /// Record that the first-party preflight skill was successfully read.
    ///
    /// The adapter only calls this after a successful tool dispatch.  The
    /// ready-state guard keeps an accidentally invoked helper fail-closed.
    pub(crate) fn mark_preflight_read(&mut self) {
        if self.state.is_ready() {
            self.preflight_read = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const SUPPORTED: &[&str] = &["2025-11-25", "2024-11-05"];

    fn valid_params() -> Value {
        json!({
            "protocolVersion": SUPPORTED[0],
            "capabilities": {},
            "clientInfo": {"name": "codex-test", "version": "1"}
        })
    }

    #[test]
    fn new_session_is_fail_closed_and_has_no_negotiation_or_preflight() {
        let session = Session::new();

        assert_eq!(session.state(), SessionState::New);
        assert!(session.state().is_new());
        assert!(!session.state().is_ready());
        assert!(!session.state().is_failed());
        assert_eq!(session.negotiated_protocol_version(), None);
        assert!(!session.write_tools_enabled());
        assert!(!session.preflight_read());
    }

    #[test]
    fn validator_is_pure_and_returns_the_negotiated_protocol() {
        let params = valid_params();
        let request = validate_initialize_params(Some(&params), SUPPORTED)
            .expect("valid initialize params pass validation");

        assert_eq!(request.protocol_version(), SUPPORTED[0]);
    }

    #[test]
    fn validator_preserves_invalid_params_protocol_and_metadata_distinctions() {
        assert_eq!(
            validate_initialize_params(None, SUPPORTED),
            Err(InitializeError::InvalidParams)
        );

        assert_eq!(
            validate_initialize_params(
                Some(&json!({
                    "protocolVersion": "unsupported",
                    "capabilities": {},
                    "clientInfo": {}
                })),
                SUPPORTED,
            ),
            Err(InitializeError::UnsupportedProtocol {
                requested: "unsupported".to_owned(),
                supported: SUPPORTED
                    .iter()
                    .map(|version| (*version).to_owned())
                    .collect(),
            })
        );

        assert_eq!(
            validate_initialize_params(Some(&json!({"protocolVersion": SUPPORTED[0]})), SUPPORTED,),
            Err(InitializeError::MissingCapabilitiesOrClientInfo)
        );
    }

    #[test]
    fn successful_initialize_commits_ready_state_and_backend_write_bit() {
        let mut session = Session::new();

        let protocol = session
            .try_initialize(Some(&valid_params()), SUPPORTED, true)
            .expect("initialize succeeds");

        assert_eq!(protocol, SUPPORTED[0]);
        assert_eq!(session.state(), SessionState::Ready);
        assert_eq!(session.negotiated_protocol_version(), Some(SUPPORTED[0]));
        assert!(session.write_tools_enabled());
        assert!(!session.preflight_read());
    }

    #[test]
    fn invalid_initialize_fails_session_and_cannot_be_retried() {
        let mut session = Session::new();

        assert_eq!(
            session.try_initialize(None, SUPPORTED, true),
            Err(InitializeError::InvalidParams)
        );
        assert_eq!(session.state(), SessionState::Failed);
        assert!(session.state().is_failed());
        assert_eq!(session.negotiated_protocol_version(), None);
        assert!(!session.write_tools_enabled());

        assert_eq!(
            session.try_initialize(Some(&valid_params()), SUPPORTED, true),
            Err(InitializeError::AlreadyInitialized)
        );
        assert_eq!(session.state(), SessionState::Failed);
    }

    #[test]
    fn repeated_initialize_leaves_ready_state_unchanged() {
        let mut session = Session::new();
        session
            .try_initialize(Some(&valid_params()), SUPPORTED, false)
            .expect("first initialize succeeds");

        assert_eq!(
            session.try_initialize(Some(&valid_params()), SUPPORTED, true),
            Err(InitializeError::AlreadyInitialized)
        );
        assert_eq!(session.state(), SessionState::Ready);
        assert!(!session.write_tools_enabled());
    }

    #[test]
    fn preflight_can_only_be_recorded_after_ready() {
        let mut failed = Session::new();
        failed.mark_preflight_read();
        assert!(!failed.preflight_read());

        let mut ready = Session::new();
        ready
            .try_initialize(Some(&valid_params()), SUPPORTED, false)
            .expect("initialize succeeds");
        ready.mark_preflight_read();
        assert!(ready.preflight_read());
    }
}
