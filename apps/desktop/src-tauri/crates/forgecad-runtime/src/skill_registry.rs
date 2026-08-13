use forgecad_contracts::{SkillBundleManifestRecord, SkillExecutionAvailability};
use forgecad_core::{canonical_json_hash, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::OnceLock;

const REGISTRY_JSON: &str =
    include_str!("../../../../../../packages/forgecad-skills/registry.json");
const REGISTRY_TRUST: &str =
    include_str!("../../../../../../packages/forgecad-skills/trust/manifest.sha256");
const BUNDLE_ARCHIVE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/forgecad_skill_bundles.bin"));

const ARCHIVE_MAGIC: &[u8; 8] = b"FCBNDL01";
const MAX_ARCHIVE_FILES: usize = 512;
const MAX_ARCHIVE_BYTES: usize = 2 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: usize = 256 * 1024;
const FORBIDDEN_BUNDLE_SUFFIXES: &[&str] = &[
    ".py", ".js", ".ts", ".sh", ".wasm", ".dylib", ".so", ".dll", ".exe",
];

const REQUIRED_BUNDLE_ARTIFACTS: &[&str] = &[
    "manifest.json",
    "skill.yaml",
    "operators.lock",
    "validators/validator-set.json",
    "assets/index.json",
    "materials/index.json",
    "benchmarks/suite.yaml",
    "benchmarks/fixtures/valid.json",
    "benchmarks/fixtures/invalid-cycle-unit-finite.json",
    "benchmark-receipt.json",
    "LICENSES/ForgeCAD-FIRST-PARTY.txt",
    "NOTICE",
    "sbom.spdx.json",
    "provenance.intoto.jsonl",
    "trust/manifest.sha256",
    "signature.bundle",
    "recipes/default.recipe.json",
    "knowledge/overview.md",
    "knowledge/constraints.md",
    "knowledge/examples.md",
];

#[derive(Debug, Clone)]
struct BundleArchive {
    files: BTreeMap<String, Vec<u8>>,
}

static EMBEDDED_BUNDLE_ARCHIVE: OnceLock<Result<BundleArchive, String>> = OnceLock::new();

// This is intentionally a catalog of semantic executors, not a list of MCP
// tool names or a trust declaration from a Skill Bundle.  A lock entry joins
// this list only when the product-owned Worker/Runtime accepts and executes
// that exact typed operator contract.  MCP010D adds bounded hard-surface
// operators; legacy spellings remain omitted because a compatibility parser
// is not evidence that a real graph operator exists.
const EXECUTABLE_OPERATOR_IDS: &[&str] = &[
    "forgecad.geometry.primitive@1",
    "forgecad.geometry.primitive@2",
    "forgecad.geometry.profile-extrude@1",
    "forgecad.geometry.profile-loft@1",
    "forgecad.geometry.revolve@1",
    "forgecad.geometry.tube-sweep@1",
    "forgecad.geometry.transform@2",
    "forgecad.geometry.mirror@1",
    "forgecad.geometry.array@1",
    "forgecad.geometry.panel@1",
    "forgecad.geometry.vent-array@1",
    "forgecad.geometry.joint-stack@1",
    "forgecad.geometry.part-output@1",
    "forgecad.appearance.offline-pbr@1",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryDocument {
    schema_version: String,
    publisher: String,
    status: String,
    skills: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryEntry {
    skill_id: String,
    version: String,
    input_schema: String,
    output_schema: String,
    recipe: String,
    operator_ids: Vec<String>,
    validator_ids: Vec<String>,
    capabilities: Value,
    budgets: Value,
    benchmark_suite: String,
}

pub fn list() -> Result<Vec<SkillBundleManifestRecord>, String> {
    let document: RegistryDocument = serde_json::from_str(REGISTRY_JSON)
        .map_err(|error| format!("skill registry JSON is invalid: {error}"))?;
    validate_registry_trust()?;
    let archive = bundle_archive()?;
    if document.schema_version != "ForgeCADSkillRegistry@1"
        || document.publisher != "forgecad-first-party"
        || document.status != "development-only"
    {
        return Err("SKILL_UNTRUSTED: registry header is invalid".to_owned());
    }
    let mut seen = HashSet::new();
    let mut manifests = Vec::with_capacity(document.skills.len());
    for entry in document.skills {
        validate_entry(&entry)?;
        if !seen.insert(format!("{}@{}", entry.skill_id, entry.version)) {
            return Err("SKILL_UNTRUSTED: duplicate skill version".to_owned());
        }
        validate_embedded_bundle(&entry, archive)?;
        let canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&entry)
                .map_err(|error| format!("skill manifest serialization failed: {error}"))?,
        );
        let (execution_availability, missing_operator_ids) =
            execution_availability(&entry.operator_ids);
        manifests.push(SkillBundleManifestRecord {
            schema_version: "SkillBundleManifest@1".to_owned(),
            skill_id: entry.skill_id,
            version: entry.version,
            status: "development-only".to_owned(),
            publisher: "forgecad-first-party".to_owned(),
            contract_range: "forgecad-runtime-contracts@1".to_owned(),
            input_schema: entry.input_schema,
            output_schema: entry.output_schema,
            recipe: entry.recipe,
            operator_ids: entry.operator_ids,
            validator_ids: entry.validator_ids,
            capabilities: entry.capabilities,
            budgets: entry.budgets,
            benchmark_suite: entry.benchmark_suite,
            canonical_sha256,
            trust_profile: "development-root".to_owned(),
            signature: "development-only".to_owned(),
            execution_availability,
            missing_operator_ids,
        });
    }
    manifests.sort_by(|left, right| {
        left.skill_id
            .cmp(&right.skill_id)
            .then(left.version.cmp(&right.version))
    });
    Ok(manifests)
}

fn execution_availability(operator_ids: &[String]) -> (SkillExecutionAvailability, Vec<String>) {
    let missing_operator_ids = operator_ids
        .iter()
        .filter(|operator_id| !EXECUTABLE_OPERATOR_IDS.contains(&operator_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let availability = if missing_operator_ids.is_empty() {
        SkillExecutionAvailability::Active
    } else if missing_operator_ids.len() == operator_ids.len() {
        SkillExecutionAvailability::Unavailable
    } else {
        SkillExecutionAvailability::Partial
    };
    (availability, missing_operator_ids)
}

pub fn get(skill_id: &str, version: &str) -> Result<Option<SkillBundleManifestRecord>, String> {
    Ok(list()?
        .into_iter()
        .find(|skill| skill.skill_id == skill_id && skill.version == version))
}

fn validate_entry(entry: &RegistryEntry) -> Result<(), String> {
    if !is_lower_identifier(&entry.skill_id)
        || !is_semver(&entry.version)
        || entry.operator_ids.is_empty()
        || entry.operator_ids.len() > 32
        || entry.validator_ids.is_empty()
        || entry.validator_ids.len() > 32
        || entry.benchmark_suite.is_empty()
        || !valid_contract_reference(&entry.input_schema)
        || !valid_contract_reference(&entry.output_schema)
        || !valid_registry_recipe_reference(&entry.recipe)
    {
        return Err("SKILL_UNTRUSTED: registry entry identity or allowlist is invalid".to_owned());
    }
    if unique_nonempty(&entry.operator_ids).is_none()
        || unique_nonempty(&entry.validator_ids).is_none()
    {
        return Err("SKILL_UNTRUSTED: duplicate or empty registry allowlist entry".to_owned());
    }
    if entry
        .operator_ids
        .iter()
        .any(|operator| !is_operator_id(operator))
    {
        return Err("SKILL_UNTRUSTED: unknown operator namespace or version".to_owned());
    }
    if entry
        .validator_ids
        .iter()
        .any(|validator| !validator.ends_with("@1") || validator.contains(['/', '\\']))
    {
        return Err("SKILL_UNTRUSTED: validator allowlist is invalid".to_owned());
    }
    for key in [
        "network",
        "filesystem_read",
        "filesystem_write",
        "dynamic_code",
        "model_calls",
    ] {
        if entry.capabilities.get(key) != Some(&Value::Bool(false)) {
            return Err(format!("SKILL_UNTRUSTED: capability {key} must be false"));
        }
    }
    for key in ["geometry_execution", "render_execution"] {
        if !entry.capabilities.get(key).is_some_and(Value::is_boolean) {
            return Err(format!("SKILL_UNTRUSTED: capability {key} must be boolean"));
        }
    }
    if !entry.budgets.is_object()
        || entry
            .budgets
            .as_object()
            .is_some_and(|budgets| budgets.is_empty())
    {
        return Err("SKILL_UNTRUSTED: budgets must be a non-empty object".to_owned());
    }
    Ok(())
}

fn valid_contract_reference(reference: &str) -> bool {
    reference
        .strip_prefix("contracts/")
        .is_some_and(valid_contract_file_name)
}

fn valid_registry_recipe_reference(reference: &str) -> bool {
    reference
        .strip_prefix("recipes/")
        .is_some_and(|name| valid_contract_file_name(name) && name.ends_with(".recipe.json"))
}

fn valid_contract_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.contains(['/', '\\'])
        && !name.contains("..")
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn unique_nonempty(values: &[String]) -> Option<()> {
    if values.iter().any(String::is_empty) {
        return None;
    }
    let unique = values.iter().collect::<HashSet<_>>();
    (unique.len() == values.len()).then_some(())
}

fn validate_registry_trust() -> Result<(), String> {
    let expected = format!("{}  registry.json", sha256_hex(REGISTRY_JSON.as_bytes()));
    let lines = REGISTRY_TRUST
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect::<Vec<_>>();
    if lines != [expected] {
        return Err(
            "SKILL_UNTRUSTED: development trust root does not bind registry bytes".to_owned(),
        );
    }
    Ok(())
}

fn bundle_archive() -> Result<&'static BundleArchive, String> {
    match EMBEDDED_BUNDLE_ARCHIVE.get_or_init(parse_bundle_archive) {
        Ok(archive) => Ok(archive),
        Err(error) => Err(error.clone()),
    }
}

fn parse_bundle_archive() -> Result<BundleArchive, String> {
    if BUNDLE_ARCHIVE.len() > MAX_ARCHIVE_BYTES || !BUNDLE_ARCHIVE.starts_with(ARCHIVE_MAGIC) {
        return Err("SKILL_UNTRUSTED: embedded bundle archive header is invalid".to_owned());
    }
    let mut cursor = ARCHIVE_MAGIC.len();
    let count = take_u32(BUNDLE_ARCHIVE, &mut cursor, "file count")? as usize;
    if count == 0 || count > MAX_ARCHIVE_FILES {
        return Err("SKILL_UNTRUSTED: embedded bundle archive file count is invalid".to_owned());
    }
    let mut files = BTreeMap::new();
    for _ in 0..count {
        let path_length = take_u16(BUNDLE_ARCHIVE, &mut cursor, "path length")? as usize;
        if path_length == 0 || path_length > 512 {
            return Err("SKILL_UNTRUSTED: embedded bundle artifact path is invalid".to_owned());
        }
        let path = std::str::from_utf8(take_bytes(
            BUNDLE_ARCHIVE,
            &mut cursor,
            path_length,
            "path",
        )?)
        .map_err(|_| "SKILL_UNTRUSTED: embedded bundle path is not UTF-8".to_owned())?
        .to_owned();
        if !valid_archive_path(&path) {
            return Err("SKILL_UNTRUSTED: embedded bundle path is unsafe".to_owned());
        }
        let content_length = take_u32(BUNDLE_ARCHIVE, &mut cursor, "content length")? as usize;
        if content_length > MAX_ARTIFACT_BYTES {
            return Err("SKILL_UNTRUSTED: embedded bundle artifact exceeds size limit".to_owned());
        }
        let content = take_bytes(BUNDLE_ARCHIVE, &mut cursor, content_length, "content")?.to_vec();
        if files.insert(path, content).is_some() {
            return Err("SKILL_UNTRUSTED: embedded bundle archive has duplicate paths".to_owned());
        }
    }
    if cursor != BUNDLE_ARCHIVE.len() {
        return Err("SKILL_UNTRUSTED: embedded bundle archive has trailing data".to_owned());
    }
    Ok(BundleArchive { files })
}

fn take_u16(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u16, String> {
    let value = take_bytes(bytes, cursor, 2, label)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn take_u32(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u32, String> {
    let value = take_bytes(bytes, cursor, 4, label)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn take_bytes<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
    label: &str,
) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| format!("SKILL_UNTRUSTED: embedded bundle archive truncated {label}"))?;
    let value = &bytes[*cursor..end];
    *cursor = end;
    Ok(value)
}

fn valid_archive_path(path: &str) -> bool {
    (path.starts_with("bundles/") || path.starts_with("contracts/"))
        && !path.contains(['\\', '\0'])
        && !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
}

fn validate_embedded_bundle(entry: &RegistryEntry, archive: &BundleArchive) -> Result<(), String> {
    let key = format!("{}@{}", entry.skill_id, entry.version);
    let prefix = format!("bundles/{}/{}/", entry.skill_id, entry.version);
    let input_schema = bundle_schema_path(&entry.input_schema)?;
    let output_schema = bundle_schema_path(&entry.output_schema)?;
    let mut required = REQUIRED_BUNDLE_ARTIFACTS.to_vec();
    required.push(&input_schema);
    required.push(&output_schema);
    for relative in required {
        bundle_bytes(archive, &prefix, relative, &key)?;
    }
    for path in archive
        .files
        .keys()
        .filter(|path| path.starts_with(&prefix))
    {
        if FORBIDDEN_BUNDLE_SUFFIXES
            .iter()
            .any(|suffix| path.ends_with(suffix))
        {
            return Err(format!(
                "SKILL_UNTRUSTED: {key} embeds an executable artifact"
            ));
        }
    }

    let manifest_bytes = bundle_bytes(archive, &prefix, "manifest.json", &key)?;
    let manifest = parse_json(manifest_bytes, &format!("{key} manifest"))?;
    validate_bundle_manifest(entry, &manifest, &input_schema, &output_schema, &key)?;
    validate_skill_yaml(
        bundle_text(archive, &prefix, "skill.yaml", &key)?,
        entry,
        &key,
    )?;
    validate_operator_lock(
        bundle_text(archive, &prefix, "operators.lock", &key)?,
        &entry.operator_ids,
        &key,
    )?;
    validate_validator_set(
        parse_bundle_json(archive, &prefix, "validators/validator-set.json", &key)?,
        &entry.validator_ids,
        &key,
    )?;
    validate_asset_or_material_index(
        parse_bundle_json(archive, &prefix, "assets/index.json", &key)?,
        "SkillAssetIndex@1",
        "assets",
        entry,
        &key,
    )?;
    validate_asset_or_material_index(
        parse_bundle_json(archive, &prefix, "materials/index.json", &key)?,
        "SkillMaterialIndex@1",
        "materials",
        entry,
        &key,
    )?;
    validate_recipe(
        parse_bundle_json(archive, &prefix, "recipes/default.recipe.json", &key)?,
        entry,
        &key,
    )?;
    validate_bundle_schema(archive, &prefix, &input_schema, &entry.input_schema, &key)?;
    validate_bundle_schema(archive, &prefix, &output_schema, &entry.output_schema, &key)?;
    validate_benchmark_and_trust(archive, &prefix, entry, manifest_bytes, &key)?;
    for relative in [
        "knowledge/overview.md",
        "knowledge/constraints.md",
        "knowledge/examples.md",
        "LICENSES/ForgeCAD-FIRST-PARTY.txt",
        "NOTICE",
        "provenance.intoto.jsonl",
    ] {
        if bundle_text(archive, &prefix, relative, &key)?
            .trim()
            .is_empty()
        {
            return Err(format!(
                "SKILL_UNTRUSTED: {key} has an empty required artifact"
            ));
        }
    }
    validate_sbom(
        parse_bundle_json(archive, &prefix, "sbom.spdx.json", &key)?,
        entry,
        &key,
    )?;
    validate_provenance(
        bundle_text(archive, &prefix, "provenance.intoto.jsonl", &key)?,
        sha256_hex(manifest_bytes),
        &key,
    )?;
    Ok(())
}

fn bundle_schema_path(contract_reference: &str) -> Result<String, String> {
    let file_name = contract_reference
        .strip_prefix("contracts/")
        .filter(|name| valid_contract_file_name(name))
        .ok_or_else(|| "SKILL_UNTRUSTED: registry contract reference is unsafe".to_owned())?;
    Ok(format!("schemas/{file_name}"))
}

fn bundle_bytes<'a>(
    archive: &'a BundleArchive,
    prefix: &str,
    relative: &str,
    key: &str,
) -> Result<&'a [u8], String> {
    archive
        .files
        .get(&format!("{prefix}{relative}"))
        .map(Vec::as_slice)
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} is missing required artifact {relative}"))
}

