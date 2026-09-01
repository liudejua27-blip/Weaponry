//! Rust launcher for the checked-in, one-shot Blender knife prototype.
//!
//! The Python entrypoint is intentionally treated as an opaque fixed provider
//! here.  This adapter owns scratch staging, process arguments, timeout and
//! byte limits, and validates the result before handing temporary bytes to a
//! Runtime caller.  It never writes SQLite/CAS and it has no public method for
//! a caller-selected command, script, path, URL, add-on, or environment.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const KNIFE_WORKER_PROTOCOL: &str = "weaponry-fixed-worker-stdio-json@1";
pub const KNIFE_REQUEST_SCHEMA: &str = "WeaponryBlenderKnifeWorkerRequest@1";
pub const KNIFE_RESPONSE_SCHEMA: &str = "WeaponryBlenderKnifeWorkerResponse@1";
pub const KNIFE_RESULT_SCHEMA: &str = "WeaponryBlenderKnifeWorkerResult@1";
pub const KNIFE_OPERATION: &str = "knife_high_low_uv_bake@1";
pub const KNIFE_WORKER_ID: &str = "weaponry-blender-knife-worker@1";
pub const KNIFE_WORKER_VERSION: &str = "0.1.0";
/// The packaged provider is intentionally pinned to one Blender LTS line.
/// The fixed binary build identity is supplied by the package manifest.  The
/// checked-in development manifest uses Blender's 12-character build hash;
/// release packaging must additionally preserve the full source-offer and
/// SBOM identity before this provider can become release eligible.
pub const KNIFE_BLENDER_VERSION: &str = "5.2.1";
pub const KNIFE_BLENDER_REVISION: &str = "9e2066aef7ef";
pub const KNIFE_RECIPE_ID: &str = "weaponry.knife.blender.high-low-uv-bake@1";
pub const KNIFE_RECIPE_SHA256: &str =
    "2252319ee179752fce75ad83bd95e49e6df5f6736205894b9ed21de66d474321";
pub const KNIFE_INPUT_RELATIVE_PATH: &str = "input/source.glb";
pub const KNIFE_OUTPUT_DIRECTORY: &str = "output";
pub const KNIFE_ENTRYPOINT_RELATIVE_PATH: &str = "apps/blender-worker/weaponry_knife_worker.py";
pub const KNIFE_SOURCE_MANIFEST_RELATIVE_PATH: &str = "apps/blender-worker/manifest.json";
pub const KNIFE_REPOSITORY_BLENDER_RELATIVE_PATH: &str =
    "apps/desktop/src-tauri/target/weaponry-blender-runtime/Blender.app/Contents/MacOS/Blender";
pub const KNIFE_PACKAGED_MANIFEST_RELATIVE_PATH: &str = "weaponry-blender-worker-manifest.json";
pub const KNIFE_PACKAGED_ENTRYPOINT_RELATIVE_PATH: &str = "worker/weaponry_knife_worker.py";
pub const KNIFE_PACKAGED_SOURCE_MANIFEST_RELATIVE_PATH: &str = "worker/source-manifest.json";
pub const KNIFE_PACKAGED_BLENDER_RELATIVE_PATH: &str = "runtime/Blender.app/Contents/MacOS/Blender";
pub const KNIFE_PACKAGED_MANIFEST_SCHEMA: &str = "WeaponryBlenderPackagedWorkerManifest@1";
pub const KNIFE_SOURCE_MANIFEST_SCHEMA: &str = "WeaponryBlenderFixedWorkerManifest@1";
pub const KNIFE_BLENDER_EXECUTABLE_SHA256: &str =
    "ea651e507c6b197df0e234bfa04e5ed43e7f4d498267a7df93fcb38f21928a5c";
pub const KNIFE_DEPENDENCY_LOCK_SHA256: &str =
    "1d5086f15638f656d864e27c2a2129eaa2292a16f928742c761426b7ee101f11";
pub const KNIFE_MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
pub const KNIFE_MAX_STDOUT_BYTES: usize = 64 * 1024;
pub const KNIFE_MAX_INPUT_BYTES: usize = 96 * 1024 * 1024;
pub const KNIFE_MAX_OUTPUT_BYTES: usize = 200 * 1024 * 1024;
pub const KNIFE_MAX_RUNTIME_MS: u64 = 120_000;
pub const KNIFE_MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
pub const KNIFE_MAX_TRIANGLES: u32 = 250_000;
pub const KNIFE_TEXTURE_SIZE: u32 = 512;
pub const KNIFE_MAX_OBJECTS: u32 = 128;

