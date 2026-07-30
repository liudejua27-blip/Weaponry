//! Transport-neutral remote neural visual provider port and coordinator.
//!
//! N002 intentionally stops at a provider-produced artifact handle. Rust GLB
//! download/readback, eight-view acceptance, version promotion and UI are
//! later tasks. The coordinator is in-memory but exposes a bounded recovery
//! snapshot so the owning desktop runtime can persist it without exposing its
//! database to a provider implementation.

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use forgecad_core::{
    semantic_sha256, Neural3DBackend, Neural3DGenerationRequest, NeuralVisualGenerationJob,
    NeuralVisualStage,
};
use serde::{Deserialize, Serialize};

pub const NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION: &str = "NeuralVisualProviderReceipt@1";
pub const NEURAL_VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION: &str = "NeuralVisualRemoteJobRecord@1";
pub type NeuralVisualProviderFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, NeuralVisualProviderError>> + Send + 'static>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeuralVisualProviderReceipt {
    pub schema_version: String,
    pub backend: Neural3DBackend,
    pub provider_job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeuralVisualProviderArtifactHandle {
    pub artifact_handle_id: String,
    pub glb_sha256: String,
    pub glb_byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum NeuralVisualProviderStatus {
    Queued,
    Running,
    Ready {
        artifact: NeuralVisualProviderArtifactHandle,
    },
    Failed {
        code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeuralVisualProviderError {
    pub code: &'static str,
    pub message: String,
}

impl NeuralVisualProviderError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub trait NeuralVisualProviderPort: Send + Sync + 'static {
    fn submit(
        &self,
        request: Neural3DGenerationRequest,
        backend: Neural3DBackend,
    ) -> NeuralVisualProviderFuture<NeuralVisualProviderReceipt>;

    fn poll(
        &self,
        receipt: NeuralVisualProviderReceipt,
    ) -> NeuralVisualProviderFuture<NeuralVisualProviderStatus>;

    fn cancel(&self, receipt: NeuralVisualProviderReceipt) -> NeuralVisualProviderFuture<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeuralVisualCoordinatorConfig {
    pub max_remote_duration_ms: u64,
    pub max_active_jobs: usize,
}

impl Default for NeuralVisualCoordinatorConfig {
    fn default() -> Self {
        Self {
            max_remote_duration_ms: 15 * 60 * 1_000,
            max_active_jobs: 8,
        }
    }
}

impl NeuralVisualCoordinatorConfig {
    fn validate(self) -> Result<Self, NeuralVisualProviderError> {
        if !(1_000..=60 * 60 * 1_000).contains(&self.max_remote_duration_ms)
            || !(1..=32).contains(&self.max_active_jobs)
        {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_COORDINATOR_CONFIG_INVALID",
                "Remote duration and active-job bounds are outside the reviewed limits.",
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum NeuralVisualRemoteOutcome {
    Waiting,
    AwaitingRustAcceptance {
        artifact: NeuralVisualProviderArtifactHandle,
    },
    Failed {
        code: String,
    },
    Cancelled {
        code: String,
    },
}

impl NeuralVisualRemoteOutcome {
    fn is_terminal(&self) -> bool {
        !matches!(self, Self::Waiting)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NeuralVisualRemoteJobRecord {
    pub schema_version: String,
    pub request_sha256: String,
    pub idempotency_key: String,
    pub job: NeuralVisualGenerationJob,
    pub receipt: NeuralVisualProviderReceipt,
    pub outcome: NeuralVisualRemoteOutcome,
}

impl NeuralVisualRemoteJobRecord {
    fn validate(&self) -> Result<(), NeuralVisualProviderError> {
        if self.schema_version != NEURAL_VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION
            || self.receipt.schema_version != NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION
        {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_RECORD_SCHEMA_INVALID",
                "Remote job record or receipt does not use its exact v1 schema.",
            ));
        }
        require_sha256("request_sha256", &self.request_sha256)?;
        if self.idempotency_key.is_empty() || self.idempotency_key.len() > 128 {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_IDEMPOTENCY_INVALID",
                "Remote job idempotency key must be bounded and non-empty.",
            ));
        }
        self.job.validate().map_err(|error| {
            NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_JOB_INVALID",
                format!("Rust-owned remote job is invalid: {}", error.code()),
            )
        })?;
        if self.job.selected_backend != Some(self.receipt.backend)
            || self.job.provider_job_id.as_deref() != Some(self.receipt.provider_job_id.as_str())
        {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_RECEIPT_MISMATCH",
                "Remote receipt must exactly match the Rust-owned backend binding.",
            ));
        }
        match (&self.outcome, self.job.stage) {
            (
                NeuralVisualRemoteOutcome::AwaitingRustAcceptance { artifact },
                NeuralVisualStage::PbrRefining,
            ) => artifact.validate(),
            (NeuralVisualRemoteOutcome::Failed { code }, NeuralVisualStage::Failed)
            | (NeuralVisualRemoteOutcome::Cancelled { code }, NeuralVisualStage::Cancelled) => {
                require_code(code)
            }
            (NeuralVisualRemoteOutcome::Waiting, NeuralVisualStage::GeometryGenerating) => Ok(()),
            _ => Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_OUTCOME_MISMATCH",
                "Remote outcome does not match the Rust-owned generation stage.",
            )),
        }
    }
}

