//! Atomic, Rust-owned Provider credential snapshots.
//!
//! A configured Provider is one immutable tuple: base URL, model and API key.
//! The API key is first staged under a random Keychain account; only after the
//! staged value is read back does an atomic metadata rename make that account
//! active. Readers resolve exactly the account named by the metadata snapshot,
//! so a failed or interrupted save can never combine old metadata with a new
//! key. The previous Keychain item is removed only after the commit.

use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::deepseek_provider::{
    DeepSeekCredentialSource, DeepSeekCredentialSourceError, DeepSeekCredentials,
};

pub const DEFAULT_PROVIDER_BASE_URL: &str = "https://api.deepseek.com";
pub const DEFAULT_PROVIDER_MODEL: &str = "deepseek-v4-pro";

const KEYCHAIN_SERVICE: &str = "ForgeCAD Agent Provider";
const LEGACY_KEYCHAIN_ACCOUNT: &str = "default";
const METADATA_MAX_BYTES: u64 = 64 * 1024;
const BASE_URL_MAX_BYTES: usize = 2_048;
const MODEL_MAX_BYTES: usize = 160;
const API_KEY_MAX_BYTES: usize = 4_096;

fn provider_status_not_checked() -> String {
    "not_checked".to_string()
}

fn provider_status_unavailable() -> String {
    "unavailable".to_string()
}

/// Metadata returned to the desktop UI and persisted without any secret.
///
/// `credential_id` is a random, non-secret generation identifier. Legacy
/// metadata does not have it and resolves the one pre-K002 Keychain account.
#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderConfigMetadata {
    pub base_url: String,
    pub model: String,
    pub configured: bool,
    pub storage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<String>,
    #[serde(default = "provider_status_not_checked")]
    pub metadata_status: String,
    #[serde(default = "provider_status_not_checked")]
    pub secret_status: String,
    #[serde(default = "provider_status_not_checked")]
    pub supervisor_status: String,
    #[serde(default = "provider_status_unavailable")]
    pub capability_status: String,
    #[serde(default)]
    pub failure_code: Option<String>,
}

impl fmt::Debug for ProviderConfigMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderConfigMetadata")
            .field("base_url", &"[REDACTED]")
            .field("model", &"[REDACTED]")
            .field("configured", &self.configured)
            .field("storage", &self.storage)
            .field(
                "credential_id",
                &self.credential_id.as_ref().map(|_| "[REDACTED]"),
            )
            .field("metadata_status", &self.metadata_status)
            .field("secret_status", &self.secret_status)
            .field("supervisor_status", &self.supervisor_status)
            .field("capability_status", &self.capability_status)
            .field("failure_code", &self.failure_code)
            .finish()
    }
}

impl ProviderConfigMetadata {
    fn disabled(storage: &str) -> Self {
        Self {
            base_url: DEFAULT_PROVIDER_BASE_URL.to_string(),
            model: DEFAULT_PROVIDER_MODEL.to_string(),
            configured: false,
            storage: storage.to_string(),
            credential_id: None,
            metadata_status: "valid".to_string(),
            secret_status: "missing".to_string(),
            supervisor_status: "not_checked".to_string(),
            capability_status: "offline".to_string(),
            failure_code: None,
        }
    }

    fn missing(storage: &str) -> Self {
        let mut metadata = Self::disabled(storage);
        metadata.metadata_status = "missing".to_string();
        metadata.failure_code = Some("PROVIDER_METADATA_MISSING".to_string());
        metadata
    }

    fn invalid(storage: &str) -> Self {
        let mut metadata = Self::disabled(storage);
        metadata.metadata_status = "invalid".to_string();
        metadata.failure_code = Some("PROVIDER_METADATA_INVALID".to_string());
        metadata
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ProviderStoreError;

impl fmt::Debug for ProviderStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderStoreError([REDACTED])")
    }
}

impl fmt::Display for ProviderStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("The Provider credential snapshot could not be accessed.")
    }
}