const SCRATCH_PREFIX: &str = "weaponry-blender-knife";
const MAX_ID_BYTES: usize = 128;
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeInputGlb {
    pub kind: String,
    pub relative_path: String,
    pub sha256: String,
    pub byte_size: u64,
    pub mime: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeBudgets {
    pub max_runtime_ms: u64,
    pub max_memory_bytes: u64,
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub max_triangles: u32,
    pub texture_size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifePolicies {
    pub network_policy: String,
    pub filesystem_policy: String,
    pub script_policy: String,
    pub output_policy: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeWorkerRequest {
    pub schema_version: String,
    pub operation: String,
    pub request_id: String,
    pub project_id: String,
    pub candidate_id: String,
    pub input_glb: KnifeInputGlb,
    pub recipe_id: String,
    pub recipe_sha256: String,
    pub budgets: KnifeBudgets,
    pub policies: KnifePolicies,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeWorkerResponse {
    pub schema_version: String,
    pub protocol: String,
    pub request_id: String,
    pub operation: String,
    pub ok: bool,
    pub result: Option<Value>,
    pub error: Option<KnifeWorkerErrorEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnifeWorkerErrorEnvelope {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct KnifeBlenderInstall {
    pub root: PathBuf,
    pub blender_executable: PathBuf,
    pub entrypoint: PathBuf,
    /// Hashes re-checked at construction and immediately before every spawn.
    /// Keeping these identities on the install prevents a caller from
    /// replacing the fixed sidecar or Python entrypoint after bootstrap.
    pub blender_executable_sha256: String,
    pub entrypoint_sha256: String,
    pub blender_version: String,
    pub blender_revision: String,
    pub worker_bundle_sha256: String,
    pub dependency_lock_sha256: String,
}

impl KnifeBlenderInstall {
    /// Resolve the checked-in repository layout.
    ///
    /// This is deliberately a development-only convenience.  The Blender
    /// binary, Python entrypoint and source manifest are still fixed paths and
    /// are checked against the same immutable identity as a packaged worker.
    pub fn from_repository_root(root: impl AsRef<Path>) -> Result<Self, KnifeWorkerError> {
        let root = canonical_root(root)?;
        let entrypoint = join_fixed_relative(&root, KNIFE_ENTRYPOINT_RELATIVE_PATH)?;
        let source_manifest = join_fixed_relative(&root, KNIFE_SOURCE_MANIFEST_RELATIVE_PATH)?;
        let blender_executable =
            join_fixed_relative(&root, KNIFE_REPOSITORY_BLENDER_RELATIVE_PATH)?;

        let source_manifest_value = read_json_file(&source_manifest, 2 * 1024 * 1024)?;
        let source_metadata = validate_source_manifest(&source_manifest_value)?;
        let entrypoint_sha256 = source_metadata
            .entrypoint_sha256
            .unwrap_or(sha256_file(&entrypoint)?);
        verify_file_hash(&entrypoint, &entrypoint_sha256, "Worker entrypoint")?;
        verify_file_hash(
            &blender_executable,
            KNIFE_BLENDER_EXECUTABLE_SHA256,
            "Blender executable",
        )?;

        let install = Self {
            root,
            blender_executable,
            entrypoint,
            blender_executable_sha256: KNIFE_BLENDER_EXECUTABLE_SHA256.to_owned(),
            entrypoint_sha256: entrypoint_sha256.clone(),
            blender_version: source_metadata.blender_version,
            blender_revision: source_metadata.blender_revision,
            worker_bundle_sha256: entrypoint_sha256,
            dependency_lock_sha256: source_metadata.dependency_lock_sha256,
        };
        install.validate_metadata()?;
        Ok(install)
    }

    /// Resolve a staged package produced by `stage_weaponry_blender_worker.py`.
    ///
    /// The package manifest is the only source of layout metadata, but it is
    /// not trusted on its own: its canonical hash, resource-tree hash, fixed
    /// source manifest, entrypoint bytes and Blender executable bytes are all
    /// re-read and verified before an install can be constructed.  No caller
    /// supplied executable, script, add-on, URL or environment is accepted.
    pub fn from_packaged_manifest(root: impl AsRef<Path>) -> Result<Self, KnifeWorkerError> {
        let root = canonical_root(root)?;
        let manifest_path = join_fixed_relative(&root, KNIFE_PACKAGED_MANIFEST_RELATIVE_PATH)?;
        let manifest = read_json_file(&manifest_path, 2 * 1024 * 1024)?;
        let package_metadata = validate_packaged_manifest(&manifest, &root)?;
        let entrypoint = join_fixed_relative(&root, KNIFE_PACKAGED_ENTRYPOINT_RELATIVE_PATH)?;
        let source_manifest =
            join_fixed_relative(&root, KNIFE_PACKAGED_SOURCE_MANIFEST_RELATIVE_PATH)?;
        let blender_executable = join_fixed_relative(&root, KNIFE_PACKAGED_BLENDER_RELATIVE_PATH)?;

        verify_file_hash(
            &entrypoint,
            &package_metadata.entrypoint_sha256,
            "packaged Worker entrypoint",
        )?;
        verify_file_hash(
            &source_manifest,
            &package_metadata.source_manifest_sha256,
            "packaged Worker source manifest",
        )?;
        verify_file_hash(
            &blender_executable,
            &package_metadata.blender_executable_sha256,
            "packaged Blender executable",
        )?;
        let source_manifest_value = read_json_file(&source_manifest, 2 * 1024 * 1024)?;
        let source_metadata = validate_source_manifest(&source_manifest_value)?;
        if source_metadata.entrypoint_sha256.as_deref()
            != Some(package_metadata.entrypoint_sha256.as_str())
            || source_metadata.dependency_lock_sha256 != package_metadata.dependency_lock_sha256
        {
            return Err(KnifeWorkerError::Hash(
                "packaged Worker source manifest identity differs from its package manifest"
                    .to_owned(),
            ));
        }

        let install = Self {
            root,
            blender_executable,
            entrypoint,
            blender_executable_sha256: package_metadata.blender_executable_sha256,
            entrypoint_sha256: package_metadata.entrypoint_sha256.clone(),
            blender_version: package_metadata.blender_version,
            blender_revision: package_metadata.blender_revision,
            worker_bundle_sha256: package_metadata.resource_tree_sha256,
            dependency_lock_sha256: package_metadata.dependency_lock_sha256,
        };
        install.validate_metadata()?;
        Ok(install)
    }

    /// Resolve either the checked-in repository layout or a packaged layout.
    /// Packaged manifests take precedence when present.  This helper is useful
    /// for a Runtime bootstrap that has one trusted resource-root handle.
    pub fn from_fixed_root(root: impl AsRef<Path>) -> Result<Self, KnifeWorkerError> {
        let root = root.as_ref();
        if root.join(KNIFE_PACKAGED_MANIFEST_RELATIVE_PATH).is_file() {
            Self::from_packaged_manifest(root)
        } else {
            Self::from_repository_root(root)
        }
    }

    /// Construct install metadata from the legacy call shape.
    ///
    /// The old API accepted caller-selected relative paths.  Keep it as a
    /// source-compatible shim for existing code, but only accept the two
    /// fixed layouts and discard all caller-provided identity values in favor
    /// of the verified manifests above.
    #[deprecated(note = "use from_packaged_manifest or from_repository_root")]
    pub fn from_packaged_root(
        root: impl AsRef<Path>,
        blender_executable_relative: &str,
        entrypoint_relative: &str,
        blender_version: impl Into<String>,
        blender_revision: impl Into<String>,
        worker_bundle_sha256: impl Into<String>,
        dependency_lock_sha256: impl Into<String>,
    ) -> Result<Self, KnifeWorkerError> {
        let _ = (
            blender_version.into(),
            blender_revision.into(),
            worker_bundle_sha256.into(),
        );
        let _ = dependency_lock_sha256.into();
        let root = root.as_ref();
        if blender_executable_relative == KNIFE_PACKAGED_BLENDER_RELATIVE_PATH
            && entrypoint_relative == KNIFE_PACKAGED_ENTRYPOINT_RELATIVE_PATH
        {
            return Self::from_packaged_manifest(root);
        }
        if blender_executable_relative == KNIFE_REPOSITORY_BLENDER_RELATIVE_PATH
            && entrypoint_relative == KNIFE_ENTRYPOINT_RELATIVE_PATH
        {
            return Self::from_repository_root(root);
        }
        Err(KnifeWorkerError::Invalid(
            "caller-selected Blender paths are not allowed; use a fixed repository or package layout"
                .to_owned(),
        ))
    }

    fn validate_metadata(&self) -> Result<(), KnifeWorkerError> {
        if self.blender_version != KNIFE_BLENDER_VERSION
            || self.blender_revision != KNIFE_BLENDER_REVISION
            || self.blender_executable_sha256 != KNIFE_BLENDER_EXECUTABLE_SHA256
            || !is_sha256(&self.entrypoint_sha256)
            || !is_sha256(&self.worker_bundle_sha256)
            || !is_sha256(&self.dependency_lock_sha256)
            || self.entrypoint.file_name().and_then(|name| name.to_str())
                != Some("weaponry_knife_worker.py")
        {
            return Err(KnifeWorkerError::Invalid(
                "fixed Blender install metadata is invalid".to_owned(),
            ));
        }
        Ok(())
    }

    /// Re-check the immutable bytes immediately before invoking Blender.
    /// Package construction already performs the same checks; this second
    /// check closes the time-of-check/time-of-use gap for a long-lived Runtime
    /// install handle.
    fn verify_runtime_files(&self) -> Result<(), KnifeWorkerError> {
        verify_file_hash(
            &self.blender_executable,
            &self.blender_executable_sha256,
            "fixed Blender executable",
        )?;
        verify_file_hash(
            &self.entrypoint,
            &self.entrypoint_sha256,
            "fixed Worker entrypoint",
        )
    }
}

#[derive(Debug, Clone)]
pub struct KnifeBlenderWorker {
    install: KnifeBlenderInstall,
    packaged: bool,
}

#[derive(Debug, Clone)]
pub struct KnifeWorkerRun {
    pub response: KnifeWorkerResponse,
    pub artifacts: BTreeMap<String, Vec<u8>>,
    pub blender_version: String,
    pub blender_revision: String,
    pub worker_id: String,
    pub worker_bundle_sha256: String,
    pub dependency_lock_sha256: String,
    pub input_sha256: String,
    pub stdout_sha256: String,
    pub packaged: bool,
}

impl KnifeWorkerRun {
    /// Runtime-facing identity projection.  This is a receipt projection only;
    /// it does not imply visual, human, engine, or production-stage success.
    pub fn identity_projection(&self) -> Value {
        let outputs = self
            .artifacts
            .iter()
            .map(|(path, bytes)| {
                serde_json::json!({
                    "relative_path": path,
                    "sha256": sha256_hex(bytes),
                    "byte_size": bytes.len()
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "schema_version":"WeaponryBlenderKnifeWorkerIdentity@1",
            "protocol":KNIFE_WORKER_PROTOCOL,
            "worker_id":self.worker_id,
            "blender_version":self.blender_version,
            "blender_revision":self.blender_revision,
            "worker_bundle_sha256":self.worker_bundle_sha256,
            "dependency_lock_sha256":self.dependency_lock_sha256,
            "input_sha256":self.input_sha256,
            "stdout_sha256":self.stdout_sha256,
            "outputs":outputs,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "canonical_sha256":""
        })
    }
}

impl KnifeBlenderWorker {
    pub fn new(install: KnifeBlenderInstall) -> Result<Self, KnifeWorkerError> {
        install.validate_metadata()?;
        install.verify_runtime_files()?;
        Ok(Self {
            packaged: install
                .root
                .join(KNIFE_PACKAGED_MANIFEST_RELATIVE_PATH)
                .is_file(),
            install,
        })
    }

    pub fn install(&self) -> &KnifeBlenderInstall {
        &self.install
    }

    /// Parse one bounded, closed JSON request and execute the fixed Worker.
    /// This is the Runtime-facing convenience API; the source bytes still
    /// arrive out-of-band from Runtime-owned CAS/readback and are never
    /// represented as a caller-controlled path in the JSON contract.
    pub fn run_json(
        &self,
        request_bytes: &[u8],
        source_glb: &[u8],
    ) -> Result<KnifeWorkerRun, KnifeWorkerError> {
        if request_bytes.is_empty() || request_bytes.len() > KNIFE_MAX_REQUEST_BYTES {
            return Err(KnifeWorkerError::Resource(
                "Blender knife request exceeds its byte bound".to_owned(),
            ));
        }
        let request = serde_json::from_slice::<KnifeWorkerRequest>(request_bytes)
            .map_err(KnifeWorkerError::Json)?;
        self.run(&request, source_glb)
    }

    /// Execute against a Runtime-created staging root whose only input is the
    /// fixed `input/source.glb` path.  The root must already be isolated and
    /// owned by the caller; this method never accepts a path from JSON and
    /// never follows a symlink input.  Output files remain in that root for a
    /// CLI caller to consume, while the returned run still contains verified
    /// in-memory bytes for a Runtime caller.
    pub fn run_from_staged_root(
        &self,
        request: &KnifeWorkerRequest,
        staged_root: impl AsRef<Path>,
    ) -> Result<KnifeWorkerRun, KnifeWorkerError> {
        validate_request(request)?;
        let staged_root = canonical_root(staged_root)?;
        let input_path = safe_join_input(&staged_root, KNIFE_INPUT_RELATIVE_PATH)?;
        let input_metadata = fs::symlink_metadata(&input_path).map_err(KnifeWorkerError::Io)?;
        if !input_metadata.file_type().is_file() {
            return Err(KnifeWorkerError::Invalid(
                "fixed staged Worker input is not a regular file".to_owned(),
            ));
        }
        let source_glb = read_limited_file(&input_path, KNIFE_MAX_INPUT_BYTES)?;
        if source_glb.len() as u64 != request.input_glb.byte_size
            || sha256_hex(&source_glb) != request.input_glb.sha256
        {
            return Err(KnifeWorkerError::Hash(
                "fixed staged Worker input differs from the request".to_owned(),
            ));
        }
        self.run_in_scratch(request, &source_glb, Some(staged_root.as_path()))
    }

    pub fn run(
        &self,
        request: &KnifeWorkerRequest,
        source_glb: &[u8],
    ) -> Result<KnifeWorkerRun, KnifeWorkerError> {
        validate_request(request)?;
        if source_glb.len() as u64 != request.input_glb.byte_size
            || source_glb.len() > KNIFE_MAX_INPUT_BYTES
            || sha256_hex(source_glb) != request.input_glb.sha256
        {
            return Err(KnifeWorkerError::Hash(
                "staged source GLB bytes do not match the request".to_owned(),
            ));
        }
        let scratch = ScratchDir::create()?;
        self.run_in_scratch(request, source_glb, Some(scratch.path()))
    }

    fn run_in_scratch(
        &self,
        request: &KnifeWorkerRequest,
        source_glb: &[u8],
        scratch_root: Option<&Path>,
    ) -> Result<KnifeWorkerRun, KnifeWorkerError> {
        validate_request(request)?;
        self.install.verify_runtime_files()?;
        let owned_scratch;
        let scratch_path = if let Some(scratch_root) = scratch_root {
            scratch_root
        } else {
            owned_scratch = ScratchDir::create()?;
            owned_scratch.path()
        };
        let input_path = scratch_path.join(KNIFE_INPUT_RELATIVE_PATH);
        ensure_fixed_directory(scratch_path, "input")?;
        ensure_output_directory(scratch_path)?;
        match fs::symlink_metadata(&input_path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                let existing = read_limited_file(&input_path, KNIFE_MAX_INPUT_BYTES)?;
                if existing != source_glb {
                    return Err(KnifeWorkerError::Hash(
                        "fixed staged Worker input differs from the requested bytes".to_owned(),
                    ));
                }
            }
            Ok(_) => {
                return Err(KnifeWorkerError::Invalid(
                    "fixed staged Worker input is not a regular file".to_owned(),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::write(&input_path, source_glb).map_err(KnifeWorkerError::Io)?;
            }
            Err(error) => return Err(KnifeWorkerError::Io(error)),
        }

        let request_bytes =
            canonical_json_bytes(&serde_json::to_value(request).map_err(KnifeWorkerError::Json)?)
                .map_err(KnifeWorkerError::Invalid)?;
        if request_bytes.len() > KNIFE_MAX_REQUEST_BYTES {
            return Err(KnifeWorkerError::Resource(
                "Blender knife request exceeds its byte bound".to_owned(),
            ));
        }
        let mut command = Command::new(&self.install.blender_executable);
        command
            .env_clear()
            .arg("--background")
            .arg("--factory-startup")
            .arg("--disable-autoexec")
            // Blender's subdivision/dependency-graph evaluation may otherwise
            // choose different worker-thread traversal orders for the same
            // mesh.  The knife provider values reproducible artifact bytes
            // over throughput, so both the global worker pool and depsgraph
            // evaluation are fixed to one thread.
            .arg("--threads")
            .arg("1")
            .arg("--debug-depsgraph-no-threads")
            .arg("--python-exit-code")
            .arg("1")
            .arg("--python")
            .arg(&self.install.entrypoint)
            .arg("--")
            .arg("--scratch-root")
            .arg(scratch_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(KnifeWorkerError::Io)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| KnifeWorkerError::Invalid("Blender stdin unavailable".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| KnifeWorkerError::Invalid("Blender stdout unavailable".to_owned()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| KnifeWorkerError::Invalid("Blender stderr unavailable".to_owned()))?;
        let stdin_thread = std::thread::spawn(move || write_request(stdin, &request_bytes));
        let stdout_thread =
            std::thread::spawn(move || read_limited(stdout, KNIFE_MAX_STDOUT_BYTES));
        let stderr_thread = std::thread::spawn(move || read_limited(stderr, 64 * 1024));
        let (status, timed_out) = wait_bounded(
            &mut child,
            Duration::from_millis(request.budgets.max_runtime_ms),
        )?;
        let _ = stdin_thread.join();
        let stdout = stdout_thread
            .join()
            .map_err(|_| KnifeWorkerError::Invalid("Blender stdout reader panicked".to_owned()))?
            .map_err(KnifeWorkerError::Io)?;
        let stderr = stderr_thread
            .join()
            .map_err(|_| KnifeWorkerError::Invalid("Blender stderr reader panicked".to_owned()))?
            .map_err(KnifeWorkerError::Io)?;
        if timed_out {
            return Err(KnifeWorkerError::Timeout {
                stderr: bounded_text(&stderr),
            });
        }
        // Blender's background mode writes informational lines to stdout.  Do
        // not accept arbitrary JSON or relax the response schema: the Python
        // worker emits exactly one canonical response line, which is the only
        // line allowed to cross this boundary.
        let response = extract_response(&stdout)?;
        validate_response(&response, request)?;
        validate_install_bound_response(&response, &self.install)?;
        if !response.ok {
            let error = response
                .error
                .as_ref()
                .expect("validated failed response has an error");
            return Err(KnifeWorkerError::WorkerRejected {
                code: error.code.clone(),
                message: error.message.clone(),
            });
        }
        if !status.success() {
            return Err(KnifeWorkerError::WorkerExit {
                code: status.code(),
                stderr: bounded_text(&stderr),
            });
        }
        let artifacts =
            collect_artifacts(&response, scratch_path, request.budgets.max_output_bytes)?;
        let run = KnifeWorkerRun {
            response,
            artifacts,
            blender_version: self.install.blender_version.clone(),
            blender_revision: self.install.blender_revision.clone(),
            worker_id: KNIFE_WORKER_ID.to_owned(),
            worker_bundle_sha256: self.install.worker_bundle_sha256.clone(),
            dependency_lock_sha256: self.install.dependency_lock_sha256.clone(),
            input_sha256: request.input_glb.sha256.clone(),
            stdout_sha256: sha256_hex(&stdout),
            packaged: self.packaged,
        };
        Ok(run)
    }
}

pub fn validate_request(request: &KnifeWorkerRequest) -> Result<(), KnifeWorkerError> {
    if request.schema_version != KNIFE_REQUEST_SCHEMA
        || request.operation != KNIFE_OPERATION
        || !is_id(&request.request_id)
        || !is_id(&request.project_id)
        || !is_id(&request.candidate_id)
        || request.recipe_id != KNIFE_RECIPE_ID
        || request.recipe_sha256 != KNIFE_RECIPE_SHA256
    {
        return Err(KnifeWorkerError::Invalid(
            "knife Worker request marker or recipe is not allowlisted".to_owned(),
        ));
    }
    if request.input_glb.kind != "authoring_mesh_glb"
        || request.input_glb.relative_path != KNIFE_INPUT_RELATIVE_PATH
        || request.input_glb.mime != "model/gltf-binary"
        || !is_sha256(&request.input_glb.sha256)
        || request.input_glb.byte_size == 0
        || request.input_glb.byte_size > KNIFE_MAX_INPUT_BYTES as u64
    {
        return Err(KnifeWorkerError::Invalid(
            "knife Worker input descriptor is invalid".to_owned(),
        ));
    }
    if request.budgets.max_runtime_ms == 0
        || request.budgets.max_runtime_ms > KNIFE_MAX_RUNTIME_MS
        || request.budgets.max_memory_bytes == 0
        || request.budgets.max_memory_bytes > KNIFE_MAX_MEMORY_BYTES
        || request.budgets.max_input_bytes < request.input_glb.byte_size
        || request.budgets.max_input_bytes > KNIFE_MAX_INPUT_BYTES as u64
        || request.budgets.max_output_bytes == 0
        || request.budgets.max_output_bytes > KNIFE_MAX_OUTPUT_BYTES as u64
        || request.budgets.max_triangles == 0
        || request.budgets.max_triangles > KNIFE_MAX_TRIANGLES
        || request.budgets.texture_size != KNIFE_TEXTURE_SIZE
    {
        return Err(KnifeWorkerError::Resource(
            "knife Worker budget is outside fixed bounds".to_owned(),
        ));
    }
    let expected_policies = KnifePolicies {
        network_policy: "disabled".to_owned(),
        filesystem_policy: "runtime_scratch_only".to_owned(),
        script_policy: "frozen_bundle_only".to_owned(),
        output_policy: "temporary_observation_runtime_adoption".to_owned(),
    };
    if request.policies != expected_policies {
        return Err(KnifeWorkerError::Invalid(
            "knife Worker policies are not the fixed policy".to_owned(),
        ));
    }
    let supplied = &request.canonical_sha256;
    if !is_sha256(supplied) || request_canonical_sha256(request)? != *supplied {
        return Err(KnifeWorkerError::Hash(
            "knife Worker request canonical hash does not match".to_owned(),
        ));
    }
    let bytes =
        canonical_json_bytes(&serde_json::to_value(request).map_err(KnifeWorkerError::Json)?)
            .map_err(KnifeWorkerError::Invalid)?;
    if bytes.len() > KNIFE_MAX_REQUEST_BYTES {
        return Err(KnifeWorkerError::Resource(
            "knife Worker request exceeds its bounded envelope".to_owned(),
        ));
    }
    Ok(())
}

pub fn validate_response(
    response: &KnifeWorkerResponse,
    request: &KnifeWorkerRequest,
) -> Result<(), KnifeWorkerError> {
    validate_request(request)?;
    if response.schema_version != KNIFE_RESPONSE_SCHEMA
        || response.protocol != KNIFE_WORKER_PROTOCOL
        || response.request_id != request.request_id
        || response.operation != KNIFE_OPERATION
    {
        return Err(KnifeWorkerError::Protocol(
            "knife Worker response identity is invalid".to_owned(),
        ));
    }
    if response.ok {
        if response.error.is_some() {
            return Err(KnifeWorkerError::Protocol(
                "successful knife Worker response contains an error".to_owned(),
            ));
        }
        let result = response.result.as_ref().ok_or_else(|| {
            KnifeWorkerError::Protocol("successful knife Worker response lacks result".to_owned())
        })?;
        validate_result(result, request)?;
    } else {
        if response.result.is_some() {
            return Err(KnifeWorkerError::Protocol(
                "failed knife Worker response contains a result".to_owned(),
            ));
        }
        let error = response.error.as_ref().ok_or_else(|| {
            KnifeWorkerError::Protocol("failed knife Worker response lacks error".to_owned())
        })?;
        if !is_id(&error.code) || error.message.is_empty() || error.message.len() > 512 {
            return Err(KnifeWorkerError::Protocol(
                "knife Worker error envelope is invalid".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_install_bound_response(
    response: &KnifeWorkerResponse,
    install: &KnifeBlenderInstall,
) -> Result<(), KnifeWorkerError> {
    if !response.ok {
        return Ok(());
    }
    let result = response
        .result
        .as_ref()
        .and_then(Value::as_object)
        .ok_or_else(|| {
            KnifeWorkerError::Protocol("validated Worker result is missing".to_owned())
        })?;
    let entrypoint_sha256 = result
        .get("worker_entrypoint_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            KnifeWorkerError::Protocol(
                "Worker entrypoint hash is missing from its result".to_owned(),
            )
        })?;
    if entrypoint_sha256 != install.entrypoint_sha256
        || result.get("dependency_lock_sha256").and_then(Value::as_str)
            != Some(install.dependency_lock_sha256.as_str())
        || result.get("blender_version").and_then(Value::as_str)
            != Some(install.blender_version.as_str())
        || result.get("blender_revision").and_then(Value::as_str)
            != Some(install.blender_revision.as_str())
        || result.get("blender_build_hash").and_then(Value::as_str)
            != Some(install.blender_revision.as_str())
    {
        return Err(KnifeWorkerError::Hash(
            "Worker result identity differs from the fixed install".to_owned(),
        ));
    }
    Ok(())
}

fn extract_response(stdout: &[u8]) -> Result<KnifeWorkerResponse, KnifeWorkerError> {
    let mut response = None;
    for line in stdout.split(|byte| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(candidate) = serde_json::from_slice::<KnifeWorkerResponse>(line) else {
            continue;
        };
        if response.is_some() {
            return Err(KnifeWorkerError::Protocol(
                "stdout contains more than one closed JSON response".to_owned(),
            ));
        }
        response = Some(candidate);
    }
    response.ok_or_else(|| {
        KnifeWorkerError::Protocol("stdout contains no closed JSON worker response".to_owned())
    })
}

fn validate_result(value: &Value, request: &KnifeWorkerRequest) -> Result<(), KnifeWorkerError> {
    let object = value.as_object().ok_or_else(|| {
        KnifeWorkerError::Protocol("knife Worker result is not an object".to_owned())
    })?;
    require_exact_fields(
        object,
        &[
            "schema_version",
            "operation",
            "request_id",
            "project_id",
            "candidate_id",
            "source_authoring_mesh_sha256",
            "recipe_sha256",
            "policy",
            "worker_id",
            "worker_version",
            "blender_version",
            "blender_revision",
            "blender_build_hash",
            "worker_entrypoint_sha256",
            "dependency_lock_sha256",
            "input_canonical_sha256",
            "outputs",
            "stats",
            "checks",
            "runtime_write_performed",
            "stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "canonical_sha256",
        ],
        "knife Worker result",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(KNIFE_RESULT_SCHEMA)
        || object.get("operation").and_then(Value::as_str) != Some(KNIFE_OPERATION)
        || object.get("request_id").and_then(Value::as_str) != Some(request.request_id.as_str())
        || object.get("project_id").and_then(Value::as_str) != Some(request.project_id.as_str())
        || object.get("candidate_id").and_then(Value::as_str) != Some(request.candidate_id.as_str())
        || object
            .get("source_authoring_mesh_sha256")
            .and_then(Value::as_str)
            != Some(request.input_glb.sha256.as_str())
        || object.get("recipe_sha256").and_then(Value::as_str) != Some(KNIFE_RECIPE_SHA256)
        || object.get("worker_id").and_then(Value::as_str) != Some(KNIFE_WORKER_ID)
        || object.get("worker_version").and_then(Value::as_str) != Some(KNIFE_WORKER_VERSION)
        || object.get("blender_version").and_then(Value::as_str) != Some(KNIFE_BLENDER_VERSION)
        || object.get("blender_revision").and_then(Value::as_str) != Some(KNIFE_BLENDER_REVISION)
        || object.get("blender_build_hash").and_then(Value::as_str) != Some(KNIFE_BLENDER_REVISION)
        || object.get("policy").and_then(Value::as_str)
            != Some("fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1")
        || !object
            .get("worker_entrypoint_sha256")
            .and_then(Value::as_str)
            .is_some_and(is_sha256)
        || object.get("dependency_lock_sha256").and_then(Value::as_str)
            != Some(KNIFE_DEPENDENCY_LOCK_SHA256)
        || object.get("input_canonical_sha256").and_then(Value::as_str)
            != Some(request.canonical_sha256.as_str())
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("stage_advanced").and_then(Value::as_bool) != Some(false)
        || object.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || object.get("version_created").and_then(Value::as_bool) != Some(false)
        || object.get("export_performed").and_then(Value::as_bool) != Some(false)
    {
        return Err(KnifeWorkerError::Protocol(
            "knife Worker result identity or non-promoting flags drifted".to_owned(),
        ));
    }
    let supplied = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            KnifeWorkerError::Protocol("knife Worker result hash is missing".to_owned())
        })?;
    if !is_sha256(supplied) || canonical_without_field(value, "canonical_sha256")? != supplied {
        return Err(KnifeWorkerError::Hash(
            "knife Worker result canonical hash does not match".to_owned(),
        ));
    }
    let outputs = object
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            KnifeWorkerError::Protocol("knife Worker result outputs are missing".to_owned())
        })?;
    if outputs.is_empty() || outputs.len() > 512 {
        return Err(KnifeWorkerError::Resource(
            "knife Worker output count is outside its bound".to_owned(),
        ));
    }
    let mut paths = std::collections::BTreeSet::new();
    let allowed_kinds = [
        "high_glb",
        "low_glb",
        "normal_map",
        "ao_map",
        "worker_manifest",
    ];
    for output in outputs {
        let record = output.as_object().ok_or_else(|| {
            KnifeWorkerError::Protocol("knife Worker output record is not an object".to_owned())
        })?;
        require_exact_fields(
            record,
            &[
                "kind",
                "relative_path",
                "mime",
                "byte_size",
                "sha256",
                "cas_owner",
                "durability",
            ],
            "knife Worker output",
        )?;
        let relative = record
            .get("relative_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                KnifeWorkerError::Protocol("knife Worker output path is missing".to_owned())
            })?;
        if !safe_output_relative(relative) || !paths.insert(relative.to_owned()) {
            return Err(KnifeWorkerError::Protocol(
                "knife Worker output path is unsafe or duplicated".to_owned(),
            ));
        }
        if !record
            .get("kind")
            .and_then(Value::as_str)
            .is_some_and(|kind| allowed_kinds.contains(&kind))
            || !record
                .get("mime")
                .and_then(Value::as_str)
                .is_some_and(|mime| {
                    mime == "model/gltf-binary" || mime == "image/png" || mime == "application/json"
                })
            || !record
                .get("sha256")
                .and_then(Value::as_str)
                .is_some_and(is_sha256)
            || !record
                .get("byte_size")
                .and_then(Value::as_u64)
                .is_some_and(|size| size > 0 && size <= KNIFE_MAX_OUTPUT_BYTES as u64)
        {
            return Err(KnifeWorkerError::Protocol(
                "knife Worker output record is invalid".to_owned(),
            ));
        }
    }
    let stats = object
        .get("stats")
        .and_then(Value::as_object)
        .ok_or_else(|| KnifeWorkerError::Protocol("knife Worker stats are missing".to_owned()))?;
    require_exact_fields(
        stats,
        &[
            "source_object_count",
            "high_object_count",
            "low_object_count",
            "source_triangle_count",
            "high_triangle_count",
            "low_triangle_count",
            "bake_map_count",
            "texture_size",
        ],
        "knife Worker stats",
    )?;
    if stats.get("texture_size").and_then(Value::as_u64) != Some(KNIFE_TEXTURE_SIZE as u64) {
        return Err(KnifeWorkerError::Protocol(
            "knife Worker texture size drifted".to_owned(),
        ));
    }
    let checks = object
        .get("checks")
        .and_then(Value::as_object)
        .ok_or_else(|| KnifeWorkerError::Protocol("knife Worker checks are missing".to_owned()))?;
    require_exact_fields(
        checks,
        &[
            "validator_status",
            "readback_status",
            "deterministic_replay_status",
            "stage_eligibility",
            "human_status",
            "engine_status",
        ],
        "knife Worker checks",
    )?;
    Ok(())
}

fn require_exact_fields(
    object: &Map<String, Value>,
    expected: &[&str],
    label: &str,
) -> Result<(), KnifeWorkerError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(KnifeWorkerError::Protocol(format!(
            "{label} fields are not closed"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SourceManifestMetadata {
    blender_version: String,
    blender_revision: String,
    entrypoint_sha256: Option<String>,
    dependency_lock_sha256: String,
}

#[derive(Debug, Clone)]
struct PackagedManifestMetadata {
    blender_version: String,
    blender_revision: String,
    blender_executable_sha256: String,
    entrypoint_sha256: String,
    source_manifest_sha256: String,
    dependency_lock_sha256: String,
    resource_tree_sha256: String,
}

fn canonical_root(root: impl AsRef<Path>) -> Result<PathBuf, KnifeWorkerError> {
    let root = fs::canonicalize(root).map_err(KnifeWorkerError::Io)?;
    if !root.is_dir() {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker resource root is not a directory".to_owned(),
        ));
    }
    Ok(root)
}

fn read_json_file(path: &Path, limit: usize) -> Result<Value, KnifeWorkerError> {
    let metadata = fs::symlink_metadata(path).map_err(KnifeWorkerError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker manifest is not a regular file".to_owned(),
        ));
    }
    let bytes = read_limited_file(path, limit)?;
    serde_json::from_slice(&bytes).map_err(KnifeWorkerError::Json)
}

fn verify_file_hash(path: &Path, expected: &str, label: &str) -> Result<(), KnifeWorkerError> {
    if !is_sha256(expected) {
        return Err(KnifeWorkerError::Hash(format!(
            "{label} expected hash is invalid"
        )));
    }
    let metadata = fs::symlink_metadata(path).map_err(KnifeWorkerError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(KnifeWorkerError::Invalid(format!(
            "{label} is not a regular file"
        )));
    }
    #[cfg(unix)]
    if path.ends_with(KNIFE_REPOSITORY_BLENDER_RELATIVE_PATH)
        || path.ends_with(KNIFE_PACKAGED_BLENDER_RELATIVE_PATH)
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(KnifeWorkerError::Invalid(format!(
                "{label} is not executable"
            )));
        }
    }
    let actual = sha256_file(path)?;
    if actual != expected {
        return Err(KnifeWorkerError::Hash(format!(
            "{label} bytes differ from the fixed manifest"
        )));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, KnifeWorkerError> {
    let file = File::open(path).map_err(KnifeWorkerError::Io)?;
    let bytes = read_limited(file, KNIFE_MAX_OUTPUT_BYTES.max(KNIFE_MAX_INPUT_BYTES))
        .map_err(KnifeWorkerError::Io)?;
    Ok(sha256_hex(&bytes))
}

fn validate_source_manifest(value: &Value) -> Result<SourceManifestMetadata, KnifeWorkerError> {
    let object = value.as_object().ok_or_else(|| {
        KnifeWorkerError::Protocol("fixed Worker source manifest is not an object".to_owned())
    })?;
    require_exact_fields(
        object,
        &[
            "schema_version",
            "status",
            "worker_id",
            "worker_version",
            "entrypoint",
            "entrypoint_sha256",
            "entrypoint_hash_policy",
            "protocol",
            "operation",
            "request_schema",
            "response_schema",
            "result_schema",
            "recipe_id",
            "recipe_sha256",
            "dependency_lock_sha256",
            "policy",
            "host",
            "transport",
            "fixed_recipe",
            "limits",
            "output_policy",
            "distribution_gates",
            "runtime_integration",
            "package_status",
            "removal_fallback",
        ],
        "fixed Worker source manifest",
    )?;
    if field_string(object, "schema_version")? != KNIFE_SOURCE_MANIFEST_SCHEMA
        || field_string(object, "status")? != "isolated-prototype"
        || field_string(object, "worker_id")? != KNIFE_WORKER_ID
        || field_string(object, "worker_version")? != KNIFE_WORKER_VERSION
        || field_string(object, "entrypoint")? != KNIFE_ENTRYPOINT_RELATIVE_PATH
        || field_string(object, "entrypoint_hash_policy")? != "DERIVED_FROM_STAGED_ENTRYPOINT_BYTES"
        || field_string(object, "protocol")? != KNIFE_WORKER_PROTOCOL
        || field_string(object, "operation")? != KNIFE_OPERATION
        || field_string(object, "request_schema")? != KNIFE_REQUEST_SCHEMA
        || field_string(object, "response_schema")? != KNIFE_RESPONSE_SCHEMA
        || field_string(object, "result_schema")? != KNIFE_RESULT_SCHEMA
        || field_string(object, "recipe_id")? != KNIFE_RECIPE_ID
        || field_string(object, "recipe_sha256")? != KNIFE_RECIPE_SHA256
        || field_string(object, "dependency_lock_sha256")? != KNIFE_DEPENDENCY_LOCK_SHA256
        || field_string(object, "policy")?
            != "fixed-built-in-bevel-weighted-normal-decimate-smart-uv-cycles-bake@1"
        || field_string(object, "runtime_integration")? != "not_connected"
        || field_string(object, "package_status")? != "not_packaged"
        || field_string(object, "removal_fallback")?
            != "forgecad-native-workers-and-capability-unavailable"
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker source manifest identity is not allowlisted".to_owned(),
        ));
    }

    let entrypoint_sha256 = match object.get("entrypoint_sha256") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_sha256(value) => Some(value.to_owned()),
        _ => {
            return Err(KnifeWorkerError::Hash(
                "fixed Worker entrypoint hash is invalid".to_owned(),
            ));
        }
    };
    let host = object_value(object, "host", "fixed Worker source manifest host")?;
    require_exact_fields(
        host,
        &[
            "blender_version",
            "blender_display_version",
            "source_revision",
            "build_hash",
            "build_branch",
            "build_platform",
            "build_type",
            "bundle_id",
            "team_id",
            "signing_authority",
            "source_license",
            "binary_sha256",
            "bundle_tree_sha256",
            "bundle_file_count",
            "bundle_total_bytes",
            "python_bundle_sha256",
            "license_resources",
            "download_artifact",
        ],
        "fixed Worker source manifest host",
    )?;
    let blender_version = field_string(host, "blender_version")?;
    let blender_revision = field_string(host, "source_revision")?;
    if blender_version != KNIFE_BLENDER_VERSION
        || field_string(host, "blender_display_version")? != "5.2.1 LTS"
        || blender_revision != KNIFE_BLENDER_REVISION
        || field_string(host, "build_hash")? != KNIFE_BLENDER_REVISION
        || field_string(host, "build_branch")? != "blender-v5.2-release"
        || field_string(host, "build_platform")? != "Darwin"
        || field_string(host, "build_type")? != "Release"
        || field_string(host, "bundle_id")? != "org.blenderfoundation.blender"
        || field_string(host, "team_id")? != "68UA947AUU"
        || field_string(host, "signing_authority")?
            != "Developer ID Application: Stichting Blender Foundation (68UA947AUU)"
        || field_string(host, "source_license")? != "GPL-3.0-or-later"
        || field_string(host, "binary_sha256")? != KNIFE_BLENDER_EXECUTABLE_SHA256
        || field_string(host, "bundle_tree_sha256")?
            != "a1719f3e1c7fc846e811de3c9d32ff72f2130016a3290fa88eb4c8e9e1032317"
        || host.get("bundle_file_count").and_then(Value::as_u64) != Some(6498)
        || host.get("bundle_total_bytes").and_then(Value::as_u64) != Some(934431449)
        || field_string(host, "python_bundle_sha256")?
            != "0fd0b83738e588928e8f6008138b8f812f66f9c837d984d6dd803495ce2dc7a0"
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker Blender host identity is not allowlisted".to_owned(),
        ));
    }
    validate_license_resources(object_value(
        host,
        "license_resources",
        "fixed Worker source manifest license resources",
    )?)?;
    validate_download_artifact(object_value(
        host,
        "download_artifact",
        "fixed Worker source manifest download artifact",
    )?)?;

    let transport = object_value(
        object,
        "transport",
        "fixed Worker source manifest transport",
    )?;
    require_exact_fields(
        transport,
        &[
            "kind",
            "network",
            "input_relative_path",
            "output_relative_directory",
            "max_request_bytes",
            "max_stdout_bytes",
            "max_input_bytes",
            "max_output_bytes",
        ],
        "fixed Worker source manifest transport",
    )?;
    if field_string(transport, "kind")? != "stdin-stdout-json-one-shot"
        || transport.get("network").and_then(Value::as_bool) != Some(false)
        || field_string(transport, "input_relative_path")? != KNIFE_INPUT_RELATIVE_PATH
        || field_string(transport, "output_relative_directory")? != KNIFE_OUTPUT_DIRECTORY
        || transport.get("max_request_bytes").and_then(Value::as_u64)
            != Some(KNIFE_MAX_REQUEST_BYTES as u64)
        || transport.get("max_stdout_bytes").and_then(Value::as_u64)
            != Some(KNIFE_MAX_STDOUT_BYTES as u64)
        || transport.get("max_input_bytes").and_then(Value::as_u64)
            != Some(KNIFE_MAX_INPUT_BYTES as u64)
        || transport.get("max_output_bytes").and_then(Value::as_u64)
            != Some(KNIFE_MAX_OUTPUT_BYTES as u64)
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker transport manifest is not allowlisted".to_owned(),
        ));
    }
    validate_fixed_recipe(object_value(
        object,
        "fixed_recipe",
        "fixed Worker source manifest recipe",
    )?)?;
    validate_fixed_limits(object_value(
        object,
        "limits",
        "fixed Worker source manifest limits",
    )?)?;
    validate_fixed_output_policy(object_value(
        object,
        "output_policy",
        "fixed Worker source manifest output policy",
    )?)?;
    let distribution = object_value(
        object,
        "distribution_gates",
        "fixed Worker source manifest distribution gates",
    )?;
    require_exact_fields(
        distribution,
        &[
            "gpl_source_offer",
            "legal_review",
            "notice",
            "spdx_sbom",
            "release_eligible",
        ],
        "fixed Worker source manifest distribution gates",
    )?;
    if field_string(distribution, "gpl_source_offer")? != "NOT_INCLUDED_DEVELOPMENT_STAGING"
        || field_string(distribution, "notice")? != "NOT_INCLUDED_DEVELOPMENT_STAGING"
        || field_string(distribution, "spdx_sbom")? != "NOT_INCLUDED_DEVELOPMENT_STAGING"
        || field_string(distribution, "legal_review")? != "NOT_RUN"
        || distribution
            .get("release_eligible")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker source manifest distribution gates are not pending".to_owned(),
        ));
    }

    Ok(SourceManifestMetadata {
        blender_version: blender_version.to_owned(),
        blender_revision: blender_revision.to_owned(),
        entrypoint_sha256,
        dependency_lock_sha256: KNIFE_DEPENDENCY_LOCK_SHA256.to_owned(),
    })
}

