use serde::Deserialize;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const ARCHIVE_MAGIC: &[u8; 8] = b"FCBNDL01";
// Capacity guard for the fixed active Bundle set plus checked-in contract
// schemas. Keep bounded headroom for typed contract growth; this is not a
// count assertion for the current cohort (the manifest checker owns that).
// Keep this in lockstep with `skill_registry.rs`. The archive currently
// contains the closed first-party Bundle files plus every versioned contract
// schema. 768 leaves bounded headroom for additive V2 production contracts
// without weakening the byte-size or path-validation gates below.
// Keep the embedded declarative archive bounded while leaving explicit room
// for the commercial-asset Low/UV contract families. The previous 768 cap
// was already saturated by 499 public schemas plus the active first-party
// Bundle files, so adding closed contracts could not compile at all.
const MAX_ARCHIVE_FILES: usize = 896;
// Declarative file contents remain capped at 3 MiB. The serialized envelope
// additionally carries one bounded path and two length fields per file.
const MAX_ARCHIVE_BYTES: usize = 3 * 1024 * 1024;
const MAX_ARCHIVE_ENVELOPE_BYTES: usize =
    12 + MAX_ARCHIVE_BYTES + MAX_ARCHIVE_FILES * (2 + 512 + 4);
const MAX_ARTIFACT_BYTES: usize = 256 * 1024;

#[derive(Deserialize)]
struct SkillRegistry {
    skills: Vec<RegistrySkill>,
}

#[derive(Deserialize)]
struct RegistrySkill {
    skill_id: String,
    version: String,
}

fn collect_files(root: &Path, prefix: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    let entries = fs::read_dir(root).unwrap_or_else(|error| {
        panic!("cannot read Skill build input {}: {error}", root.display())
    });
    let mut paths = entries
        .map(|entry| entry.expect("Skill build input directory entry").path())
        .collect::<Vec<_>>();
    paths.sort();

    for path in paths {
        let metadata = fs::symlink_metadata(&path).unwrap_or_else(|error| {
            panic!("cannot stat Skill build input {}: {error}", path.display())
        });
        if metadata.file_type().is_symlink() {
            panic!(
                "Skill build input must not contain symlinks: {}",
                path.display()
            );
        }
        if metadata.is_dir() {
            collect_files(&path, prefix, files);
            continue;
        }
        if !metadata.is_file() {
            panic!(
                "Skill build input must be a regular file: {}",
                path.display()
            );
        }
        if metadata.len() > MAX_ARTIFACT_BYTES as u64 {
            panic!(
                "Skill build input exceeds the declarative artifact limit: {}",
                path.display()
            );
        }
        let repository_relative = path.strip_prefix(prefix).unwrap_or_else(|_| {
            panic!(
                "Skill build input escaped its source root: {}",
                path.display()
            )
        });
        let normalized = repository_relative
            .components()
            .map(|component| match component {
                Component::Normal(value) => value
                    .to_str()
                    .expect("Skill build input path must be UTF-8")
                    .to_owned(),
                _ => panic!(
                    "Skill build input path is not normalized: {}",
                    path.display()
                ),
            })
            .collect::<Vec<_>>()
            .join("/");
        if normalized.is_empty() || normalized.contains('\\') {
            panic!("Skill build input path is unsafe: {}", path.display());
        }
        println!("cargo:rerun-if-changed={}", path.display());
        files.push((normalized, fs::read(&path).expect("read Skill build input")));
    }
}

fn valid_bundle_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.contains("..")
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