fn bundle_text<'a>(
    archive: &'a BundleArchive,
    prefix: &str,
    relative: &str,
    key: &str,
) -> Result<&'a str, String> {
    std::str::from_utf8(bundle_bytes(archive, prefix, relative, key)?)
        .map_err(|_| format!("SKILL_UNTRUSTED: {key} artifact {relative} is not UTF-8"))
}

fn parse_bundle_json(
    archive: &BundleArchive,
    prefix: &str,
    relative: &str,
    key: &str,
) -> Result<Value, String> {
    parse_json(
        bundle_bytes(archive, prefix, relative, key)?,
        &format!("{key} {relative}"),
    )
}

fn parse_json(bytes: &[u8], label: &str) -> Result<Value, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("SKILL_UNTRUSTED: {label} JSON is invalid: {error}"))
}

fn validate_bundle_manifest(
    entry: &RegistryEntry,
    manifest: &Value,
    input_schema: &str,
    output_schema: &str,
    key: &str,
) -> Result<(), String> {
    let object = manifest
        .as_object()
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} manifest must be an object"))?;
    let expected_fields = BTreeSet::from([
        "benchmark_suite",
        "budgets",
        "canonical_sha256",
        "capabilities",
        "contract_range",
        "input_schema",
        "operator_ids",
        "output_schema",
        "publisher",
        "recipe",
        "schema_version",
        "signature",
        "skill_id",
        "status",
        "trust_profile",
        "validator_ids",
        "version",
    ]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_fields {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} manifest has an unexpected field set"
        ));
    }
    let expected_strings = [
        ("schema_version", "SkillBundleManifest@1"),
        ("skill_id", entry.skill_id.as_str()),
        ("version", entry.version.as_str()),
        ("status", "development-only"),
        ("publisher", "forgecad-first-party"),
        ("contract_range", "forgecad-runtime-contracts@1"),
        ("input_schema", input_schema),
        ("output_schema", output_schema),
        ("recipe", "recipes/default.recipe.json"),
        ("trust_profile", "development-root"),
        ("signature", "development-only"),
        ("benchmark_suite", entry.benchmark_suite.as_str()),
    ];
    for (field, expected) in expected_strings {
        if manifest.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(format!(
                "SKILL_UNTRUSTED: {key} manifest {field} drifts from registry"
            ));
        }
    }
    if manifest.get("operator_ids") != Some(&json!(entry.operator_ids))
        || manifest.get("validator_ids") != Some(&json!(entry.validator_ids))
        || manifest.get("capabilities") != Some(&entry.capabilities)
        || manifest.get("budgets") != Some(&entry.budgets)
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} manifest lock or policy drifts from registry"
        ));
    }
    let canonical_sha256 = manifest
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} manifest canonical hash is invalid"))?;
    let mut draft = manifest.clone();
    draft
        .as_object_mut()
        .expect("manifest was checked as object")
        .remove("canonical_sha256");
    if canonical_json_hash(&draft) != canonical_sha256 {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} manifest canonical hash does not bind fields"
        ));
    }
    Ok(())
}