fn validate_packaged_manifest(
    value: &Value,
    root: &Path,
) -> Result<PackagedManifestMetadata, KnifeWorkerError> {
    let object = value.as_object().ok_or_else(|| {
        KnifeWorkerError::Protocol("packaged Worker manifest is not an object".to_owned())
    })?;
    require_exact_fields(
        object,
        &[
            "schema_version",
            "status",
            "blender",
            "worker",
            "policy",
            "distribution_gates",
            "provenance",
            "resource_tree_file_count",
            "resource_tree_sha256",
            "resource_tree_total_bytes",
            "runtime_invocation",
            "canonical_sha256",
        ],
        "packaged Worker manifest",
    )?;
    let canonical = field_string(object, "canonical_sha256")?;
    if !is_sha256(canonical) || canonical_without_field(value, "canonical_sha256")? != canonical {
        return Err(KnifeWorkerError::Hash(
            "packaged Worker manifest canonical hash does not match".to_owned(),
        ));
    }
    if field_string(object, "schema_version")? != KNIFE_PACKAGED_MANIFEST_SCHEMA
        || field_string(object, "status")? != "DEVELOPMENT_STAGED_NOT_RELEASE_ELIGIBLE"
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker manifest identity is not allowlisted".to_owned(),
        ));
    }

    let blender = object_value(object, "blender", "packaged Worker Blender manifest")?;
    require_exact_fields(
        blender,
        &[
            "version",
            "display_version",
            "source_revision",
            "build_hash",
            "build_branch",
            "build_platform",
            "build_type",
            "bundle_id",
            "team_id",
            "signing_authority",
            "bundle_tree_sha256",
            "bundle_file_count",
            "bundle_total_bytes",
            "python_bundle_sha256",
            "license_resources",
            "download_artifact",
            "executable_path",
            "executable_sha256",
        ],
        "packaged Worker Blender manifest",
    )?;
    if field_string(blender, "version")? != KNIFE_BLENDER_VERSION
        || field_string(blender, "display_version")? != "5.2.1 LTS"
        || field_string(blender, "source_revision")? != KNIFE_BLENDER_REVISION
        || field_string(blender, "build_hash")? != KNIFE_BLENDER_REVISION
        || field_string(blender, "build_branch")? != "blender-v5.2-release"
        || field_string(blender, "build_platform")? != "Darwin"
        || field_string(blender, "build_type")? != "Release"
        || field_string(blender, "bundle_id")? != "org.blenderfoundation.blender"
        || field_string(blender, "team_id")? != "68UA947AUU"
        || field_string(blender, "signing_authority")?
            != "Developer ID Application: Stichting Blender Foundation (68UA947AUU)"
        || field_string(blender, "bundle_tree_sha256")?
            != "a1719f3e1c7fc846e811de3c9d32ff72f2130016a3290fa88eb4c8e9e1032317"
        || blender.get("bundle_file_count").and_then(Value::as_u64) != Some(6498)
        || blender.get("bundle_total_bytes").and_then(Value::as_u64) != Some(934431449)
        || field_string(blender, "python_bundle_sha256")?
            != "0fd0b83738e588928e8f6008138b8f812f66f9c837d984d6dd803495ce2dc7a0"
        || field_string(blender, "executable_path")? != KNIFE_PACKAGED_BLENDER_RELATIVE_PATH
        || field_string(blender, "executable_sha256")? != KNIFE_BLENDER_EXECUTABLE_SHA256
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker Blender identity is not allowlisted".to_owned(),
        ));
    }
    validate_license_resources(object_value(
        blender,
        "license_resources",
        "packaged Worker Blender license resources",
    )?)?;
    validate_download_artifact(object_value(
        blender,
        "download_artifact",
        "packaged Worker Blender download artifact",
    )?)?;

    let worker = object_value(object, "worker", "packaged Worker manifest worker")?;
    require_exact_fields(
        worker,
        &[
            "dependency_lock_sha256",
            "entrypoint_hash_policy",
            "entrypoint_path",
            "entrypoint_sha256",
            "source_manifest_path",
            "source_manifest_sha256",
            "worker_id",
            "worker_version",
            "protocol",
            "operation",
            "recipe_sha256",
        ],
        "packaged Worker manifest worker",
    )?;
    if field_string(worker, "entrypoint_path")? != KNIFE_PACKAGED_ENTRYPOINT_RELATIVE_PATH
        || field_string(worker, "entrypoint_hash_policy")? != "DERIVED_FROM_STAGED_ENTRYPOINT_BYTES"
        || field_string(worker, "source_manifest_path")?
            != KNIFE_PACKAGED_SOURCE_MANIFEST_RELATIVE_PATH
        || field_string(worker, "worker_id")? != KNIFE_WORKER_ID
        || field_string(worker, "worker_version")? != KNIFE_WORKER_VERSION
        || field_string(worker, "protocol")? != KNIFE_WORKER_PROTOCOL
        || field_string(worker, "operation")? != KNIFE_OPERATION
        || field_string(worker, "recipe_sha256")? != KNIFE_RECIPE_SHA256
        || field_string(worker, "dependency_lock_sha256")? != KNIFE_DEPENDENCY_LOCK_SHA256
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker operation identity is not allowlisted".to_owned(),
        ));
    }
    let entrypoint_sha256 = field_string(worker, "entrypoint_sha256")?.to_owned();
    let source_manifest_sha256 = field_string(worker, "source_manifest_sha256")?.to_owned();
    if !is_sha256(&entrypoint_sha256) || !is_sha256(&source_manifest_sha256) {
        return Err(KnifeWorkerError::Hash(
            "packaged Worker source hashes are invalid".to_owned(),
        ));
    }

    let policy = object_value(object, "policy", "packaged Worker manifest policy")?;
    require_exact_fields(
        policy,
        &[
            "network",
            "filesystem",
            "script",
            "blender_autoexec",
            "blender_factory_startup",
            "user_python_environment",
            "runtime_environment",
            "runtime_write_performed",
        ],
        "packaged Worker manifest policy",
    )?;
    if field_string(policy, "network")? != "disabled"
        || field_string(policy, "filesystem")? != "runtime_scratch_only"
        || field_string(policy, "script")? != "frozen_bundle_only"
        || field_string(policy, "blender_autoexec")? != "disabled"
        || field_string(policy, "blender_factory_startup")? != "required"
        || field_string(policy, "user_python_environment")? != "disabled"
        || field_string(policy, "runtime_environment")? != "cleared_by_runtime_launcher"
        || policy
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker policy is not allowlisted".to_owned(),
        ));
    }

    let distribution = object_value(
        object,
        "distribution_gates",
        "packaged Worker distribution gates",
    )?;
    require_exact_fields(
        distribution,
        &[
            "blender_bundle_signature",
            "blender_license",
            "gpl_source_offer",
            "legal_review",
            "notice",
            "product_distribution_signature",
            "release_blockers",
            "spdx_sbom",
            "release_eligible",
        ],
        "packaged Worker distribution gates",
    )?;
    if field_string(distribution, "blender_bundle_signature")? != "PASS_VERIFIED"
        || field_string(distribution, "blender_license")? != "PRESENT_BUNDLE_RESOURCE"
        || field_string(distribution, "gpl_source_offer")? != "NOT_INCLUDED"
        || field_string(distribution, "legal_review")? != "NOT_RUN"
        || field_string(distribution, "notice")? != "NOT_INCLUDED"
        || field_string(distribution, "product_distribution_signature")? != "NOT_RUN"
        || distribution.get("release_blockers")
            != Some(&serde_json::json!([
                "GPL_SOURCE_OFFER_NOT_INCLUDED",
                "NOTICE_NOT_INCLUDED",
                "SPDX_SBOM_NOT_INCLUDED",
                "LEGAL_REVIEW_NOT_RUN",
                "PRODUCT_DISTRIBUTION_SIGNATURE_NOT_RUN"
            ]))
        || field_string(distribution, "spdx_sbom")? != "NOT_INCLUDED"
        || distribution
            .get("release_eligible")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker distribution gates are not pending".to_owned(),
        ));
    }

    let provenance = object_value(object, "provenance", "packaged Worker provenance")?;
    require_exact_fields(
        provenance,
        &[
            "bundle_tree_sha256",
            "download_artifact_sha256",
            "download_artifact_status",
            "input",
        ],
        "packaged Worker provenance",
    )?;
    if field_string(provenance, "bundle_tree_sha256")?
        != "a1719f3e1c7fc846e811de3c9d32ff72f2130016a3290fa88eb4c8e9e1032317"
        || !provenance
            .get("download_artifact_sha256")
            .is_some_and(Value::is_null)
        || field_string(provenance, "download_artifact_status")? != "NOT_AVAILABLE_IN_WORKSPACE"
        || field_string(provenance, "input")? != "locally_installed_blender_app"
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker provenance is not allowlisted".to_owned(),
        ));
    }

    let invocation = object_value(object, "runtime_invocation", "packaged Worker invocation")?;
    require_exact_fields(
        invocation,
        &["arguments", "caller_controls", "environment"],
        "packaged Worker invocation",
    )?;
    if invocation.get("arguments")
        != Some(&serde_json::json!([
            "--background",
            "--factory-startup",
            "--disable-autoexec",
            "--threads",
            "1",
            "--debug-depsgraph-no-threads",
            "--python-exit-code",
            "1",
            "--python",
            "<sealed-worker-entrypoint>",
            "--",
            "--scratch-root",
            "<runtime-scratch>"
        ]))
        || invocation.get("caller_controls")
            != Some(&serde_json::json!({
                "addon": false,
                "environment": false,
                "network": false,
                "path": false,
                "python": false,
                "url": false
            }))
        || field_string(invocation, "environment")? != "cleared_by_runtime_launcher"
    {
        return Err(KnifeWorkerError::Invalid(
            "packaged Worker invocation policy is not allowlisted".to_owned(),
        ));
    }

    let manifest_resource_tree_sha256 = field_string(object, "resource_tree_sha256")?.to_owned();
    if !is_sha256(&manifest_resource_tree_sha256) {
        return Err(KnifeWorkerError::Hash(
            "packaged Worker resource tree hash is invalid".to_owned(),
        ));
    }
    let tree = resource_tree_summary(root, KNIFE_PACKAGED_MANIFEST_RELATIVE_PATH)?;
    if tree.sha256 != manifest_resource_tree_sha256
        || object
            .get("resource_tree_file_count")
            .and_then(Value::as_u64)
            != Some(tree.file_count)
        || object
            .get("resource_tree_total_bytes")
            .and_then(Value::as_u64)
            != Some(tree.total_bytes)
    {
        return Err(KnifeWorkerError::Hash(
            "packaged Worker resource tree differs from its manifest".to_owned(),
        ));
    }

    // The package manifest does not repeat the source manifest's dependency
    // lock, so the fixed lock identity is checked after the source manifest is
    // opened by the caller.
    Ok(PackagedManifestMetadata {
        blender_version: KNIFE_BLENDER_VERSION.to_owned(),
        blender_revision: KNIFE_BLENDER_REVISION.to_owned(),
        blender_executable_sha256: KNIFE_BLENDER_EXECUTABLE_SHA256.to_owned(),
        entrypoint_sha256,
        source_manifest_sha256,
        dependency_lock_sha256: KNIFE_DEPENDENCY_LOCK_SHA256.to_owned(),
        resource_tree_sha256: manifest_resource_tree_sha256,
    })
}