impl NeuralVisualProviderArtifactHandle {
    fn validate(&self) -> Result<(), NeuralVisualProviderError> {
        require_id("artifact_handle_id", &self.artifact_handle_id)?;
        require_sha256("glb_sha256", &self.glb_sha256)?;
        if self.glb_byte_size == 0 {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_ARTIFACT_EMPTY",
                "Provider artifact handle cannot describe empty bytes.",
            ));
        }
        Ok(())
    }
}

struct CoordinatorState {
    jobs: HashMap<String, NeuralVisualRemoteJobRecord>,
    idempotency_to_job: HashMap<String, String>,
}

pub struct NeuralVisualCoordinator {
    provider: Arc<dyn NeuralVisualProviderPort>,
    config: NeuralVisualCoordinatorConfig,
    state: Mutex<CoordinatorState>,
}

impl NeuralVisualCoordinator {
    pub fn new(
        provider: Arc<dyn NeuralVisualProviderPort>,
        config: NeuralVisualCoordinatorConfig,
    ) -> Result<Self, NeuralVisualProviderError> {
        Ok(Self {
            provider,
            config: config.validate()?,
            state: Mutex::new(CoordinatorState {
                jobs: HashMap::new(),
                idempotency_to_job: HashMap::new(),
            }),
        })
    }

