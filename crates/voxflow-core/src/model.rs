use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::paths::VoxflowPaths;

pub const PROFILE_DIR_ENV: &str = "VOXFLOW_PROFILE_DIR";
pub const MANIFEST_LOCK_FILE: &str = "manifest.lock";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelProfileDocument {
    pub profile: ModelProfile,
    pub source: ModelSource,
    #[serde(default)]
    pub files: Vec<ModelFileSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelProfile {
    pub id: String,
    pub label: String,
    pub kind: ModelKind,
    pub backend: String,
    pub version: String,
    pub license: String,
    pub languages: Vec<String>,
    pub streaming: bool,
    pub recommended: bool,
    pub min_ram_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ModelKind {
    AsrStreaming,
    AsrRefiner,
    IntentClassifier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelSource {
    pub url: String,
    pub size_bytes: u64,
    /// Checksum of the downloadable archive (when `url` points at one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelFileSpec {
    pub path: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelImportMode {
    Copy,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelImportResult {
    pub model: ModelInventoryItem,
    pub mode: ModelImportMode,
    pub manifest: ManifestLock,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelDeleteResult {
    pub model_id: String,
    pub deleted: bool,
    pub released_bytes: u64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelLocalState {
    NotInstalled,
    Ready,
    Active,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInventoryItem {
    pub profile: ModelProfile,
    pub source: ModelSource,
    pub local: ModelLocalStatus,
    pub profile_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelLocalStatus {
    pub state: ModelLocalState,
    pub path: String,
    pub manifest_present: bool,
    pub total_size_bytes: u64,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestLock {
    pub model_id: String,
    pub profile_version: String,
    pub source_url: String,
    pub installed_at_unix: u64,
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub fn default_profile_dir() -> PathBuf {
    std::env::var_os(PROFILE_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("model-profiles"))
}

pub fn load_profiles(profile_dir: &Path) -> Result<Vec<ModelProfileDocument>> {
    let mut entries = fs::read_dir(profile_dir)
        .with_context(|| format!("read model profile dir {}", profile_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    let mut profiles = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read profile {}", path.display()))?;
        let document = toml::from_str::<ModelProfileDocument>(&text)
            .with_context(|| format!("parse profile {}", path.display()))?;
        profiles.push(document);
    }
    Ok(profiles)
}

pub fn list_model_inventory(
    paths: &VoxflowPaths,
    active_asr: &str,
) -> Result<Vec<ModelInventoryItem>> {
    let profile_dir = default_profile_dir();
    list_model_inventory_from_dir(&profile_dir, paths, active_asr)
}

pub fn list_model_inventory_from_dir(
    profile_dir: &Path,
    paths: &VoxflowPaths,
    active_asr: &str,
) -> Result<Vec<ModelInventoryItem>> {
    load_profiles(profile_dir).map(|profiles| {
        profiles
            .iter()
            .map(|profile| inventory_item(profile, paths, active_asr))
            .collect()
    })
}

pub fn verify_model_by_id(
    profile_dir: &Path,
    paths: &VoxflowPaths,
    active_asr: &str,
    model_id: &str,
) -> Result<ModelInventoryItem> {
    let profiles = load_profiles(profile_dir)?;
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.profile.id == model_id)
    else {
        bail!("model.not_found:{model_id}");
    };
    Ok(inventory_item(profile, paths, active_asr))
}

pub fn import_model_by_id(
    profile_dir: &Path,
    paths: &VoxflowPaths,
    active_asr: &str,
    model_id: &str,
    source_dir: &Path,
    mode: ModelImportMode,
) -> Result<ModelImportResult> {
    let profiles = load_profiles(profile_dir)?;
    let Some(profile) = profiles
        .iter()
        .find(|profile| profile.profile.id == model_id)
    else {
        bail!("model.not_found:{model_id}");
    };

    let profile_issues = validate_profile(profile);
    if has_blocking_profile_issues(&profile_issues) {
        bail!("model.profile_invalid:{}", profile_issues.join(","));
    }
    if !source_dir.is_dir() {
        bail!("model.import_source_invalid:{}", source_dir.display());
    }
    let source_dir = source_dir
        .canonicalize()
        .with_context(|| format!("canonicalize import source {}", source_dir.display()))?;
    verify_import_source(profile, &source_dir)?;

    let final_dir = paths.models.join(model_id);
    if final_dir.exists() {
        bail!("model.already_installed:{model_id}");
    }
    let staging_dir = paths
        .cache
        .join("imports")
        .join(format!("{model_id}-{}", unix_seconds()));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)
            .with_context(|| format!("remove stale staging dir {}", staging_dir.display()))?;
    }
    fs::create_dir_all(&staging_dir)
        .with_context(|| format!("create staging dir {}", staging_dir.display()))?;

    let install_result = install_profile_files(profile, &source_dir, &staging_dir, mode)
        .and_then(|()| write_manifest_lock(profile, &staging_dir));
    let manifest = match install_result {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging_dir);
            return Err(error);
        }
    };

    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create models dir {}", parent.display()))?;
    }
    fs::rename(&staging_dir, &final_dir).with_context(|| {
        format!(
            "install model {} from {} to {}",
            model_id,
            staging_dir.display(),
            final_dir.display()
        )
    })?;

    Ok(ModelImportResult {
        model: inventory_item(profile, paths, active_asr),
        mode,
        manifest,
    })
}

pub fn ensure_model_ready_by_id(
    profile_dir: &Path,
    paths: &VoxflowPaths,
    active_asr: &str,
    model_id: &str,
) -> Result<ModelInventoryItem> {
    let model = verify_model_by_id(profile_dir, paths, active_asr, model_id)?;
    match model.local.state {
        ModelLocalState::Ready | ModelLocalState::Active => Ok(model),
        ModelLocalState::NotInstalled => bail!("model.not_ready:not_installed"),
        ModelLocalState::Broken => bail!("model.not_ready:{}", model.local.issues.join(",")),
    }
}

pub fn delete_model_by_id(
    profile_dir: &Path,
    paths: &VoxflowPaths,
    active_asr: &str,
    model_id: &str,
) -> Result<ModelDeleteResult> {
    let profiles = load_profiles(profile_dir)?;
    if !profiles
        .iter()
        .any(|profile| profile.profile.id == model_id)
    {
        bail!("model.not_found:{model_id}");
    }
    let model_dir = paths.models.join(model_id);
    if model_id == active_asr {
        bail!("model.active_locked:{model_id}");
    }
    if !model_dir.exists() {
        return Ok(ModelDeleteResult {
            model_id: model_id.to_string(),
            deleted: false,
            released_bytes: 0,
            path: model_dir.display().to_string(),
        });
    }
    let released_bytes = path_size_bytes(&model_dir)
        .with_context(|| format!("measure model dir {}", model_dir.display()))?;
    if model_dir.is_dir() {
        fs::remove_dir_all(&model_dir)
            .with_context(|| format!("delete model dir {}", model_dir.display()))?;
    } else {
        fs::remove_file(&model_dir)
            .with_context(|| format!("delete model file {}", model_dir.display()))?;
    }
    Ok(ModelDeleteResult {
        model_id: model_id.to_string(),
        deleted: true,
        released_bytes,
        path: model_dir.display().to_string(),
    })
}

pub fn write_manifest_lock(
    profile: &ModelProfileDocument,
    model_dir: &Path,
) -> Result<ManifestLock> {
    let mut files = Vec::new();
    for file in &profile.files {
        let path = model_dir.join(&file.path);
        let metadata =
            fs::metadata(&path).with_context(|| format!("read model file {}", path.display()))?;
        files.push(ManifestFile {
            path: file.path.clone(),
            size_bytes: metadata.len(),
            sha256: sha256_file(&path)?,
        });
    }
    let manifest = ManifestLock {
        model_id: profile.profile.id.clone(),
        profile_version: profile.profile.version.clone(),
        source_url: profile.source.url.clone(),
        installed_at_unix: unix_seconds(),
        files,
    };
    let manifest_path = model_dir.join(MANIFEST_LOCK_FILE);
    let text = toml::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, text)
        .with_context(|| format!("write manifest {}", manifest_path.display()))?;
    Ok(manifest)
}

fn verify_import_source(profile: &ModelProfileDocument, source_dir: &Path) -> Result<()> {
    let mut issues = Vec::new();
    for file in &profile.files {
        if let Err(issue) = verify_profile_file(source_dir, file) {
            issues.push(issue);
            continue;
        }
        if let Err(issue) = verify_lightweight_format(source_dir, &file.path) {
            issues.push(issue);
        }
    }
    if !issues.is_empty() {
        bail!("model.import_verify_failed:{}", issues.join(","));
    }
    Ok(())
}

fn install_profile_files(
    profile: &ModelProfileDocument,
    source_dir: &Path,
    staging_dir: &Path,
    mode: ModelImportMode,
) -> Result<()> {
    for file in &profile.files {
        let source = source_dir.join(&file.path);
        let target = staging_dir.join(&file.path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create model file parent {}", parent.display()))?;
        }
        match mode {
            ModelImportMode::Copy => {
                fs::copy(&source, &target).with_context(|| {
                    format!(
                        "copy model file {} to {}",
                        source.display(),
                        target.display()
                    )
                })?;
            }
            ModelImportMode::Symlink => {
                symlink_file(&source, &target)?;
            }
        }
    }
    Ok(())
}

fn inventory_item(
    profile: &ModelProfileDocument,
    paths: &VoxflowPaths,
    active_asr: &str,
) -> ModelInventoryItem {
    let profile_issues = validate_profile(profile);
    let local = verify_local_status(profile, &paths.models.join(&profile.profile.id), active_asr);
    ModelInventoryItem {
        profile: profile.profile.clone(),
        source: profile.source.clone(),
        local,
        profile_issues,
    }
}

fn verify_local_status(
    profile: &ModelProfileDocument,
    model_dir: &Path,
    active_asr: &str,
) -> ModelLocalStatus {
    let mut issues = Vec::new();
    let manifest_present = model_dir.join(MANIFEST_LOCK_FILE).is_file();
    if !model_dir.exists() {
        return ModelLocalStatus {
            state: ModelLocalState::NotInstalled,
            path: model_dir.display().to_string(),
            manifest_present,
            total_size_bytes: 0,
            issues,
        };
    }

    let mut total_size_bytes = 0;
    for file in &profile.files {
        match verify_profile_file(model_dir, file) {
            Ok(size_bytes) => {
                total_size_bytes += size_bytes;
            }
            Err(issue) => issues.push(issue),
        }
    }
    if !manifest_present {
        issues.push("model.manifest_missing".to_string());
    }

    let state = if issues.is_empty() {
        if profile.profile.id == active_asr {
            ModelLocalState::Active
        } else {
            ModelLocalState::Ready
        }
    } else {
        ModelLocalState::Broken
    };

    ModelLocalStatus {
        state,
        path: model_dir.display().to_string(),
        manifest_present,
        total_size_bytes,
        issues,
    }
}

fn verify_profile_file(model_dir: &Path, file: &ModelFileSpec) -> std::result::Result<u64, String> {
    if !is_hex_sha256(&file.sha256) {
        return Err(format!("model.checksum_invalid:{}", file.path));
    }
    let path = model_dir.join(&file.path);
    let metadata = fs::metadata(&path).map_err(|_| format!("model.file_missing:{}", file.path))?;
    if !metadata.is_file() {
        return Err(format!("model.file_not_regular:{}", file.path));
    }
    let actual =
        sha256_file(&path).map_err(|_| format!("model.checksum_read_failed:{}", file.path))?;
    if actual != file.sha256.to_ascii_lowercase() {
        return Err(format!("model.checksum_failed:{}", file.path));
    }
    Ok(metadata.len())
}

fn path_size_bytes(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    let mut total = 0;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        total += path_size_bytes(&entry.path())?;
    }
    Ok(total)
}

fn validate_profile(profile: &ModelProfileDocument) -> Vec<String> {
    let mut issues = Vec::new();
    if profile.profile.id.trim().is_empty() {
        issues.push("profile.id_missing".to_string());
    }
    if profile.profile.license.trim().is_empty() {
        issues.push("profile.license_missing".to_string());
    }
    if profile.profile.recommended && profile.profile.license.trim().is_empty() {
        issues.push("profile.recommended_without_license".to_string());
    }
    if !profile.source.url.starts_with("https://") {
        issues.push("profile.source_not_https".to_string());
    }
    if profile.source.url.contains("example.invalid") {
        issues.push("profile.source_placeholder".to_string());
    }
    if profile.files.is_empty() {
        issues.push("profile.files_missing".to_string());
    }
    for file in &profile.files {
        if !is_hex_sha256(&file.sha256) {
            issues.push(format!("profile.checksum_invalid:{}", file.path));
        }
    }
    issues
}

fn has_blocking_profile_issues(issues: &[String]) -> bool {
    issues
        .iter()
        .any(|issue| issue.starts_with("profile.id_") || issue.starts_with("profile.checksum_"))
}

fn verify_lightweight_format(
    source_dir: &Path,
    relative_path: &str,
) -> std::result::Result<(), String> {
    let path = source_dir.join(relative_path);
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("onnx") => {
            let metadata = fs::metadata(&path)
                .map_err(|_| format!("model.format_unreadable:{relative_path}"))?;
            if metadata.len() == 0 {
                return Err(format!("model.format_empty:{relative_path}"));
            }
            Ok(())
        }
        Some("safetensors") => verify_safetensors_header(&path, relative_path),
        _ => Ok(()),
    }
}

fn verify_safetensors_header(path: &Path, relative_path: &str) -> std::result::Result<(), String> {
    let mut file =
        fs::File::open(path).map_err(|_| format!("model.format_unreadable:{relative_path}"))?;
    let mut len_bytes = [0_u8; 8];
    file.read_exact(&mut len_bytes)
        .map_err(|_| format!("model.format_invalid:{relative_path}"))?;
    let header_len = u64::from_le_bytes(len_bytes);
    let file_len = file
        .metadata()
        .map_err(|_| format!("model.format_unreadable:{relative_path}"))?
        .len();
    if header_len == 0 || header_len + 8 > file_len {
        return Err(format!("model.format_invalid:{relative_path}"));
    }
    Ok(())
}

#[cfg(unix)]
fn symlink_file(source: &Path, target: &Path) -> Result<()> {
    unix_fs::symlink(source, target).with_context(|| {
        format!(
            "symlink model file {} to {}",
            source.display(),
            target.display()
        )
    })
}

#[cfg(not(unix))]
fn symlink_file(_source: &Path, _target: &Path) -> Result<()> {
    bail!("model.symlink_unsupported")
}

fn is_hex_sha256(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::VoxflowPaths;

    fn unique_temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "voxflow-model-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn test_paths(home: PathBuf) -> VoxflowPaths {
        VoxflowPaths {
            config: home.join("config.toml"),
            models: home.join("models"),
            cache: home.join("cache"),
            logs: home.join("logs"),
            run: home.join("run"),
            ledger: home.join("ledger"),
            runtime_dir: home.join("run").join("voxflow"),
            socket: home.join("run").join("voxflow").join("core.sock"),
            home,
        }
    }

    fn write_profile(dir: &Path, id: &str, sha256: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join(format!("{id}.toml")),
            format!(
                r#"[profile]
id = "{id}"
label = "Test Model"
kind = "asr-streaming"
backend = "sherpa-onnx"
version = "2026.06"
license = "Apache-2.0"
languages = ["zh", "en"]
streaming = true
recommended = true
min_ram_mb = 128

[source]
url = "https://models.example.test/{id}/"
size_bytes = 3

[[files]]
path = "tokens.txt"
sha256 = "{sha256}"
"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn import_model_copy_installs_manifest_and_ready_status() {
        let root = unique_temp_dir("import-copy");
        let profiles = root.join("profiles");
        let source = root.join("source");
        let paths = test_paths(root.join("home"));
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("tokens.txt"), b"abc").unwrap();
        let sha = sha256_file(&source.join("tokens.txt")).unwrap();
        write_profile(&profiles, "streaming-zh-en-small", &sha);

        let result = import_model_by_id(
            &profiles,
            &paths,
            "other-model",
            "streaming-zh-en-small",
            &source,
            ModelImportMode::Copy,
        )
        .unwrap();

        assert_eq!(result.mode, ModelImportMode::Copy);
        assert_eq!(result.model.local.state, ModelLocalState::Ready);
        assert!(paths
            .models
            .join("streaming-zh-en-small")
            .join(MANIFEST_LOCK_FILE)
            .is_file());
        assert_eq!(
            fs::read(
                paths
                    .models
                    .join("streaming-zh-en-small")
                    .join("tokens.txt")
            )
            .unwrap(),
            b"abc"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ready_check_rejects_missing_model() {
        let root = unique_temp_dir("ready-missing");
        let profiles = root.join("profiles");
        let paths = test_paths(root.join("home"));
        write_profile(&profiles, "streaming-zh-en-small", &"0".repeat(64));

        let error =
            ensure_model_ready_by_id(&profiles, &paths, "other-model", "streaming-zh-en-small")
                .unwrap_err();

        assert!(error.to_string().starts_with("model.not_ready:"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_model_removes_ready_model_but_rejects_active_model() {
        let root = unique_temp_dir("delete");
        let profiles = root.join("profiles");
        let source = root.join("source");
        let paths = test_paths(root.join("home"));
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("tokens.txt"), b"abc").unwrap();
        let sha = sha256_file(&source.join("tokens.txt")).unwrap();
        write_profile(&profiles, "streaming-zh-en-small", &sha);
        import_model_by_id(
            &profiles,
            &paths,
            "other-model",
            "streaming-zh-en-small",
            &source,
            ModelImportMode::Copy,
        )
        .unwrap();

        let active_error = delete_model_by_id(
            &profiles,
            &paths,
            "streaming-zh-en-small",
            "streaming-zh-en-small",
        )
        .unwrap_err();
        assert!(active_error.to_string().starts_with("model.active_locked:"));

        let deleted =
            delete_model_by_id(&profiles, &paths, "other-model", "streaming-zh-en-small").unwrap();
        assert!(deleted.deleted);
        assert!(deleted.released_bytes >= 3);
        assert!(!paths.models.join("streaming-zh-en-small").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn import_model_symlink_installs_links_without_touching_source_manifest() {
        let root = unique_temp_dir("import-symlink");
        let profiles = root.join("profiles");
        let source = root.join("source");
        let paths = test_paths(root.join("home"));
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("tokens.txt"), b"abc").unwrap();
        let sha = sha256_file(&source.join("tokens.txt")).unwrap();
        write_profile(&profiles, "streaming-zh-en-small", &sha);

        let result = import_model_by_id(
            &profiles,
            &paths,
            "streaming-zh-en-small",
            "streaming-zh-en-small",
            &source,
            ModelImportMode::Symlink,
        )
        .unwrap();

        assert_eq!(result.model.local.state, ModelLocalState::Active);
        assert!(fs::symlink_metadata(
            paths
                .models
                .join("streaming-zh-en-small")
                .join("tokens.txt")
        )
        .unwrap()
        .file_type()
        .is_symlink());
        assert!(!source.join(MANIFEST_LOCK_FILE).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_marks_missing_model_as_not_installed() {
        let root = unique_temp_dir("missing");
        let profiles = root.join("profiles");
        let paths = test_paths(root.join("home"));
        write_profile(&profiles, "streaming-zh-en-small", &"0".repeat(64));

        let inventory =
            list_model_inventory_from_dir(&profiles, &paths, "streaming-zh-en-small").unwrap();

        assert_eq!(inventory.len(), 1);
        assert_eq!(inventory[0].local.state, ModelLocalState::NotInstalled);
        assert!(inventory[0].local.issues.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verify_ready_model_requires_checksum_and_manifest() {
        let root = unique_temp_dir("ready");
        let profiles = root.join("profiles");
        let paths = test_paths(root.join("home"));
        let model_dir = paths.models.join("streaming-zh-en-small");
        fs::create_dir_all(&model_dir).unwrap();
        fs::write(model_dir.join("tokens.txt"), b"abc").unwrap();
        let sha = sha256_file(&model_dir.join("tokens.txt")).unwrap();
        write_profile(&profiles, "streaming-zh-en-small", &sha);
        let profile = load_profiles(&profiles).unwrap().remove(0);
        write_manifest_lock(&profile, &model_dir).unwrap();

        let inventory =
            list_model_inventory_from_dir(&profiles, &paths, "streaming-zh-en-small").unwrap();

        assert_eq!(inventory[0].local.state, ModelLocalState::Active);
        assert!(inventory[0].local.issues.is_empty());
        assert_eq!(inventory[0].local.total_size_bytes, 3);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn placeholder_checksum_is_reported_as_broken_when_installed() {
        let root = unique_temp_dir("placeholder");
        let profiles = root.join("profiles");
        let paths = test_paths(root.join("home"));
        let model_dir = paths.models.join("streaming-zh-en-small");
        fs::create_dir_all(&model_dir).unwrap();
        fs::write(model_dir.join("tokens.txt"), b"abc").unwrap();
        write_profile(
            &profiles,
            "streaming-zh-en-small",
            "TO_BE_FILLED_AFTER_D1_POC",
        );

        let inventory =
            list_model_inventory_from_dir(&profiles, &paths, "streaming-zh-en-small").unwrap();

        assert_eq!(inventory[0].local.state, ModelLocalState::Broken);
        assert!(inventory[0]
            .profile_issues
            .contains(&"profile.checksum_invalid:tokens.txt".to_string()));
        assert!(inventory[0]
            .local
            .issues
            .contains(&"model.checksum_invalid:tokens.txt".to_string()));
        let _ = fs::remove_dir_all(root);
    }
}