fn field_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, KnifeWorkerError> {
    object.get(field).and_then(Value::as_str).ok_or_else(|| {
        KnifeWorkerError::Protocol(format!("fixed Worker manifest field {field} is invalid"))
    })
}

fn object_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    label: &str,
) -> Result<&'a Map<String, Value>, KnifeWorkerError> {
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(|| KnifeWorkerError::Protocol(format!("{label} is not an object")))
}

fn validate_license_resources(object: &Map<String, Value>) -> Result<(), KnifeWorkerError> {
    require_exact_fields(
        object,
        &["copyright", "gpl_text", "license", "third_party_index"],
        "fixed Worker license resources",
    )?;
    let expected = [
        (
            "copyright",
            "Contents/Resources/text/copyright.txt",
            "770e6530a763692598a4d52431be0c4bda4499550b7c38cceda2053241453d73",
            1585_u64,
        ),
        (
            "gpl_text",
            "Contents/Resources/text/license/spdx/GPL-3.0-or-later.txt",
            "8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903",
            35147_u64,
        ),
        (
            "license",
            "Contents/Resources/text/license/license.md",
            "5cfbe909ac56d683c671faffabfcfbf6ba7d47f8fce0cff813d1002c46c13487",
            185192_u64,
        ),
        (
            "third_party_index",
            "Contents/Resources/text/license/licenses.json",
            "6d03630395465b615e7be22cc3700d30daab725929e7b761090c53145a01b6db",
            3662_u64,
        ),
    ];
    for (name, path, sha256, byte_size) in expected {
        let record = object_value(object, name, "fixed Worker license resource")?;
        require_exact_fields(
            record,
            &["path", "sha256", "byte_size"],
            "fixed Worker license resource",
        )?;
        if field_string(record, "path")? != path
            || field_string(record, "sha256")? != sha256
            || record.get("byte_size").and_then(Value::as_u64) != Some(byte_size)
        {
            return Err(KnifeWorkerError::Invalid(
                "fixed Worker license resource identity is not allowlisted".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_download_artifact(object: &Map<String, Value>) -> Result<(), KnifeWorkerError> {
    require_exact_fields(
        object,
        &["kind", "status", "sha256", "provenance"],
        "fixed Worker download artifact",
    )?;
    if field_string(object, "kind")? != "blender-official-macos-arm64-dmg"
        || field_string(object, "status")? != "NOT_AVAILABLE_IN_WORKSPACE"
        || !object.get("sha256").is_some_and(Value::is_null)
        || field_string(object, "provenance")? != "locally_installed_sidecar_only"
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker download artifact identity is not allowlisted".to_owned(),
        ));
    }
    Ok(())
}

fn validate_fixed_recipe(object: &Map<String, Value>) -> Result<(), KnifeWorkerError> {
    require_exact_fields(
        object,
        &[
            "high",
            "low",
            "bake",
            "renderer",
            "texture_size",
            "margin_texels",
            "cage_extrusion_m",
            "randomness",
        ],
        "fixed Worker recipe",
    )?;
    if object.get("high") != Some(&serde_json::json!(["bevel", "weighted_normal"]))
        || object.get("low")
            != Some(&serde_json::json!([
                "decimate",
                "weighted_normal",
                "smart_project_uv"
            ]))
        || object.get("bake") != Some(&serde_json::json!(["tangent_normal", "ambient_occlusion"]))
        || field_string(object, "renderer")? != "cycles-cpu"
        || object.get("texture_size").and_then(Value::as_u64) != Some(KNIFE_TEXTURE_SIZE as u64)
        || object.get("margin_texels").and_then(Value::as_u64) != Some(8)
        || object.get("cage_extrusion_m").and_then(Value::as_f64) != Some(0.02)
        || field_string(object, "randomness")? != "disabled"
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker recipe is not allowlisted".to_owned(),
        ));
    }
    Ok(())
}

