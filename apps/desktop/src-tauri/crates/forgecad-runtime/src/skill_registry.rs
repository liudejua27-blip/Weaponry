use forgecad_contracts::SkillBundleManifestRecord;
use forgecad_core::canonical_json_hash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

const REGISTRY_JSON: &str = include_str!("../../../../../../packages/forgecad-skills/registry.json");

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
        let canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&entry)
                .map_err(|error| format!("skill manifest serialization failed: {error}"))?,
        );
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
        });
    }
    manifests.sort_by(|left, right| {
        left.skill_id
            .cmp(&right.skill_id)
            .then(left.version.cmp(&right.version))
    });
    Ok(manifests)
}

pub fn get(skill_id: &str, version: &str) -> Result<Option<SkillBundleManifestRecord>, String> {
    Ok(list()?.into_iter().find(|skill| {
        skill.skill_id == skill_id && skill.version == version
    }))
}

fn validate_entry(entry: &RegistryEntry) -> Result<(), String> {
    if !is_lower_identifier(&entry.skill_id)
        || !is_semver(&entry.version)
        || entry.operator_ids.is_empty()
        || entry.validator_ids.is_empty()
        || entry.benchmark_suite.is_empty()
    {
        return Err("SKILL_UNTRUSTED: registry entry identity or allowlist is invalid".to_owned());
    }
    if entry
        .operator_ids
        .iter()
        .any(|operator| !operator.starts_with("forgecad.") || !operator.ends_with("@1"))
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
    Ok(())
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
        && parts.iter().all(|part| {
            !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
        })
}

pub fn list_result() -> Result<Value, String> {
    Ok(json!({
        "schema_version": "SkillListResult@1",
        "skills": list()?
    }))
}

pub fn get_result(skill_id: &str, version: &str) -> Result<Value, String> {
    let skill = get(skill_id, version)?
        .ok_or_else(|| "CAPABILITY_UNAVAILABLE: Skill version is not in the first-party registry".to_owned())?;
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
        assert_eq!(first.len(), 10);
        assert_eq!(first, second);
        assert!(first.iter().all(|skill| {
            skill.publisher == "forgecad-first-party"
                && skill.status == "development-only"
                && skill.signature == "development-only"
                && skill.capabilities["network"] == false
                && skill.capabilities["dynamic_code"] == false
                && skill.canonical_sha256.len() == 64
        }));
    }

    #[test]
    fn unknown_skill_is_not_silently_selected() {
        assert!(get("not-a-skill", "0.1.0").expect("registry").is_none());
    }
}