    pub async fn submit(
        &self,
        request: &Neural3DGenerationRequest,
        backend: Neural3DBackend,
        job_id: String,
    ) -> Result<NeuralVisualRemoteJobRecord, NeuralVisualProviderError> {
        request.validate().map_err(|error| {
            NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_REQUEST_INVALID",
                format!("Neural request failed Rust validation: {}", error.code()),
            )
        })?;
        require_id("job_id", &job_id)?;
        if !request.backend_preferences.contains(&backend) {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_BACKEND_NOT_REQUESTED",
                "Selected backend is not present in the ordered Rust request preferences.",
            ));
        }
        let request_sha256 = semantic_sha256(request).map_err(|_| {
            NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_REQUEST_HASH_FAILED",
                "Rust could not hash the normalized neural request.",
            )
        })?;

        {
            let state = self.lock_state()?;
            if let Some(previous_job_id) = state.idempotency_to_job.get(&request.idempotency_key) {
                let previous = state.jobs.get(previous_job_id).ok_or_else(|| {
                    NeuralVisualProviderError::new(
                        "NEURAL_VISUAL_REMOTE_STATE_CORRUPT",
                        "Idempotency index points to a missing remote job.",
                    )
                })?;
                if previous.request_sha256 != request_sha256 {
                    return Err(NeuralVisualProviderError::new(
                        "NEURAL_VISUAL_REMOTE_IDEMPOTENCY_CONFLICT",
                        "One idempotency key cannot identify two neural requests.",
                    ));
                }
                return Ok(previous.clone());
            }
            let active = state
                .jobs
                .values()
                .filter(|record| !record.outcome.is_terminal())
                .count();
            if active >= self.config.max_active_jobs {
                return Err(NeuralVisualProviderError::new(
                    "NEURAL_VISUAL_REMOTE_CAPACITY_EXCEEDED",
                    "The bounded number of active remote neural jobs is exhausted.",
                ));
            }
        }

        let mut job =
            NeuralVisualGenerationJob::queued(job_id.clone(), request).map_err(|error| {
                NeuralVisualProviderError::new(
                    "NEURAL_VISUAL_REMOTE_JOB_INVALID",
                    format!("Rust could not create the queued job: {}", error.code()),
                )
            })?;
        job.advance(NeuralVisualStage::ConceptReady)
            .map_err(core_transition_error)?;
        let receipt = self.provider.submit(request.clone(), backend).await?;
        validate_receipt(&receipt, backend)?;
        job.bind_backend(backend, receipt.provider_job_id.clone())
            .map_err(core_transition_error)?;
        job.advance(NeuralVisualStage::GeometryGenerating)
            .map_err(core_transition_error)?;
        let record = NeuralVisualRemoteJobRecord {
            schema_version: NEURAL_VISUAL_REMOTE_JOB_RECORD_SCHEMA_VERSION.into(),
            request_sha256,
            idempotency_key: request.idempotency_key.clone(),
            job,
            receipt,
            outcome: NeuralVisualRemoteOutcome::Waiting,
        };
        record.validate()?;

        let mut state = self.lock_state()?;
        if state.jobs.contains_key(&job_id)
            || state
                .idempotency_to_job
                .contains_key(&request.idempotency_key)
        {
            let _ = self.provider.cancel(record.receipt.clone()).await;
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_SUBMIT_RACE",
                "Remote job identity was claimed while the provider submit was in flight.",
            ));
        }
        state
            .idempotency_to_job
            .insert(request.idempotency_key.clone(), job_id.clone());
        state.jobs.insert(job_id, record.clone());
        Ok(record)
    }

    pub async fn poll(
        &self,
        job_id: &str,
        project_id: &str,
        turn_id: &str,
        elapsed_ms: u64,
    ) -> Result<NeuralVisualRemoteJobRecord, NeuralVisualProviderError> {
        let current = self.scoped_record(job_id, project_id, turn_id)?;
        if current.outcome.is_terminal() {
            return Ok(current);
        }
        if elapsed_ms > self.config.max_remote_duration_ms {
            let _ = self.provider.cancel(current.receipt.clone()).await;
            return self.finish(
                current,
                NeuralVisualRemoteOutcome::Failed {
                    code: "REMOTE_TIMEOUT".into(),
                },
            );
        }
        let provider_status = self.provider.poll(current.receipt.clone()).await?;
        match provider_status {
            NeuralVisualProviderStatus::Queued | NeuralVisualProviderStatus::Running => Ok(current),
            NeuralVisualProviderStatus::Ready { artifact } => {
                artifact.validate()?;
                let mut next = current;
                next.job
                    .advance(NeuralVisualStage::PbrRefining)
                    .map_err(core_transition_error)?;
                next.outcome = NeuralVisualRemoteOutcome::AwaitingRustAcceptance { artifact };
                self.replace(next)
            }
            NeuralVisualProviderStatus::Failed { code } => {
                require_code(&code)?;
                self.finish(current, NeuralVisualRemoteOutcome::Failed { code })
            }
        }
    }

    pub async fn cancel(
        &self,
        job_id: &str,
        project_id: &str,
        turn_id: &str,
    ) -> Result<NeuralVisualRemoteJobRecord, NeuralVisualProviderError> {
        let current = self.scoped_record(job_id, project_id, turn_id)?;
        if current.outcome.is_terminal() {
            return Ok(current);
        }
        self.provider.cancel(current.receipt.clone()).await?;
        self.finish(
            current,
            NeuralVisualRemoteOutcome::Cancelled {
                code: "USER_CANCELLED".into(),
            },
        )
    }

    pub fn recovery_snapshot(
        &self,
    ) -> Result<Vec<NeuralVisualRemoteJobRecord>, NeuralVisualProviderError> {
        let state = self.lock_state()?;
        let mut records = state.jobs.values().cloned().collect::<Vec<_>>();
        records.sort_by(|left, right| left.job.job_id.cmp(&right.job.job_id));
        Ok(records)
    }

    pub fn restore(
        provider: Arc<dyn NeuralVisualProviderPort>,
        config: NeuralVisualCoordinatorConfig,
        records: Vec<NeuralVisualRemoteJobRecord>,
    ) -> Result<Self, NeuralVisualProviderError> {
        let coordinator = Self::new(provider, config)?;
        let mut state = coordinator.lock_state()?;
        for record in records {
            record.validate()?;
            if state.jobs.contains_key(&record.job.job_id)
                || state
                    .idempotency_to_job
                    .contains_key(&record.idempotency_key)
            {
                return Err(NeuralVisualProviderError::new(
                    "NEURAL_VISUAL_REMOTE_RECOVERY_DUPLICATE",
                    "Recovery snapshot contains duplicate job or idempotency identity.",
                ));
            }
            state
                .idempotency_to_job
                .insert(record.idempotency_key.clone(), record.job.job_id.clone());
            state.jobs.insert(record.job.job_id.clone(), record);
        }
        drop(state);
        Ok(coordinator)
    }

    fn finish(
        &self,
        mut record: NeuralVisualRemoteJobRecord,
        outcome: NeuralVisualRemoteOutcome,
    ) -> Result<NeuralVisualRemoteJobRecord, NeuralVisualProviderError> {
        match &outcome {
            NeuralVisualRemoteOutcome::Failed { code } => record
                .job
                .fail(code.clone())
                .map_err(core_transition_error)?,
            NeuralVisualRemoteOutcome::Cancelled { code } => record
                .job
                .cancel(code.clone())
                .map_err(core_transition_error)?,
            _ => {
                return Err(NeuralVisualProviderError::new(
                    "NEURAL_VISUAL_REMOTE_FINISH_INVALID",
                    "Only failed or cancelled outcomes may use the terminal helper.",
                ))
            }
        }
        record.outcome = outcome;
        self.replace(record)
    }

    fn replace(
        &self,
        record: NeuralVisualRemoteJobRecord,
    ) -> Result<NeuralVisualRemoteJobRecord, NeuralVisualProviderError> {
        record.validate()?;
        let mut state = self.lock_state()?;
        let stored = state.jobs.get(&record.job.job_id).ok_or_else(|| {
            NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_JOB_NOT_FOUND",
                "Remote neural job is not present in the coordinator.",
            )
        })?;
        if stored.request_sha256 != record.request_sha256
            || stored.job.project_id != record.job.project_id
            || stored.job.turn_id != record.job.turn_id
        {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_LATE_RESULT_REJECTED",
                "Late remote result no longer matches the active Rust job identity.",
            ));
        }
        state.jobs.insert(record.job.job_id.clone(), record.clone());
        Ok(record)
    }

    fn scoped_record(
        &self,
        job_id: &str,
        project_id: &str,
        turn_id: &str,
    ) -> Result<NeuralVisualRemoteJobRecord, NeuralVisualProviderError> {
        let state = self.lock_state()?;
        let record = state.jobs.get(job_id).ok_or_else(|| {
            NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_JOB_NOT_FOUND",
                "Remote neural job is not present in the coordinator.",
            )
        })?;
        if record.job.project_id != project_id || record.job.turn_id != turn_id {
            return Err(NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_SCOPE_MISMATCH",
                "Remote neural job does not belong to the requested Project and Turn.",
            ));
        }
        Ok(record.clone())
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, CoordinatorState>, NeuralVisualProviderError> {
        self.state.lock().map_err(|_| {
            NeuralVisualProviderError::new(
                "NEURAL_VISUAL_REMOTE_STATE_POISONED",
                "Remote neural coordinator state is unavailable.",
            )
        })
    }
}