fn validate_fixed_limits(object: &Map<String, Value>) -> Result<(), KnifeWorkerError> {
    require_exact_fields(
        object,
        &[
            "max_runtime_ms",
            "max_memory_bytes",
            "max_triangles",
            "max_objects",
            "max_texture_size",
        ],
        "fixed Worker limits",
    )?;
    if object.get("max_runtime_ms").and_then(Value::as_u64) != Some(KNIFE_MAX_RUNTIME_MS)
        || object.get("max_memory_bytes").and_then(Value::as_u64) != Some(KNIFE_MAX_MEMORY_BYTES)
        || object.get("max_triangles").and_then(Value::as_u64) != Some(KNIFE_MAX_TRIANGLES as u64)
        || object.get("max_objects").and_then(Value::as_u64) != Some(KNIFE_MAX_OBJECTS as u64)
        || object.get("max_texture_size").and_then(Value::as_u64) != Some(KNIFE_TEXTURE_SIZE as u64)
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker limits are not allowlisted".to_owned(),
        ));
    }
    Ok(())
}

fn validate_fixed_output_policy(object: &Map<String, Value>) -> Result<(), KnifeWorkerError> {
    require_exact_fields(
        object,
        &[
            "runtime_write_performed",
            "stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "durability",
            "readback_owner",
        ],
        "fixed Worker output policy",
    )?;
    if object
        .get("runtime_write_performed")
        .and_then(Value::as_bool)
        != Some(false)
        || object.get("stage_advanced").and_then(Value::as_bool) != Some(false)
        || object.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || object.get("version_created").and_then(Value::as_bool) != Some(false)
        || object.get("export_performed").and_then(Value::as_bool) != Some(false)
        || field_string(object, "durability")? != "pending_runtime_adoption"
        || field_string(object, "readback_owner")? != "forgecad-runtime"
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Worker output policy is not allowlisted".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ResourceTreeSummary {
    sha256: String,
    file_count: u64,
    total_bytes: u64,
}