fn validate_skill_yaml(text: &str, entry: &RegistryEntry, key: &str) -> Result<(), String> {
    let input_schema = bundle_schema_path(&entry.input_schema)?;
    let output_schema = bundle_schema_path(&entry.output_schema)?;
    for (field, expected) in [
        ("schema_version", "ForgeCADSkillBundle@1"),
        ("skill_id", entry.skill_id.as_str()),
        ("version", entry.version.as_str()),
        ("status", "development-only"),
        ("publisher", "forgecad-first-party"),
        ("contract_range", "forgecad-runtime-contracts@1"),
        ("input_schema", input_schema.as_str()),
        ("output_schema", output_schema.as_str()),
        ("recipe", "recipes/default.recipe.json"),
        ("trust_profile", "development-root"),
        ("signature", "deferred-to-mcp012-013"),
        ("benchmark_suite", entry.benchmark_suite.as_str()),
    ] {
        if yaml_scalar(text, field) != Some(expected) {
            return Err(format!(
                "SKILL_UNTRUSTED: {key} skill.yaml {field} drifts from manifest"
            ));
        }
    }
    for capability in [
        "network",
        "filesystem_read",
        "filesystem_write",
        "dynamic_code",
        "model_calls",
    ] {
        if yaml_scalar(text, capability) != Some("false") {
            return Err(format!(
                "SKILL_UNTRUSTED: {key} skill.yaml enables {capability}"
            ));
        }
    }
    for capability in ["geometry_execution", "render_execution"] {
        let expected = if entry.capabilities.get(capability) == Some(&Value::Bool(true)) {
            "true"
        } else {
            "false"
        };
        if yaml_scalar(text, capability) != Some(expected) {
            return Err(format!(
                "SKILL_UNTRUSTED: {key} skill.yaml {capability} drifts"
            ));
        }
    }
    Ok(())
}