fn validate_receipt(
    receipt: &NeuralVisualProviderReceipt,
    expected_backend: Neural3DBackend,
) -> Result<(), NeuralVisualProviderError> {
    if receipt.schema_version != NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION
        || receipt.backend != expected_backend
    {
        return Err(NeuralVisualProviderError::new(
            "NEURAL_VISUAL_REMOTE_RECEIPT_INVALID",
            "Provider receipt schema or backend does not match the Rust request.",
        ));
    }
    require_id("provider_job_id", &receipt.provider_job_id)
}

pub fn validate_neural_visual_receipt(
    receipt: &NeuralVisualProviderReceipt,
    expected_backend: Neural3DBackend,
) -> Result<(), NeuralVisualProviderError> {
    validate_receipt(receipt, expected_backend)
}

fn core_transition_error(error: forgecad_core::CoreError) -> NeuralVisualProviderError {
    NeuralVisualProviderError::new(
        "NEURAL_VISUAL_REMOTE_STAGE_INVALID",
        format!(
            "Rust-owned neural stage transition failed: {}",
            error.code()
        ),
    )
}

fn require_id(field: &str, value: &str) -> Result<(), NeuralVisualProviderError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'@'))
    {
        return Err(NeuralVisualProviderError::new(
            "NEURAL_VISUAL_REMOTE_ID_INVALID",
            format!("{field} must be a bounded stable identifier."),
        ));
    }
    Ok(())
}

fn require_sha256(field: &str, value: &str) -> Result<(), NeuralVisualProviderError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(NeuralVisualProviderError::new(
            "NEURAL_VISUAL_REMOTE_SHA256_INVALID",
            format!("{field} must be a lowercase SHA-256 digest."),
        ));
    }
    Ok(())
}