fn resource_tree_summary(
    root: &Path,
    excluded: &str,
) -> Result<ResourceTreeSummary, KnifeWorkerError> {
    let mut files = Vec::new();
    collect_tree_entries(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut digest = Sha256::new();
    let mut file_count = 0_u64;
    let mut total_bytes = 0_u64;
    for (relative, path) in files {
        if relative == excluded {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(KnifeWorkerError::Io)?;
        let (kind, byte_size, payload) = if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).map_err(KnifeWorkerError::Io)?;
            let payload = target
                .as_os_str()
                .to_string_lossy()
                .into_owned()
                .into_bytes();
            (b"symlink".as_slice(), payload.len() as u64, Some(payload))
        } else if metadata.file_type().is_file() {
            (b"file".as_slice(), metadata.len(), None)
        } else {
            return Err(KnifeWorkerError::Invalid(
                "packaged Worker resource tree contains an unsupported entry".to_owned(),
            ));
        };
        file_count = file_count.saturating_add(1);
        total_bytes = total_bytes.saturating_add(byte_size);
        digest.update(relative.as_bytes());
        digest.update([0]);
        digest.update(kind);
        digest.update([0]);
        digest.update(byte_size.to_string().as_bytes());
        digest.update([0]);
        if let Some(payload) = payload {
            digest.update(payload);
        } else {
            let mut file = File::open(&path).map_err(KnifeWorkerError::Io)?;
            let mut buffer = [0_u8; 1024 * 1024];
            loop {
                let read = file.read(&mut buffer).map_err(KnifeWorkerError::Io)?;
                if read == 0 {
                    break;
                }
                digest.update(&buffer[..read]);
            }
        }
        digest.update([0]);
    }
    Ok(ResourceTreeSummary {
        sha256: digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        file_count,
        total_bytes,
    })
}