fn yaml_scalar<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}:");
    let values = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .collect::<Vec<_>>();
    (values.len() == 1).then_some(values[0])
}

fn validate_operator_lock(text: &str, expected: &[String], key: &str) -> Result<(), String> {
    let mut actual = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with('#') {
            continue;
        }
        let (operator, implementation) = line
            .split_once(" = ")
            .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} operator lock line is malformed"))?;
        if implementation != "forgecad-runtime-builtin" || !is_operator_id(operator) {
            return Err(format!("SKILL_UNTRUSTED: {key} operator lock is unsafe"));
        }
        actual.push(operator.to_owned());
    }
    if actual != expected || unique_nonempty(&actual).is_none() {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} operator lock drifts from registry"
        ));
    }
    Ok(())
}

fn validate_validator_set(value: Value, expected: &[String], key: &str) -> Result<(), String> {
    if value.get("schema_version").and_then(Value::as_str) != Some("SkillValidatorSet@1")
        || value.get("network") != Some(&Value::Bool(false))
        || value.get("dynamic_code") != Some(&Value::Bool(false))
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} validator set header is invalid"
        ));
    }
    let validators = value
        .get("validators")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} validator set is missing validators"))?;
    let actual = validators
        .iter()
        .map(|validator| {
            let object = validator.as_object().ok_or_else(|| {
                format!("SKILL_UNTRUSTED: {key} validator entry is not an object")
            })?;
            if object.keys().map(String::as_str).collect::<BTreeSet<_>>()
                != BTreeSet::from(["builtin", "id"])
            {
                return Err(format!(
                    "SKILL_UNTRUSTED: {key} validator entry has unknown fields"
                ));
            }
            let id = validator
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| is_validator_id(id));
            let builtin = validator
                .get("builtin")
                .and_then(Value::as_str)
                .filter(|builtin| is_safe_symbol(builtin));
            match (id, builtin) {
                (Some(id), Some(_)) => Ok(id.to_owned()),
                _ => Err(format!("SKILL_UNTRUSTED: {key} validator entry is unsafe")),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if unique_nonempty(&actual).is_none()
        || actual.iter().collect::<BTreeSet<_>>() != expected.iter().collect::<BTreeSet<_>>()
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} validator set drifts from registry"
        ));
    }
    Ok(())
}