impl Error for ProviderStoreError {}

trait ProviderSecretBackend: Send + Sync + 'static {
    fn read(&self, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, ProviderStoreError>;
    fn write(&self, account: &str, secret: &[u8]) -> Result<(), ProviderStoreError>;
    fn delete(&self, account: &str) -> Result<(), ProviderStoreError>;
}

#[cfg(target_os = "macos")]
#[derive(Debug, Default)]
struct MacOsKeychainBackend;

#[cfg(target_os = "macos")]
impl ProviderSecretBackend for MacOsKeychainBackend {
    fn read(&self, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, ProviderStoreError> {
        use security_framework::passwords::get_generic_password;
        use security_framework_sys::base::errSecItemNotFound;

        match get_generic_password(KEYCHAIN_SERVICE, account) {
            Ok(secret) => Ok(Some(Zeroizing::new(secret))),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(_) => Err(ProviderStoreError),
        }
    }

    fn write(&self, account: &str, secret: &[u8]) -> Result<(), ProviderStoreError> {
        security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, account, secret)
            .map_err(|_| ProviderStoreError)
    }

    fn delete(&self, account: &str) -> Result<(), ProviderStoreError> {
        use security_framework_sys::base::errSecItemNotFound;

        match security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(ProviderStoreError),
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default)]
struct UnsupportedSecretBackend;

#[cfg(not(target_os = "macos"))]
impl ProviderSecretBackend for UnsupportedSecretBackend {
    fn read(&self, _account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, ProviderStoreError> {
        Err(ProviderStoreError)
    }

    fn write(&self, _account: &str, _secret: &[u8]) -> Result<(), ProviderStoreError> {
        Err(ProviderStoreError)
    }

    fn delete(&self, _account: &str) -> Result<(), ProviderStoreError> {
        Err(ProviderStoreError)
    }
}

struct ProviderCredentialSnapshot {
    base_url: String,
    model: String,
    api_key: Zeroizing<String>,
}

/// Single synchronization and commit boundary used by Tauri commands and by
/// every dynamic Provider request.
pub struct ProviderCredentialStore {
    metadata_path: PathBuf,
    storage_name: &'static str,
    secrets: Arc<dyn ProviderSecretBackend>,
    transaction: Mutex<()>,
    #[cfg(test)]
    fail_next_metadata_commit: std::sync::atomic::AtomicBool,
}

impl fmt::Debug for ProviderCredentialStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderCredentialStore")
            .field("metadata_path", &"[REDACTED]")
            .field("storage_name", &self.storage_name)
            .finish_non_exhaustive()
    }
}

impl ProviderCredentialStore {
    pub fn production() -> Arc<Self> {
        #[cfg(target_os = "macos")]
        let secrets: Arc<dyn ProviderSecretBackend> = Arc::new(MacOsKeychainBackend);
        #[cfg(not(target_os = "macos"))]
        let secrets: Arc<dyn ProviderSecretBackend> = Arc::new(UnsupportedSecretBackend);

        Arc::new(Self {
            metadata_path: provider_metadata_path(),
            storage_name: provider_storage_name(),
            secrets,
            transaction: Mutex::new(()),
            #[cfg(test)]
            fail_next_metadata_commit: std::sync::atomic::AtomicBool::new(false),
        })
    }