fn collect_tree_entries(
    root: &Path,
    current: &Path,
    entries: &mut Vec<(String, PathBuf)>,
) -> Result<(), KnifeWorkerError> {
    let mut children = fs::read_dir(current)
        .map_err(KnifeWorkerError::Io)?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(KnifeWorkerError::Io)?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .map_err(|_| KnifeWorkerError::Invalid("resource tree path escaped root".to_owned()))?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let metadata = fs::symlink_metadata(&path).map_err(KnifeWorkerError::Io)?;
        if metadata.file_type().is_dir() {
            collect_tree_entries(root, &path, entries)?;
        } else {
            entries.push((relative, path));
        }
    }
    Ok(())
}

fn collect_artifacts(
    response: &KnifeWorkerResponse,
    scratch: &Path,
    max_output_bytes: u64,
) -> Result<BTreeMap<String, Vec<u8>>, KnifeWorkerError> {
    let result = response.result.as_ref().expect("validated result");
    let outputs = result["outputs"].as_array().expect("validated outputs");
    let mut total = 0_u64;
    let mut artifacts = BTreeMap::new();
    for output in outputs {
        let relative = output["relative_path"].as_str().expect("validated path");
        let path = safe_join_output(scratch, relative)?;
        let bytes = read_limited_file(&path, KNIFE_MAX_OUTPUT_BYTES)?;
        let expected_size = output["byte_size"].as_u64().expect("validated size");
        let expected_hash = output["sha256"].as_str().expect("validated hash");
        if bytes.len() as u64 != expected_size || sha256_hex(&bytes) != expected_hash {
            return Err(KnifeWorkerError::Hash(
                "knife Worker output bytes differ from the result manifest".to_owned(),
            ));
        }
        total = total.saturating_add(bytes.len() as u64);
        if total > max_output_bytes || total > KNIFE_MAX_OUTPUT_BYTES as u64 {
            return Err(KnifeWorkerError::Resource(
                "knife Worker outputs exceed the requested byte budget".to_owned(),
            ));
        }
        artifacts.insert(relative.to_owned(), bytes);
    }
    Ok(artifacts)
}