fn validate_asset_or_material_index(
    value: Value,
    schema_version: &str,
    collection: &str,
    entry: &RegistryEntry,
    key: &str,
) -> Result<(), String> {
    if value.get("schema_version").and_then(Value::as_str) != Some(schema_version)
        || value.get("skill_id").and_then(Value::as_str) != Some(entry.skill_id.as_str())
        || value.get("network") != Some(&Value::Bool(false))
        || !value.get(collection).is_some_and(Value::is_array)
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} {collection} index is invalid"
        ));
    }
    if value
        .get(collection)
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} declarative MCP006 bundle contains unadopted {collection} payloads"
        ));
    }
    Ok(())
}

fn validate_recipe(value: Value, entry: &RegistryEntry, key: &str) -> Result<(), String> {
    if value.get("schema_version").and_then(Value::as_str) != Some("RecipePlan@1")
        || value.get("skill_id").and_then(Value::as_str) != Some(entry.skill_id.as_str())
        || value.get("units").and_then(Value::as_str) != Some("meter")
        || value.get("coordinate_system").and_then(Value::as_str) != Some("right-handed-y-up")
    {
        return Err(format!("SKILL_UNTRUSTED: {key} recipe header is invalid"));
    }
    let canonical_sha256 = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe canonical hash is invalid"))?;
    let mut draft = value.clone();
    draft
        .as_object_mut()
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe must be an object"))?
        .remove("canonical_sha256");
    if canonical_json_hash(&draft) != canonical_sha256 {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} recipe canonical hash does not bind fields"
        ));
    }
    let nodes = value
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= 64)
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe nodes are invalid"))?;
    let mut node_ids = Vec::with_capacity(nodes.len());
    for node in nodes {
        let node_id = node
            .get("node_id")
            .and_then(Value::as_str)
            .filter(|id| is_lower_identifier(id))
            .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe node ID is invalid"))?;
        let operator_id = node
            .get("operator_id")
            .and_then(Value::as_str)
            .filter(|operator| {
                entry
                    .operator_ids
                    .iter()
                    .any(|expected| expected == operator)
            })
            .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe operator is outside lock"))?;
        let _ = operator_id;
        for field in ["input_schema", "output_schema"] {
            if !node
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(is_recipe_contract_identifier)
            {
                return Err(format!("SKILL_UNTRUSTED: {key} recipe {field} is invalid"));
            }
        }
        node_ids.push(node_id.to_owned());
    }
    if unique_nonempty(&node_ids).is_none()
        || value.get("deterministic_order") != Some(&json!(node_ids))
    {
        return Err(format!("SKILL_UNTRUSTED: {key} recipe order is invalid"));
    }
    let edges = value
        .get("edges")
        .and_then(Value::as_array)
        .filter(|edges| edges.len() <= 128)
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe edges are invalid"))?;
    let max_edges = value.get("max_edges").and_then(Value::as_u64);
    let max_nodes = value
        .get("budgets")
        .and_then(Value::as_object)
        .and_then(|budgets| budgets.get("max_nodes"))
        .and_then(Value::as_u64);
    if max_edges.is_none_or(|maximum| maximum < edges.len() as u64 || maximum > 128)
        || max_nodes.is_none_or(|maximum| maximum < nodes.len() as u64 || maximum > 512)
    {
        return Err(format!("SKILL_UNTRUSTED: {key} recipe budget is invalid"));
    }
    validate_acyclic_recipe_edges(edges, &node_ids, key)
}