    #[cfg(test)]
    fn for_test(metadata_path: PathBuf, secrets: Arc<dyn ProviderSecretBackend>) -> Arc<Self> {
        Arc::new(Self {
            metadata_path,
            storage_name: "test-keychain",
            secrets,
            transaction: Mutex::new(()),
            fail_next_metadata_commit: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn inspect(&self) -> ProviderConfigMetadata {
        let Ok(_guard) = self.lock() else {
            return ProviderConfigMetadata::invalid(self.storage_name);
        };
        self.inspect_locked()
    }

    /// Reads only the non-secret metadata file. Normal workbench startup uses
    /// this path so opening ForgeCAD never triggers a macOS Keychain password
    /// prompt. The secret is resolved only for an explicit connection test or
    /// Provider Turn through `load_snapshot_locked`.
    pub fn inspect_metadata_only(&self) -> ProviderConfigMetadata {
        let Ok(_guard) = self.lock() else {
            return ProviderConfigMetadata::invalid(self.storage_name);
        };
        self.inspect_metadata_locked()
    }

    pub fn save(
        &self,
        base_url: String,
        model: String,
        api_key: Zeroizing<String>,
    ) -> Result<ProviderConfigMetadata, String> {
        validate_provider_endpoint_model(&base_url, &model)?;
        validate_api_key(&api_key)?;
        let _guard = self
            .lock()
            .map_err(|_| "Provider credential transaction is unavailable.".to_string())?;

        let previous_account = self.current_account_locked();
        let credential_id = generate_credential_id()
            .map_err(|_| "Provider credential generation failed.".to_string())?;
        let staged_account = account_for_credential_id(&credential_id)
            .map_err(|_| "Provider credential generation failed.".to_string())?;

        self.secrets
            .write(&staged_account, api_key.as_bytes())
            .map_err(|_| "Unable to write the macOS Keychain.".to_string())?;

        let staged_secret = match self.secrets.read(&staged_account) {
            Ok(secret) => secret,
            Err(_) => {
                let _ = self.secrets.delete(&staged_account);
                return Err("Unable to verify the macOS Keychain entry.".to_string());
            }
        };
        let staged_matches = staged_secret
            .as_deref()
            .map(|value| value == api_key.as_bytes())
            .unwrap_or(false);
        if !staged_matches {
            let _ = self.secrets.delete(&staged_account);
            return Err("Unable to verify the macOS Keychain entry.".to_string());
        }

        let metadata = ProviderConfigMetadata {
            base_url,
            model,
            configured: true,
            storage: self.storage_name.to_string(),
            credential_id: Some(credential_id),
            metadata_status: "valid".to_string(),
            secret_status: "available".to_string(),
            supervisor_status: "not_checked".to_string(),
            capability_status: "unavailable".to_string(),
            failure_code: None,
        };
        if self.write_metadata_atomic(&metadata).is_err() {
            let _ = self.secrets.delete(&staged_account);
            return Err("Provider metadata could not be committed.".to_string());
        }

        if let Some(previous_account) = previous_account {
            if previous_account != staged_account {
                // Cleanup is deliberately after the commit and best effort.
                // Its failure leaves an unreachable old key, never a mixed or
                // unusable active tuple.
                let _ = self.secrets.delete(&previous_account);
            }
        }
        Ok(metadata)
    }

    pub fn clear(&self) -> Result<ProviderConfigMetadata, String> {
        let _guard = self
            .lock()
            .map_err(|_| "Provider credential transaction is unavailable.".to_string())?;
        let previous_account = self.current_account_locked();
        let metadata = ProviderConfigMetadata::disabled(self.storage_name);
        self.write_metadata_atomic(&metadata)
            .map_err(|_| "Provider metadata could not be committed.".to_string())?;
        if let Some(previous_account) = previous_account {
            let _ = self.secrets.delete(&previous_account);
        }
        Ok(metadata)
    }

    fn lock(&self) -> Result<MutexGuard<'_, ()>, ProviderStoreError> {
        self.transaction.lock().map_err(|_| ProviderStoreError)
    }

    fn inspect_locked(&self) -> ProviderConfigMetadata {
        let mut metadata = self.inspect_metadata_locked();
        if metadata.metadata_status != "valid" {
            return metadata;
        }
        if !metadata.configured {
            return metadata;
        }
        let Ok(account) = account_for_metadata(&metadata) else {
            return ProviderConfigMetadata::invalid(self.storage_name);
        };
        match self.secrets.read(&account) {
            Ok(Some(secret)) if valid_api_key_bytes(&secret) => {
                metadata.secret_status = "available".to_string();
            }
            Ok(_) => {
                metadata.configured = false;
                metadata.secret_status = "missing".to_string();
                metadata.failure_code = Some("PROVIDER_SECRET_MISSING".to_string());
            }
            Err(_) => {
                metadata.configured = false;
                metadata.secret_status = "unavailable".to_string();
                metadata.failure_code = Some("PROVIDER_SECRET_UNAVAILABLE".to_string());
            }
        }
        metadata
    }

    fn inspect_metadata_locked(&self) -> ProviderConfigMetadata {
        if !self.metadata_path.is_file() {
            return ProviderConfigMetadata::missing(self.storage_name);
        }
        let Ok(mut metadata) = self.read_metadata_locked() else {
            return ProviderConfigMetadata::invalid(self.storage_name);
        };
        if validate_stored_metadata(&metadata, self.storage_name).is_err() {
            return ProviderConfigMetadata::invalid(self.storage_name);
        }
        metadata.metadata_status = "valid".to_string();
        metadata.secret_status = if metadata.configured {
            "not_checked".to_string()
        } else {
            "missing".to_string()
        };
        metadata.supervisor_status = "not_checked".to_string();
        metadata.capability_status = "unavailable".to_string();
        metadata.failure_code = None;
        metadata
    }

    fn load_snapshot_locked(
        &self,
    ) -> Result<Option<ProviderCredentialSnapshot>, ProviderStoreError> {
        if !self.metadata_path.is_file() {
            return Ok(None);
        }
        let metadata = self.read_metadata_locked()?;
        validate_stored_metadata(&metadata, self.storage_name)?;
        if !metadata.configured {
            return Ok(None);
        }
        let account = account_for_metadata(&metadata)?;
        let Some(secret_bytes) = self.secrets.read(&account)? else {
            return Ok(None);
        };
        if !valid_api_key_bytes(&secret_bytes) {
            return Ok(None);
        }
        let secret_text = std::str::from_utf8(&secret_bytes).map_err(|_| ProviderStoreError)?;
        let api_key = Zeroizing::new(secret_text.to_string());
        Ok(Some(ProviderCredentialSnapshot {
            base_url: metadata.base_url,
            model: metadata.model,
            api_key,
        }))
    }

    fn current_account_locked(&self) -> Option<String> {
        let metadata = self.read_metadata_locked().ok()?;
        validate_stored_metadata(&metadata, self.storage_name).ok()?;
        if !metadata.configured {
            return None;
        }
        account_for_metadata(&metadata).ok()
    }

    fn read_metadata_locked(&self) -> Result<ProviderConfigMetadata, ProviderStoreError> {
        let file_metadata = fs::metadata(&self.metadata_path).map_err(|_| ProviderStoreError)?;
        if !file_metadata.is_file() || file_metadata.len() > METADATA_MAX_BYTES {
            return Err(ProviderStoreError);
        }
        let payload = fs::read(&self.metadata_path).map_err(|_| ProviderStoreError)?;
        serde_json::from_slice(&payload).map_err(|_| ProviderStoreError)
    }

    fn write_metadata_atomic(
        &self,
        metadata: &ProviderConfigMetadata,
    ) -> Result<(), ProviderStoreError> {
        #[cfg(test)]
        if self
            .fail_next_metadata_commit
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(ProviderStoreError);
        }

        let parent = self.metadata_path.parent().ok_or(ProviderStoreError)?;
        fs::create_dir_all(parent).map_err(|_| ProviderStoreError)?;
        let temp_id = generate_credential_id()?;
        let file_name = self
            .metadata_path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or(ProviderStoreError)?;
        let temporary_path = parent.join(format!(".{file_name}.{temp_id}.tmp"));
        let payload =
            Zeroizing::new(serde_json::to_vec_pretty(metadata).map_err(|_| ProviderStoreError)?);

        #[cfg(unix)]
        let options = {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true).mode(0o600);
            options
        };
        #[cfg(not(unix))]
        let options = {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            options
        };

        let result = (|| {
            let mut file = options
                .open(&temporary_path)
                .map_err(|_| ProviderStoreError)?;
            file.write_all(&payload).map_err(|_| ProviderStoreError)?;
            file.sync_all().map_err(|_| ProviderStoreError)?;
            fs::rename(&temporary_path, &self.metadata_path).map_err(|_| ProviderStoreError)?;
            if let Ok(directory) = OpenOptions::new().read(true).open(parent) {
                let _ = directory.sync_all();
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary_path);
        }
        result
    }

    #[cfg(test)]
    fn fail_next_metadata_commit(&self) {
        self.fail_next_metadata_commit
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl DeepSeekCredentialSource for ProviderCredentialStore {
    fn load(&self) -> Result<Option<DeepSeekCredentials>, DeepSeekCredentialSourceError> {
        let _guard = self.lock().map_err(|_| DeepSeekCredentialSourceError)?;
        let snapshot = self
            .load_snapshot_locked()
            .map_err(|_| DeepSeekCredentialSourceError)?;
        Ok(snapshot.map(|snapshot| {
            DeepSeekCredentials::from_zeroizing(snapshot.base_url, snapshot.model, snapshot.api_key)
        }))
    }
}

pub fn validate_provider_config_input(
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(String, String, Zeroizing<String>), String> {
    let base_url = base_url.trim().trim_end_matches('/').to_string();
    let model = model.trim().to_string();
    let api_key = Zeroizing::new(api_key.trim().to_string());
    validate_provider_endpoint_model(&base_url, &model)?;
    validate_api_key(&api_key)?;
    Ok((base_url, model, api_key))
}

fn validate_provider_endpoint_model(base_url: &str, model: &str) -> Result<(), String> {
    if base_url.is_empty() || base_url.len() > BASE_URL_MAX_BYTES {
        return Err("API Base URL 必须是有效的生产 HTTPS 地址。".to_string());
    }
    let parsed = reqwest::Url::parse(base_url)
        .map_err(|_| "API Base URL 必须是有效的生产 HTTPS 地址。".to_string())?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("API Base URL 必须是有效的生产 HTTPS 地址。".to_string());
    }
    if model.is_empty()
        || model.len() > MODEL_MAX_BYTES
        || model
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err("Model 不能为空且不能超过 160 个字符。".to_string());
    }
    Ok(())
}

fn validate_api_key(api_key: &str) -> Result<(), String> {
    if !valid_api_key_bytes(api_key.as_bytes()) {
        return Err("API Key 不能为空。".to_string());
    }
    Ok(())
}

fn valid_api_key_bytes(api_key: &[u8]) -> bool {
    !api_key.is_empty()
        && api_key.len() <= API_KEY_MAX_BYTES
        && api_key.iter().all(|byte| byte.is_ascii_graphic())
}

fn validate_stored_metadata(
    metadata: &ProviderConfigMetadata,
    expected_storage: &str,
) -> Result<(), ProviderStoreError> {
    validate_provider_endpoint_model(&metadata.base_url, &metadata.model)
        .map_err(|_| ProviderStoreError)?;
    let accepted_legacy_storage = cfg!(target_os = "macos") && metadata.storage == "macos-keychain";
    if metadata.storage != expected_storage && !accepted_legacy_storage {
        return Err(ProviderStoreError);
    }
    match (&metadata.credential_id, metadata.configured) {
        (Some(credential_id), _) if !valid_credential_id(credential_id) => Err(ProviderStoreError),
        (Some(_), false) => Err(ProviderStoreError),
        _ => Ok(()),
    }
}

fn account_for_metadata(metadata: &ProviderConfigMetadata) -> Result<String, ProviderStoreError> {
    match metadata.credential_id.as_deref() {
        Some(credential_id) => account_for_credential_id(credential_id),
        None => Ok(LEGACY_KEYCHAIN_ACCOUNT.to_string()),
    }
}

fn account_for_credential_id(credential_id: &str) -> Result<String, ProviderStoreError> {
    if !valid_credential_id(credential_id) {
        return Err(ProviderStoreError);
    }
    Ok(format!("{LEGACY_KEYCHAIN_ACCOUNT}:{credential_id}"))
}

fn valid_credential_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn generate_credential_id() -> Result<String, ProviderStoreError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ProviderStoreError)?;
    let mut output = String::with_capacity(32);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(output)
}