fn active_bundle_dirs(skill_bundles: &Path, registry_path: &Path) -> Vec<PathBuf> {
    let registry_bytes = fs::read(registry_path).unwrap_or_else(|error| {
        panic!(
            "cannot read Skill registry {}: {error}",
            registry_path.display()
        )
    });
    let registry: SkillRegistry = serde_json::from_slice(&registry_bytes).unwrap_or_else(|error| {
        panic!(
            "cannot parse Skill registry {}: {error}",
            registry_path.display()
        )
    });
    if registry.skills.is_empty() {
        panic!("Skill registry has no active entries");
    }

    let mut bundle_dirs = Vec::new();
    for entry in registry.skills {
        if !valid_bundle_component(&entry.skill_id) || !valid_bundle_component(&entry.version) {
            panic!("Skill registry has an unsafe bundle path");
        }
        let bundle = skill_bundles.join(&entry.skill_id).join(&entry.version);
        let metadata = fs::symlink_metadata(&bundle).unwrap_or_else(|error| {
            panic!(
                "active Skill bundle {} is missing: {error}",
                bundle.display()
            )
        });
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            panic!(
                "active Skill bundle must be a real directory: {}",
                bundle.display()
            );
        }
        bundle_dirs.push(bundle);
    }
    bundle_dirs.sort();
    if bundle_dirs.windows(2).any(|window| window[0] == window[1]) {
        panic!("Skill registry has duplicate active bundle paths");
    }
    bundle_dirs
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest dir"));
    let repository_root = manifest_dir.join("../../../../..");
    let skill_bundles = repository_root.join("packages/forgecad-skills/bundles");
    let skill_registry = repository_root.join("packages/forgecad-skills/registry.json");
    let contract_schemas = repository_root.join("packages/forgecad-contracts/schemas");
    println!("cargo:rerun-if-changed={}", skill_bundles.display());
    println!("cargo:rerun-if-changed={}", skill_registry.display());
    println!("cargo:rerun-if-changed={}", contract_schemas.display());

    // `collect_files` stores paths relative to each passed source directory.
    // Add only fixed, relative prefixes so the Runtime never needs a worktree
    // path or a runtime filesystem lookup to verify a Bundle.
    let mut archive_files = Vec::new();
    let mut bundle_files = Vec::new();
    for bundle in active_bundle_dirs(&skill_bundles, &skill_registry) {
        collect_files(&bundle, &skill_bundles, &mut bundle_files);
    }
    archive_files.extend(
        bundle_files
            .into_iter()
            .map(|(path, bytes)| (format!("bundles/{path}"), bytes)),
    );
    let mut schema_files = Vec::new();
    collect_files(&contract_schemas, &contract_schemas, &mut schema_files);
    archive_files.extend(
        schema_files
            .into_iter()
            .map(|(path, bytes)| (format!("contracts/{path}"), bytes)),
    );
    archive_files.sort_by(|left, right| left.0.cmp(&right.0));
    if archive_files.len() > MAX_ARCHIVE_FILES {
        panic!("Skill build archive has too many files");
    }
    if archive_files
        .windows(2)
        .any(|window| window[0].0 == window[1].0)
    {
        panic!("Skill build archive has duplicate paths");
    }

    let total_bytes = archive_files
        .iter()
        .map(|(_, bytes)| bytes.len())
        .sum::<usize>();
    if total_bytes > MAX_ARCHIVE_BYTES {
        panic!("Skill build archive exceeds the declarative size limit");
    }
    let serialized_bytes = 12
        + archive_files
            .iter()
            .map(|(path, bytes)| 2 + path.len() + 4 + bytes.len())
            .sum::<usize>();
    if serialized_bytes > MAX_ARCHIVE_ENVELOPE_BYTES {
        panic!("Skill build archive exceeds the bounded envelope limit");
    }

    let output = PathBuf::from(env::var("OUT_DIR").expect("Cargo output dir"))
        .join("forgecad_skill_bundles.bin");
    let mut archive = fs::File::create(output).expect("create Skill build archive");
    archive
        .write_all(ARCHIVE_MAGIC)
        .expect("write Skill archive magic");
    archive
        .write_all(&(archive_files.len() as u32).to_le_bytes())
        .expect("write Skill archive file count");
    for (path, bytes) in archive_files {
        let path_bytes = path.as_bytes();
        archive
            .write_all(&(path_bytes.len() as u16).to_le_bytes())
            .expect("write Skill archive path length");
        archive
            .write_all(path_bytes)
            .expect("write Skill archive path");
        archive
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .expect("write Skill archive content length");
        archive
            .write_all(&bytes)
            .expect("write Skill archive content");
    }
}