fn validate_acyclic_recipe_edges(
    edges: &[Value],
    node_ids: &[String],
    key: &str,
) -> Result<(), String> {
    let known = node_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    for node_id in node_ids {
        graph.insert(node_id.clone(), Vec::new());
    }
    for edge in edges {
        let source = edge
            .get("from")
            .and_then(Value::as_str)
            .filter(|node_id| known.contains(*node_id))
            .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe edge source is invalid"))?;
        let target = edge
            .get("to")
            .and_then(Value::as_str)
            .filter(|node_id| known.contains(*node_id))
            .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} recipe edge target is invalid"))?;
        graph
            .get_mut(source)
            .expect("known recipe node is initialized")
            .push(target.to_owned());
    }
    let mut visited = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for node_id in node_ids {
        if !visit_recipe_node(node_id, &graph, &mut visiting, &mut visited) {
            return Err(format!("SKILL_UNTRUSTED: {key} recipe contains a cycle"));
        }
    }
    Ok(())
}

fn visit_recipe_node(
    node_id: &str,
    graph: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if visited.contains(node_id) {
        return true;
    }
    if !visiting.insert(node_id.to_owned()) {
        return false;
    }
    let acyclic = graph.get(node_id).is_some_and(|children| {
        children
            .iter()
            .all(|child| visit_recipe_node(child, graph, visiting, visited))
    });
    visiting.remove(node_id);
    if acyclic {
        visited.insert(node_id.to_owned());
    }
    acyclic
}

fn validate_bundle_schema(
    archive: &BundleArchive,
    prefix: &str,
    bundle_relative: &str,
    contract_reference: &str,
    key: &str,
) -> Result<(), String> {
    let schema_name = contract_reference
        .strip_prefix("contracts/")
        .filter(|name| valid_contract_file_name(name))
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} contract reference is invalid"))?;
    let bundle_schema = bundle_bytes(archive, prefix, bundle_relative, key)?;
    let contract_schema = archive
        .files
        .get(&format!("contracts/{schema_name}"))
        .map(Vec::as_slice)
        .ok_or_else(|| format!("SKILL_UNTRUSTED: {key} contract source is missing"))?;
    if bundle_schema != contract_schema {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} bundled schema drifts from contract source"
        ));
    }
    if parse_json(bundle_schema, &format!("{key} bundled schema"))?
        .get("$schema")
        .and_then(Value::as_str)
        != Some("https://json-schema.org/draft/2020-12/schema")
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} bundled schema is not draft 2020-12"
        ));
    }
    Ok(())
}