fn require_code(value: &str) -> Result<(), NeuralVisualProviderError> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(NeuralVisualProviderError::new(
            "NEURAL_VISUAL_REMOTE_CODE_INVALID",
            "Provider failure codes must use bounded uppercase snake case.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_core::{VisualQualityTier, NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION};
    use std::collections::VecDeque;

    struct FakeProvider {
        statuses: Arc<Mutex<VecDeque<NeuralVisualProviderStatus>>>,
        submit_count: Arc<Mutex<usize>>,
        poll_count: Arc<Mutex<usize>>,
        cancel_count: Arc<Mutex<usize>>,
    }

    impl FakeProvider {
        fn new(statuses: impl IntoIterator<Item = NeuralVisualProviderStatus>) -> Self {
            Self {
                statuses: Arc::new(Mutex::new(statuses.into_iter().collect())),
                submit_count: Arc::new(Mutex::new(0)),
                poll_count: Arc::new(Mutex::new(0)),
                cancel_count: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl NeuralVisualProviderPort for FakeProvider {
        fn submit(
            &self,
            _request: Neural3DGenerationRequest,
            backend: Neural3DBackend,
        ) -> NeuralVisualProviderFuture<NeuralVisualProviderReceipt> {
            let submit_count = self.submit_count.clone();
            Box::pin(async move {
                *submit_count.lock().unwrap() += 1;
                Ok(NeuralVisualProviderReceipt {
                    schema_version: NEURAL_VISUAL_PROVIDER_RECEIPT_SCHEMA_VERSION.into(),
                    backend,
                    provider_job_id: "provider_job_1".into(),
                })
            })
        }

        fn poll(
            &self,
            _receipt: NeuralVisualProviderReceipt,
        ) -> NeuralVisualProviderFuture<NeuralVisualProviderStatus> {
            let poll_count = self.poll_count.clone();
            let statuses = self.statuses.clone();
            Box::pin(async move {
                *poll_count.lock().unwrap() += 1;
                statuses.lock().unwrap().pop_front().ok_or_else(|| {
                    NeuralVisualProviderError::new(
                        "FAKE_PROVIDER_STATUS_EXHAUSTED",
                        "Fake provider has no next status.",
                    )
                })
            })
        }

        fn cancel(&self, _receipt: NeuralVisualProviderReceipt) -> NeuralVisualProviderFuture<()> {
            let cancel_count = self.cancel_count.clone();
            Box::pin(async move {
                *cancel_count.lock().unwrap() += 1;
                Ok(())
            })
        }
    }

    fn sha(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn request(idempotency_key: &str) -> Neural3DGenerationRequest {
        Neural3DGenerationRequest {
            schema_version: NEURAL_3D_GENERATION_REQUEST_SCHEMA_VERSION.into(),
            request_id: "request_1".into(),
            project_id: "project_1".into(),
            turn_id: "turn_1".into(),
            brief_id: "brief_1".into(),
            concept_reference_id: "reference_1".into(),
            concept_reference_sha256: sha('a'),
            additional_views: vec![],
            quality_tier: VisualQualityTier::StandardAsset,
            backend_preferences: vec![Neural3DBackend::Pixal3d, Neural3DBackend::Trellis2],
            idempotency_key: idempotency_key.into(),
        }
    }

    fn run_async(future: impl Future<Output = ()>) {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(future);
    }

    #[test]
    fn submit_is_idempotent_and_conflicting_reuse_is_rejected() {
        run_async(async {
            let provider = Arc::new(FakeProvider::new([]));
            let coordinator =
                NeuralVisualCoordinator::new(provider.clone(), Default::default()).unwrap();
            let first = coordinator
                .submit(
                    &request("same_key"),
                    Neural3DBackend::Pixal3d,
                    "job_1".into(),
                )
                .await
                .unwrap();
            let replay = coordinator
                .submit(
                    &request("same_key"),
                    Neural3DBackend::Pixal3d,
                    "job_2".into(),
                )
                .await
                .unwrap();
            assert_eq!(first, replay);
            assert_eq!(*provider.submit_count.lock().unwrap(), 1);

            let mut drifted = request("same_key");
            drifted.quality_tier = VisualQualityTier::CollectibleAsset;
            assert_eq!(
                coordinator
                    .submit(&drifted, Neural3DBackend::Pixal3d, "job_3".into())
                    .await
                    .unwrap_err()
                    .code,
                "NEURAL_VISUAL_REMOTE_IDEMPOTENCY_CONFLICT"
            );
        });
    }

    #[test]
    fn provider_ready_stops_before_rust_acceptance_and_survives_restore() {
        run_async(async {
            let provider = Arc::new(FakeProvider::new([
                NeuralVisualProviderStatus::Running,
                NeuralVisualProviderStatus::Ready {
                    artifact: NeuralVisualProviderArtifactHandle {
                        artifact_handle_id: "handle_1".into(),
                        glb_sha256: sha('b'),
                        glb_byte_size: 4096,
                    },
                },
            ]));
            let coordinator =
                NeuralVisualCoordinator::new(provider.clone(), Default::default()).unwrap();
            coordinator
                .submit(&request("key_1"), Neural3DBackend::Trellis2, "job_1".into())
                .await
                .unwrap();
            let running = coordinator
                .poll("job_1", "project_1", "turn_1", 10)
                .await
                .unwrap();
            assert!(matches!(
                running.outcome,
                NeuralVisualRemoteOutcome::Waiting
            ));
            let ready = coordinator
                .poll("job_1", "project_1", "turn_1", 20)
                .await
                .unwrap();
            assert_eq!(ready.job.stage, NeuralVisualStage::PbrRefining);
            assert!(matches!(
                ready.outcome,
                NeuralVisualRemoteOutcome::AwaitingRustAcceptance { .. }
            ));

            let restored = NeuralVisualCoordinator::restore(
                provider,
                Default::default(),
                coordinator.recovery_snapshot().unwrap(),
            )
            .unwrap();
            assert_eq!(restored.recovery_snapshot().unwrap(), vec![ready]);
        });
    }

    #[test]
    fn cancellation_and_timeout_block_late_provider_results() {
        run_async(async {
            let provider = Arc::new(FakeProvider::new([NeuralVisualProviderStatus::Ready {
                artifact: NeuralVisualProviderArtifactHandle {
                    artifact_handle_id: "late_handle".into(),
                    glb_sha256: sha('c'),
                    glb_byte_size: 2048,
                },
            }]));
            let coordinator =
                NeuralVisualCoordinator::new(provider.clone(), Default::default()).unwrap();
            coordinator
                .submit(
                    &request("key_cancel"),
                    Neural3DBackend::Pixal3d,
                    "job_cancel".into(),
                )
                .await
                .unwrap();
            let cancelled = coordinator
                .cancel("job_cancel", "project_1", "turn_1")
                .await
                .unwrap();
            let replay = coordinator
                .poll("job_cancel", "project_1", "turn_1", 1)
                .await
                .unwrap();
            assert_eq!(replay, cancelled);
            assert_eq!(*provider.poll_count.lock().unwrap(), 0);

            let timeout_provider =
                Arc::new(FakeProvider::new([NeuralVisualProviderStatus::Running]));
            let timeout_coordinator =
                NeuralVisualCoordinator::new(timeout_provider.clone(), Default::default()).unwrap();
            timeout_coordinator
                .submit(
                    &request("key_timeout"),
                    Neural3DBackend::Pixal3d,
                    "job_timeout".into(),
                )
                .await
                .unwrap();
            let timed_out = timeout_coordinator
                .poll(
                    "job_timeout",
                    "project_1",
                    "turn_1",
                    NeuralVisualCoordinatorConfig::default().max_remote_duration_ms + 1,
                )
                .await
                .unwrap();
            assert!(matches!(
                timed_out.outcome,
                NeuralVisualRemoteOutcome::Failed { ref code } if code == "REMOTE_TIMEOUT"
            ));
            assert_eq!(*timeout_provider.poll_count.lock().unwrap(), 0);
            assert_eq!(*timeout_provider.cancel_count.lock().unwrap(), 1);
        });
    }

    #[test]
    fn project_or_turn_scope_mismatch_is_fail_closed() {
        run_async(async {
            let provider = Arc::new(FakeProvider::new([]));
            let coordinator = NeuralVisualCoordinator::new(provider, Default::default()).unwrap();
            coordinator
                .submit(
                    &request("key_scope"),
                    Neural3DBackend::Pixal3d,
                    "job_scope".into(),
                )
                .await
                .unwrap();
            assert_eq!(
                coordinator
                    .poll("job_scope", "other_project", "turn_1", 1)
                    .await
                    .unwrap_err()
                    .code,
                "NEURAL_VISUAL_REMOTE_SCOPE_MISMATCH"
            );
        });
    }
}