pub fn provider_storage_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos-keychain"
    } else {
        "secret-file-required"
    }
}

fn provider_metadata_path() -> PathBuf {
    let base = if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library").join("Application Support"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|path| PathBuf::from(path).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("ForgeCAD")
        .join("provider.json")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Barrier, Mutex,
        },
        thread,
    };

    use zeroize::Zeroizing;

    use super::*;

    #[derive(Default)]
    struct FakeSecretBackend {
        values: Mutex<BTreeMap<String, Vec<u8>>>,
        fail_write: AtomicBool,
        read_count: AtomicUsize,
    }

    impl FakeSecretBackend {
        fn fail_next_write(&self) {
            self.fail_write.store(true, Ordering::SeqCst);
        }

        fn contains(&self, value: &[u8]) -> bool {
            self.values
                .lock()
                .unwrap()
                .values()
                .any(|candidate| candidate == value)
        }

        fn read_count(&self) -> usize {
            self.read_count.load(Ordering::SeqCst)
        }
    }

    impl ProviderSecretBackend for FakeSecretBackend {
        fn read(&self, account: &str) -> Result<Option<Zeroizing<Vec<u8>>>, ProviderStoreError> {
            self.read_count.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .values
                .lock()
                .map_err(|_| ProviderStoreError)?
                .get(account)
                .cloned()
                .map(Zeroizing::new))
        }

        fn write(&self, account: &str, secret: &[u8]) -> Result<(), ProviderStoreError> {
            if self.fail_write.swap(false, Ordering::SeqCst) {
                return Err(ProviderStoreError);
            }
            self.values
                .lock()
                .map_err(|_| ProviderStoreError)?
                .insert(account.to_string(), secret.to_vec());
            Ok(())
        }

        fn delete(&self, account: &str) -> Result<(), ProviderStoreError> {
            self.values
                .lock()
                .map_err(|_| ProviderStoreError)?
                .remove(account);
            Ok(())
        }
    }

    struct TestStore {
        root: PathBuf,
        store: Arc<ProviderCredentialStore>,
        secrets: Arc<FakeSecretBackend>,
    }

    impl TestStore {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "forgecad-provider-credentials-{}",
                generate_credential_id().unwrap()
            ));
            fs::create_dir_all(&root).unwrap();
            let secrets = Arc::new(FakeSecretBackend::default());
            let store =
                ProviderCredentialStore::for_test(root.join("provider.json"), secrets.clone());
            Self {
                root,
                store,
                secrets,
            }
        }

        fn snapshot(&self) -> Option<(String, String, String)> {
            let _guard = self.store.lock().unwrap();
            self.store.load_snapshot_locked().unwrap().map(|snapshot| {
                (
                    snapshot.base_url,
                    snapshot.model,
                    snapshot.api_key.to_string(),
                )
            })
        }
    }

    impl Drop for TestStore {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn save_tuple(store: &ProviderCredentialStore, suffix: &str) {
        store
            .save(
                format!("https://{suffix}.example.test"),
                format!("model-{suffix}"),
                Zeroizing::new(format!("key-{suffix}")),
            )
            .unwrap();
    }

    #[test]
    fn metadata_only_inspection_never_reads_secret_backend() {
        let test = TestStore::new();
        save_tuple(&test.store, "startup");
        let reads_after_explicit_save = test.secrets.read_count();

        let metadata = test.store.inspect_metadata_only();

        assert_eq!(test.secrets.read_count(), reads_after_explicit_save);
        assert!(metadata.configured);
        assert_eq!(metadata.metadata_status, "valid");
        assert_eq!(metadata.secret_status, "not_checked");
        assert_eq!(metadata.supervisor_status, "not_checked");
        assert_eq!(metadata.capability_status, "unavailable");

        let inspected = test.store.inspect();
        assert_eq!(test.secrets.read_count(), reads_after_explicit_save + 1);
        assert_eq!(inspected.secret_status, "available");
    }

    #[test]
    fn save_and_dynamic_load_use_one_generation_tuple() {
        let test = TestStore::new();
        save_tuple(&test.store, "one");
        assert_eq!(
            test.snapshot(),
            Some((
                "https://one.example.test".to_string(),
                "model-one".to_string(),
                "key-one".to_string()
            ))
        );
        save_tuple(&test.store, "two");
        assert_eq!(
            test.snapshot(),
            Some((
                "https://two.example.test".to_string(),
                "model-two".to_string(),
                "key-two".to_string()
            ))
        );
        assert!(!test.secrets.contains(b"key-one"));
        assert!(test.secrets.contains(b"key-two"));
    }

    #[test]
    fn failed_secret_or_metadata_commit_preserves_old_complete_tuple() {
        let test = TestStore::new();
        save_tuple(&test.store, "old");

        test.secrets.fail_next_write();
        assert!(test
            .store
            .save(
                "https://new.example.test".to_string(),
                "model-new".to_string(),
                Zeroizing::new("key-new".to_string()),
            )
            .is_err());
        assert_eq!(
            test.snapshot(),
            Some((
                "https://old.example.test".to_string(),
                "model-old".to_string(),
                "key-old".to_string()
            ))
        );

        test.store.fail_next_metadata_commit();
        assert!(test
            .store
            .save(
                "https://new.example.test".to_string(),
                "model-new".to_string(),
                Zeroizing::new("key-new".to_string()),
            )
            .is_err());
        assert_eq!(
            test.snapshot(),
            Some((
                "https://old.example.test".to_string(),
                "model-old".to_string(),
                "key-old".to_string()
            ))
        );
        assert!(!test.secrets.contains(b"key-new"));
    }

    #[test]
    fn failed_clear_commit_retains_old_tuple_and_successful_clear_removes_it() {
        let test = TestStore::new();
        save_tuple(&test.store, "old");
        test.store.fail_next_metadata_commit();
        assert!(test.store.clear().is_err());
        assert_eq!(test.snapshot().unwrap().2, "key-old");
        test.store.clear().unwrap();
        assert_eq!(test.snapshot(), None);
        assert!(!test.secrets.contains(b"key-old"));
    }

    #[test]
    fn concurrent_saves_and_reads_never_mix_endpoint_model_and_key() {
        let test = TestStore::new();
        save_tuple(&test.store, "zero");
        let store = test.store.clone();
        let barrier = Arc::new(Barrier::new(3));
        let writer_barrier = barrier.clone();
        let writer_store = store.clone();
        let writer = thread::spawn(move || {
            writer_barrier.wait();
            for index in 1..=30 {
                save_tuple(&writer_store, &format!("generation-{index}"));
            }
        });
        let reader_barrier = barrier.clone();
        let reader_store = store.clone();
        let reader = thread::spawn(move || {
            reader_barrier.wait();
            for _ in 0..300 {
                let _guard = reader_store.lock().unwrap();
                let snapshot = reader_store.load_snapshot_locked().unwrap().unwrap();
                let host = reqwest::Url::parse(&snapshot.base_url)
                    .unwrap()
                    .host_str()
                    .unwrap()
                    .trim_end_matches(".example.test")
                    .to_string();
                assert_eq!(snapshot.model, format!("model-{host}"));
                assert_eq!(snapshot.api_key.as_str(), format!("key-{host}"));
            }
        });
        barrier.wait();
        writer.join().unwrap();
        reader.join().unwrap();
    }

    #[test]
    fn legacy_metadata_is_revalidated_before_becoming_ready() {
        let test = TestStore::new();
        let invalid = ProviderConfigMetadata {
            base_url: "http://api.deepseek.com".to_string(),
            model: "deepseek model".to_string(),
            configured: true,
            storage: "test-keychain".to_string(),
            credential_id: None,
            metadata_status: "not_checked".to_string(),
            secret_status: "not_checked".to_string(),
            supervisor_status: "not_checked".to_string(),
            capability_status: "unavailable".to_string(),
            failure_code: None,
        };
        fs::write(
            &test.store.metadata_path,
            serde_json::to_vec(&invalid).unwrap(),
        )
        .unwrap();
        test.secrets
            .write(LEGACY_KEYCHAIN_ACCOUNT, b"legacy-key")
            .unwrap();
        let inspected = test.store.inspect();
        assert!(!inspected.configured);
        assert_eq!(inspected.metadata_status, "invalid");
        assert_eq!(
            inspected.failure_code.as_deref(),
            Some("PROVIDER_METADATA_INVALID")
        );
        let _guard = test.store.lock().unwrap();
        assert!(test.store.load_snapshot_locked().is_err());
    }

    #[test]
    fn valid_legacy_snapshot_loads_as_one_tuple_and_migrates_on_save() {
        let test = TestStore::new();
        let legacy = ProviderConfigMetadata {
            base_url: "https://legacy.example.test".to_string(),
            model: "model-legacy".to_string(),
            configured: true,
            storage: "test-keychain".to_string(),
            credential_id: None,
            metadata_status: "not_checked".to_string(),
            secret_status: "not_checked".to_string(),
            supervisor_status: "not_checked".to_string(),
            capability_status: "unavailable".to_string(),
            failure_code: None,
        };
        fs::write(
            &test.store.metadata_path,
            serde_json::to_vec(&legacy).unwrap(),
        )
        .unwrap();
        test.secrets
            .write(LEGACY_KEYCHAIN_ACCOUNT, b"key-legacy")
            .unwrap();
        assert_eq!(
            test.snapshot(),
            Some((
                "https://legacy.example.test".to_string(),
                "model-legacy".to_string(),
                "key-legacy".to_string()
            ))
        );

        save_tuple(&test.store, "current");
        assert!(!test.secrets.contains(b"key-legacy"));
        assert_eq!(test.snapshot().unwrap().2, "key-current");
    }

    #[test]
    fn production_source_contains_no_security_cli_or_secret_argv_path() {
        let module_source = include_str!("provider_credentials.rs");
        let main_source = include_str!("main.rs");
        let forbidden_cli = ["Command::new(\"", "/usr/bin", "/security", "\")"].concat();
        assert!(!module_source.contains(&forbidden_cli));
        assert!(!main_source.contains(&forbidden_cli));
        assert!(module_source.contains("security_framework::passwords::set_generic_password"));
    }

    #[test]
    fn metadata_debug_redacts_endpoint_model_and_generation_identifier() {
        let metadata = ProviderConfigMetadata {
            base_url: "https://private-provider.example.test/tenant".to_string(),
            model: "private-model-name".to_string(),
            configured: true,
            storage: "test-keychain".to_string(),
            credential_id: Some("0123456789abcdef0123456789abcdef".to_string()),
            metadata_status: "valid".to_string(),
            secret_status: "available".to_string(),
            supervisor_status: "running".to_string(),
            capability_status: "ready".to_string(),
            failure_code: None,
        };

        let debug = format!("{metadata:?}");
        assert!(!debug.contains("private-provider"));
        assert!(!debug.contains("private-model-name"));
        assert!(!debug.contains("0123456789abcdef0123456789abcdef"));
        assert!(debug.contains("[REDACTED]"));

        // Debug redaction must not change the explicit UI/persistence DTO.
        let serialized = serde_json::to_value(&metadata).unwrap();
        assert_eq!(
            serialized["base_url"],
            "https://private-provider.example.test/tenant"
        );
        assert_eq!(serialized["model"], "private-model-name");
    }
}