fn validate_benchmark_and_trust(
    archive: &BundleArchive,
    prefix: &str,
    entry: &RegistryEntry,
    manifest_bytes: &[u8],
    key: &str,
) -> Result<(), String> {
    let suite = bundle_text(archive, prefix, "benchmarks/suite.yaml", key)?;
    if yaml_scalar(suite, "suite_id") != Some(entry.benchmark_suite.as_str())
        || yaml_scalar(suite, "status") != Some("passed")
        || !suite.contains("fixtures/valid.json")
        || !suite.contains("fixtures/invalid-cycle-unit-finite.json")
    {
        return Err(format!("SKILL_UNTRUSTED: {key} benchmark suite is invalid"));
    }
    let valid = parse_bundle_json(archive, prefix, "benchmarks/fixtures/valid.json", key)?;
    let invalid = parse_bundle_json(
        archive,
        prefix,
        "benchmarks/fixtures/invalid-cycle-unit-finite.json",
        key,
    )?;
    if valid
        .get("expected")
        .and_then(Value::as_object)
        .and_then(|expected| expected.get("dag"))
        != Some(&Value::String("acyclic".to_owned()))
        || invalid
            .get("expected")
            .and_then(Value::as_object)
            .and_then(|expected| expected.get("dag"))
            != Some(&Value::String("reject".to_owned()))
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} benchmark fixtures are invalid"
        ));
    }
    let receipt = parse_bundle_json(archive, prefix, "benchmark-receipt.json", key)?;
    let expected_fixture_sha256 = canonical_json_hash(&json!({"valid": valid, "invalid": invalid}));
    if receipt.get("schema_version").and_then(Value::as_str) != Some("SkillBenchmarkReceipt@1")
        || receipt.get("skill_id").and_then(Value::as_str) != Some(entry.skill_id.as_str())
        || receipt.get("version").and_then(Value::as_str) != Some(entry.version.as_str())
        || receipt.get("status").and_then(Value::as_str) != Some("passed")
        || receipt.get("suite_id").and_then(Value::as_str) != Some(entry.benchmark_suite.as_str())
        || receipt.get("fixture_sha256").and_then(Value::as_str)
            != Some(expected_fixture_sha256.as_str())
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} benchmark receipt is not bound"
        ));
    }
    let recipe_bytes = bundle_bytes(archive, prefix, "recipes/default.recipe.json", key)?;
    let manifest_sha256 = sha256_hex(manifest_bytes);
    let expected_trust = [
        format!("{manifest_sha256}  manifest.json"),
        format!("{}  recipes/default.recipe.json", sha256_hex(recipe_bytes)),
    ];
    let trust = bundle_text(archive, prefix, "trust/manifest.sha256", key)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if trust != expected_trust {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} trust manifest is incomplete"
        ));
    }
    let signature = parse_bundle_json(archive, prefix, "signature.bundle", key)?;
    if signature.get("schema_version").and_then(Value::as_str)
        != Some("ForgeCADDevelopmentSignature@1")
        || signature.get("status").and_then(Value::as_str) != Some("deferred-to-mcp012-013")
        || signature.get("trust_profile").and_then(Value::as_str) != Some("development-root")
        || signature.get("manifest_sha256").and_then(Value::as_str)
            != Some(manifest_sha256.as_str())
        || signature.get("cryptographic_signature") != Some(&Value::Null)
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} signature placeholder is invalid"
        ));
    }
    Ok(())
}

fn validate_sbom(value: Value, entry: &RegistryEntry, key: &str) -> Result<(), String> {
    let expected_name = format!("forgecad-skill-{}", entry.skill_id);
    if value.get("spdxVersion").and_then(Value::as_str) != Some("SPDX-2.3")
        || value.get("name").and_then(Value::as_str) != Some(expected_name.as_str())
        || !value.get("packages").is_some_and(Value::is_array)
    {
        return Err(format!("SKILL_UNTRUSTED: {key} SBOM is invalid"));
    }
    Ok(())
}

fn validate_provenance(text: &str, manifest_sha256: String, key: &str) -> Result<(), String> {
    let records = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_json(line.as_bytes(), &format!("{key} provenance")))
        .collect::<Result<Vec<_>, _>>()?;
    if records.len() != 1
        || records[0].get("_type").and_then(Value::as_str)
            != Some("https://in-toto.io/Statement/v1")
        || records[0]
            .get("subject")
            .and_then(Value::as_array)
            .and_then(|subjects| subjects.first())
            .and_then(|subject| subject.get("digest"))
            .and_then(|digest| digest.get("sha256"))
            .and_then(Value::as_str)
            != Some(manifest_sha256.as_str())
    {
        return Err(format!(
            "SKILL_UNTRUSTED: {key} provenance does not bind manifest bytes"
        ));
    }
    Ok(())
}

fn is_operator_id(value: &str) -> bool {
    value.starts_with("forgecad.")
        && value
            .rsplit_once('@')
            .is_some_and(|(name, version)| is_safe_symbol(name) && matches!(version, "1" | "2"))
}

fn is_validator_id(value: &str) -> bool {
    value
        .rsplit_once('@')
        .is_some_and(|(name, version)| is_safe_symbol(name) && version == "1")
}

fn is_recipe_contract_identifier(value: &str) -> bool {
    value.rsplit_once('@').is_some_and(|(name, version)| {
        !name.is_empty()
            && matches!(version, "1" | "2")
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    })
}