fn request_canonical_sha256(request: &KnifeWorkerRequest) -> Result<String, KnifeWorkerError> {
    let mut value = serde_json::to_value(request).map_err(KnifeWorkerError::Json)?;
    value["canonical_sha256"] = Value::String(String::new());
    canonical_json_hash(&value).map_err(KnifeWorkerError::Invalid)
}

fn canonical_without_field(value: &Value, field: &str) -> Result<String, KnifeWorkerError> {
    let mut value = value.clone();
    value[field] = Value::String(String::new());
    canonical_json_hash(&value).map_err(KnifeWorkerError::Invalid)
}

pub fn canonical_json_hash(value: &Value) -> Result<String, String> {
    Ok(sha256_hex(&canonical_json_bytes(value)?))
}

pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    write_canonical(value, &mut bytes)?;
    Ok(bytes)
}

fn write_canonical(value: &Value, output: &mut Vec<u8>) -> Result<(), String> {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
        Value::String(value) => serde_json::to_writer(&mut *output, value)
            .map_err(|error| format!("canonical string serialization failed: {error}"))?,
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                write_canonical(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            output.push(b'{');
            for (index, key) in keys.iter().enumerate() {
                if index != 0 {
                    output.push(b',');
                }
                serde_json::to_writer(&mut *output, key)
                    .map_err(|error| format!("canonical key serialization failed: {error}"))?;
                output.push(b':');
                write_canonical(&values[*key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn safe_output_relative(value: &str) -> bool {
    value.starts_with("output/")
        && !Path::new(value).is_absolute()
        && !value.contains('\\')
        && !value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && value.len() <= 256
}

fn safe_join_input(root: &Path, relative: &str) -> Result<PathBuf, KnifeWorkerError> {
    if relative != KNIFE_INPUT_RELATIVE_PATH {
        return Err(KnifeWorkerError::Invalid(
            "Worker input path is not the fixed staged path".to_owned(),
        ));
    }
    Ok(root.join(relative))
}

fn ensure_fixed_directory(root: &Path, name: &str) -> Result<PathBuf, KnifeWorkerError> {
    let path = root.join(name);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(path),
        Ok(_) => Err(KnifeWorkerError::Invalid(format!(
            "fixed staged Worker directory {name} is not a real directory"
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir(&path).map_err(KnifeWorkerError::Io)?;
            Ok(path)
        }
        Err(error) => Err(KnifeWorkerError::Io(error)),
    }
}

fn ensure_output_directory(root: &Path) -> Result<PathBuf, KnifeWorkerError> {
    let output = ensure_fixed_directory(root, KNIFE_OUTPUT_DIRECTORY)?;
    let mut entries = fs::read_dir(&output).map_err(KnifeWorkerError::Io)?;
    if entries
        .next()
        .transpose()
        .map_err(KnifeWorkerError::Io)?
        .is_some()
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed staged Worker output directory must be empty".to_owned(),
        ));
    }
    Ok(output)
}

fn safe_join_output(root: &Path, relative: &str) -> Result<PathBuf, KnifeWorkerError> {
    if !safe_output_relative(relative) {
        return Err(KnifeWorkerError::Protocol(
            "Worker output path escaped the fixed output directory".to_owned(),
        ));
    }
    let output_root = root.join(KNIFE_OUTPUT_DIRECTORY);
    let output_root = fs::canonicalize(&output_root).map_err(KnifeWorkerError::Io)?;
    if !output_root.starts_with(root) {
        return Err(KnifeWorkerError::Protocol(
            "Worker output directory escaped the scratch root".to_owned(),
        ));
    }
    let path = root.join(relative);
    let parent = path
        .parent()
        .ok_or_else(|| KnifeWorkerError::Protocol("Worker output path has no parent".to_owned()))?;
    let canonical_parent = fs::canonicalize(parent).map_err(KnifeWorkerError::Io)?;
    if !canonical_parent.starts_with(&output_root) {
        return Err(KnifeWorkerError::Protocol(
            "Worker output path escaped the fixed output directory".to_owned(),
        ));
    }
    let metadata = fs::symlink_metadata(&path).map_err(KnifeWorkerError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(KnifeWorkerError::Protocol(
            "Worker output is not a regular file".to_owned(),
        ));
    }
    Ok(path)
}

fn join_fixed_relative(root: &Path, relative: &str) -> Result<PathBuf, KnifeWorkerError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || relative.contains("..")
        || relative.contains('\\')
        || relative.contains("://")
    {
        return Err(KnifeWorkerError::Invalid(
            "fixed Blender path is not a safe packaged relative path".to_owned(),
        ));
    }
    Ok(root.join(path))
}

fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_request(mut stdin: impl Write, bytes: &[u8]) -> io::Result<()> {
    stdin.write_all(bytes)?;
    stdin.flush()
}

fn read_limited(mut reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if output.len().saturating_add(read) > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Worker stream exceeded its fixed byte bound",
            ));
        }
        output.extend_from_slice(&buffer[..read]);
    }
    Ok(output)
}

fn read_limited_file(path: &Path, limit: usize) -> Result<Vec<u8>, KnifeWorkerError> {
    let file = File::open(path).map_err(KnifeWorkerError::Io)?;
    read_limited(file, limit).map_err(KnifeWorkerError::Io)
}

fn wait_bounded(
    child: &mut Child,
    timeout: Duration,
) -> Result<(ExitStatus, bool), KnifeWorkerError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(KnifeWorkerError::Io)? {
            return Ok((status, false));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().map_err(KnifeWorkerError::Io)?;
            return Ok((status, true));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn bounded_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(512).collect()
}

struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn create() -> Result<Self, KnifeWorkerError> {
        let root = std::env::temp_dir();
        for _ in 0..32 {
            let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = root.join(format!(
                "{SCRATCH_PREFIX}-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => return Ok(Self { path: candidate }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(KnifeWorkerError::Io(error)),
            }
        }
        Err(KnifeWorkerError::Resource(
            "could not allocate a unique Blender scratch directory".to_owned(),
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
pub enum KnifeWorkerError {
    Invalid(String),
    Protocol(String),
    Hash(String),
    Resource(String),
    Io(io::Error),
    Json(serde_json::Error),
    Timeout { stderr: String },
    WorkerRejected { code: String, message: String },
    WorkerExit { code: Option<i32>, stderr: String },
}

impl fmt::Display for KnifeWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => write!(formatter, "BLENDER_KNIFE_INVALID:{message}"),
            Self::Protocol(message) => write!(formatter, "BLENDER_KNIFE_PROTOCOL:{message}"),
            Self::Hash(message) => write!(formatter, "BLENDER_KNIFE_HASH_MISMATCH:{message}"),
            Self::Resource(message) => write!(formatter, "BLENDER_KNIFE_RESOURCE_LIMIT:{message}"),
            Self::Io(error) => write!(formatter, "BLENDER_KNIFE_IO:{error}"),
            Self::Json(error) => write!(formatter, "BLENDER_KNIFE_JSON:{error}"),
            Self::Timeout { stderr } => write!(formatter, "BLENDER_KNIFE_TIMEOUT:{stderr}"),
            Self::WorkerRejected { code, message } => {
                write!(formatter, "BLENDER_KNIFE_REJECTED:{code}:{message}")
            }
            Self::WorkerExit { code, stderr } => {
                write!(formatter, "BLENDER_KNIFE_EXIT:{code:?}:{stderr}")
            }
        }
    }
}

impl std::error::Error for KnifeWorkerError {}

impl From<serde_json::Error> for KnifeWorkerError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> KnifeWorkerRequest {
        let mut value = serde_json::json!({
            "schema_version": KNIFE_REQUEST_SCHEMA,
            "operation": KNIFE_OPERATION,
            "request_id": "dragonfang-r8",
            "project_id": "weaponry-dragonfang",
            "candidate_id": "dragonfang-kukri-r8",
            "input_glb": {
                "kind": "authoring_mesh_glb",
                "relative_path": KNIFE_INPUT_RELATIVE_PATH,
                "sha256": "a".repeat(64),
                "byte_size": 1,
                "mime": "model/gltf-binary"
            },
            "recipe_id": KNIFE_RECIPE_ID,
            "recipe_sha256": KNIFE_RECIPE_SHA256,
            "budgets": {
                "max_runtime_ms": KNIFE_MAX_RUNTIME_MS,
                "max_memory_bytes": KNIFE_MAX_MEMORY_BYTES,
                "max_input_bytes": KNIFE_MAX_INPUT_BYTES,
                "max_output_bytes": KNIFE_MAX_OUTPUT_BYTES,
                "max_triangles": KNIFE_MAX_TRIANGLES,
                "texture_size": KNIFE_TEXTURE_SIZE
            },
            "policies": {
                "network_policy": "disabled",
                "filesystem_policy": "runtime_scratch_only",
                "script_policy": "frozen_bundle_only",
                "output_policy": "temporary_observation_runtime_adoption"
            },
            "canonical_sha256": ""
        });
        value["canonical_sha256"] =
            Value::String(canonical_json_hash(&value).expect("canonical request hash"));
        serde_json::from_value(value).expect("typed request")
    }

    #[test]
    fn closed_knife_request_rejects_path_controls_and_accepts_fixed_recipe() {
        let request = request();
        validate_request(&request).expect("fixed request validates");
        let mut value = serde_json::to_value(&request).expect("request value");
        value["unexpected"] = Value::String("path".to_owned());
        assert!(serde_json::from_value::<KnifeWorkerRequest>(value).is_err());

        let mut changed = request;
        changed.recipe_id = "caller-script.py".to_owned();
        assert!(validate_request(&changed).is_err());
    }
}