fn is_safe_symbol(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.contains("..")
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_lower_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

fn is_semver(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

pub fn list_result() -> Result<Value, String> {
    Ok(json!({
        "schema_version": "SkillListResult@1",
        "skills": list()?
    }))
}

pub fn get_result(skill_id: &str, version: &str) -> Result<Value, String> {
    let skill = get(skill_id, version)?.ok_or_else(|| {
        "CAPABILITY_UNAVAILABLE: Skill version is not in the first-party registry".to_owned()
    })?;
    Ok(json!({
        "schema_version": "SkillGetResult@1",
        "skill": skill
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_first_party_bounded_and_deterministic() {
        let first = list().expect("registry");
        let second = list().expect("registry second read");
        assert_eq!(first.len(), 11);
        assert_eq!(first, second);
        assert!(first
            .iter()
            .filter(|skill| {
                skill.skill_id != "primitive-blockout"
                    && skill.skill_id != "hard-surface-detail"
                    && skill.skill_id != "uv-pbr"
            })
            .all(|skill| {
                skill.publisher == "forgecad-first-party"
                    && skill.status == "development-only"
                    && skill.signature == "development-only"
                    && skill.capabilities["network"] == false
                    && skill.capabilities["dynamic_code"] == false
                    && skill.canonical_sha256.len() == 64
                    && skill.execution_availability != SkillExecutionAvailability::Active
            }));
        let primitive = first
            .iter()
            .find(|skill| skill.skill_id == "primitive-blockout")
            .expect("primitive blockout skill");
        assert_eq!(primitive.version, "0.2.0");
        assert_eq!(
            primitive.execution_availability,
            SkillExecutionAvailability::Active
        );
        assert!(primitive.missing_operator_ids.is_empty());
        let hard_surface = first
            .iter()
            .find(|skill| skill.skill_id == "hard-surface-detail")
            .expect("hard surface detail skill");
        assert_eq!(hard_surface.version, "0.2.0");
        assert_eq!(
            hard_surface.execution_availability,
            SkillExecutionAvailability::Active
        );
        assert!(hard_surface.missing_operator_ids.is_empty());
        let uv_pbr = first
            .iter()
            .find(|skill| skill.skill_id == "uv-pbr")
            .expect("uv-pbr skill");
        assert_eq!(uv_pbr.version, "0.2.0");
        assert_eq!(
            uv_pbr.execution_availability,
            SkillExecutionAvailability::Active
        );
        assert!(uv_pbr.missing_operator_ids.is_empty());
    }

    #[test]
    fn registry_execution_availability_is_derived_from_real_operator_consumers() {
        let skills = list().expect("registry");
        let silhouette = skills
            .iter()
            .find(|skill| skill.skill_id == "silhouette-blockout")
            .expect("silhouette blockout skill");
        assert_eq!(
            silhouette.execution_availability,
            SkillExecutionAvailability::Partial
        );
        assert_eq!(
            silhouette.missing_operator_ids,
            vec!["forgecad.geometry.transform@1".to_owned()]
        );

        let reference_intake = skills
            .iter()
            .find(|skill| skill.skill_id == "reference-intake")
            .expect("reference intake skill");
        assert_eq!(
            reference_intake.execution_availability,
            SkillExecutionAvailability::Unavailable
        );
        assert_eq!(
            reference_intake.missing_operator_ids,
            vec![
                "forgecad.reference.validate@1".to_owned(),
                "forgecad.reference.inventory@1".to_owned(),
            ]
        );

        let (availability, missing) =
            execution_availability(&["forgecad.geometry.primitive@1".to_owned()]);
        assert_eq!(availability, SkillExecutionAvailability::Active);
        assert!(missing.is_empty());

        let (availability, missing) =
            execution_availability(&["forgecad.geometry.primitive@2".to_owned()]);
        assert_eq!(availability, SkillExecutionAvailability::Active);
        assert!(missing.is_empty());
    }

    #[test]
    fn unknown_skill_is_not_silently_selected() {
        assert!(get("not-a-skill", "0.1.0").expect("registry").is_none());
    }

    #[test]
    fn bundle_integrity_rejects_missing_or_drifting_artifacts_before_availability() {
        let document: RegistryDocument =
            serde_json::from_str(REGISTRY_JSON).expect("registry JSON");
        let entry = document
            .skills
            .into_iter()
            .find(|entry| entry.skill_id == "silhouette-blockout")
            .expect("silhouette entry");
        let archive = bundle_archive().expect("embedded archive");
        validate_embedded_bundle(&entry, archive).expect("valid immutable bundle");

        let prefix = format!("bundles/{}/{}/", entry.skill_id, entry.version);
        let mut missing_lock = archive.clone();
        missing_lock
            .files
            .remove(&format!("{prefix}operators.lock"));
        assert!(validate_embedded_bundle(&entry, &missing_lock).is_err());

        let mut changed_lock = archive.clone();
        changed_lock.files.insert(
            format!("{prefix}operators.lock"),
            b"forgecad.geometry.primitive@1 = unexpected-runtime\n".to_vec(),
        );
        assert!(validate_embedded_bundle(&entry, &changed_lock).is_err());

        let mut active_only_if_verified = entry.clone();
        active_only_if_verified.operator_ids = vec!["forgecad.geometry.primitive@1".to_owned()];
        assert!(validate_embedded_bundle(&active_only_if_verified, archive).is_err());
    }
}
