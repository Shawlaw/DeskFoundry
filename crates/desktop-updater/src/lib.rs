//! Signed, framework-agnostic updates for portable Windows desktop applications.
//!
//! The application process checks and downloads an update.  A tiny helper binary
//! (which calls [`run_helper_from_args`]) performs the file replacement after the
//! application has exited, then keeps rollback files until the new process calls
//! [`acknowledge_if_requested`].

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use reqwest::{blocking::Client, header::CACHE_CONTROL, Proxy};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

pub const UPDATE_MANIFEST_SCHEMA_VERSION: u8 = 1;
pub const DEFAULT_MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_MAX_EXTRACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const HELPER_ACK_ENV: &str = "DESKTOP_UPDATER_ACK_PATH";
const HELPER_WAIT_TIMEOUT: Duration = Duration::from_secs(60);

pub type Result<T> = std::result::Result<T, UpdateError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateError {
    message: String,
}

impl UpdateError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for UpdateError {}

impl From<io::Error> for UpdateError {
    fn from(error: io::Error) -> Self {
        Self::new(error.to_string())
    }
}

/// Static application data required to trust and apply updates.
#[derive(Debug, Clone)]
pub struct UpdateConfig {
    pub app_id: String,
    pub channel: String,
    pub current_version: String,
    pub manifest_url: String,
    pub signature_url: String,
    /// Base64-encoded 32-byte Ed25519 public key. Never ship a private key.
    pub public_key_base64: String,
    pub target: String,
    pub proxy_url: Option<String>,
    pub max_download_bytes: u64,
}

impl UpdateConfig {
    pub fn new(
        app_id: impl Into<String>,
        channel: impl Into<String>,
        current_version: impl Into<String>,
        manifest_url: impl Into<String>,
        signature_url: impl Into<String>,
        public_key_base64: impl Into<String>,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            channel: channel.into(),
            current_version: current_version.into(),
            manifest_url: manifest_url.into(),
            signature_url: signature_url.into(),
            public_key_base64: public_key_base64.into(),
            target: "windows-x64".to_string(),
            proxy_url: None,
            max_download_bytes: DEFAULT_MAX_DOWNLOAD_BYTES,
        }
    }
}

/// The exact bytes of this JSON are signed separately. Do not add a signature
/// field here: keeping it detached removes JSON canonicalization ambiguity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateManifest {
    pub schema_version: u8,
    pub app_id: String,
    pub channel: String,
    pub version: String,
    pub published_at: String,
    pub target: String,
    pub asset: UpdateAsset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAsset {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCandidate {
    pub manifest: UpdateManifest,
}

impl UpdateCandidate {
    pub fn version(&self) -> &str {
        &self.manifest.version
    }

    pub fn notes_url(&self) -> Option<&str> {
        self.manifest.notes_url.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    UpToDate,
    UpdateAvailable(UpdateCandidate),
}

/// Checks a detached, signed JSON manifest. The caller should run this in its
/// own background task because this API deliberately uses blocking I/O.
pub fn check(config: &UpdateConfig) -> Result<CheckResult> {
    let client = build_http_client(config)?;
    let manifest_bytes = fetch_small(&client, &cache_busted_url(&config.manifest_url), 512 * 1024)?;
    let signature_bytes =
        fetch_small(&client, &cache_busted_url(&config.signature_url), 16 * 1024)?;
    let manifest = verify_signed_manifest(config, &manifest_bytes, &signature_bytes)?;
    let current = Version::parse(&config.current_version)
        .map_err(|error| UpdateError::new(format!("Invalid current app version: {error}")))?;
    let candidate = Version::parse(&manifest.version)
        .map_err(|error| UpdateError::new(format!("Invalid update version: {error}")))?;

    if candidate > current {
        Ok(CheckResult::UpdateAvailable(UpdateCandidate { manifest }))
    } else {
        Ok(CheckResult::UpToDate)
    }
}

/// Verifies a manifest obtained by another transport. This is useful for apps
/// that already own an HTTP stack or for deterministic tests.
pub fn verify_signed_manifest(
    config: &UpdateConfig,
    manifest_bytes: &[u8],
    signature_bytes: &[u8],
) -> Result<UpdateManifest> {
    let public_key = decode_public_key(&config.public_key_base64)?;
    let signature_text = std::str::from_utf8(signature_bytes)
        .map_err(|_| UpdateError::new("Update signature is not UTF-8"))?
        .trim();
    let signature = STANDARD
        .decode(signature_text)
        .map_err(|_| UpdateError::new("Update signature is not valid Base64"))?;
    let signature = Signature::from_slice(&signature)
        .map_err(|_| UpdateError::new("Update signature has an invalid length"))?;
    public_key
        .verify_strict(manifest_bytes, &signature)
        .map_err(|_| UpdateError::new("Update manifest signature verification failed"))?;

    let manifest: UpdateManifest = serde_json::from_slice(manifest_bytes)
        .map_err(|error| UpdateError::new(format!("Invalid update manifest JSON: {error}")))?;
    validate_manifest(config, &manifest)?;
    Ok(manifest)
}

/// A fully downloaded archive. Its hash and size have already been verified.
#[derive(Debug, Clone)]
pub struct DownloadedUpdate {
    pub candidate: UpdateCandidate,
    pub package_path: PathBuf,
}

/// Downloads an available update to `updates_dir`. Partial downloads are never
/// returned and are removed after a failure.
pub fn download(
    config: &UpdateConfig,
    candidate: UpdateCandidate,
    updates_dir: &Path,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<DownloadedUpdate> {
    validate_manifest(config, &candidate.manifest)?;
    fs::create_dir_all(updates_dir).map_err(io_error("Unable to create update directory"))?;

    let package_name = format!(
        "{}-{}.zip",
        sanitize_file_component(&candidate.manifest.version),
        &candidate.manifest.asset.sha256[..16]
    );
    let package_path = updates_dir.join(&package_name);
    if package_path.exists()
        && verify_file_hash_and_size(
            &package_path,
            &candidate.manifest.asset.sha256,
            candidate.manifest.asset.size,
        )?
    {
        on_progress(candidate.manifest.asset.size, candidate.manifest.asset.size);
        return Ok(DownloadedUpdate {
            candidate,
            package_path,
        });
    }
    if package_path.exists() {
        fs::remove_file(&package_path)
            .map_err(io_error("Unable to discard invalid cached update"))?;
    }

    let temporary_path = updates_dir.join(format!(".{package_name}.part-{}", unique_suffix()));
    let download_result = download_to_path(
        config,
        &candidate.manifest.asset,
        &temporary_path,
        &mut on_progress,
    );
    if let Err(error) = download_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    fs::rename(&temporary_path, &package_path)
        .map_err(io_error("Unable to finalize downloaded update"))?;
    Ok(DownloadedUpdate {
        candidate,
        package_path,
    })
}

/// Describes exactly what an update archive is permitted to replace. The
/// allow-list is an important second line of defence in addition to signature
/// and hash validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortableLayout {
    /// Optional single directory stored at the root of the ZIP.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_root: Option<String>,
    pub replace_files: Vec<String>,
    /// Files required in the release archive but deliberately left untouched in
    /// the installation. A portable-mode marker is a common example.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preserve_files: Vec<String>,
    #[serde(default = "default_max_extract_bytes")]
    pub max_extract_bytes: u64,
}

impl PortableLayout {
    pub fn flat(replace_files: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            archive_root: None,
            replace_files: replace_files.into_iter().map(Into::into).collect(),
            preserve_files: Vec::new(),
            max_extract_bytes: DEFAULT_MAX_EXTRACT_BYTES,
        }
    }

    pub fn with_preserved_files(
        mut self,
        preserve_files: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.preserve_files = preserve_files.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, Clone)]
pub struct ApplyRequest {
    /// The updater executable that ships beside the application. It is copied
    /// before launch so the archive may safely replace it.
    pub helper_path: PathBuf,
    pub install_dir: PathBuf,
    pub restart_executable: PathBuf,
    pub layout: PortableLayout,
}

#[derive(Debug, Clone)]
pub struct PendingUpdate {
    pub version: String,
    pub plan_path: PathBuf,
}

/// Starts a temporary updater helper. Callers should exit their main process
/// after this succeeds. The returned helper owns replacement and restart.
pub fn apply_and_restart(
    downloaded: &DownloadedUpdate,
    request: &ApplyRequest,
) -> Result<PendingUpdate> {
    validate_layout(&request.layout)?;
    if !downloaded.package_path.is_file() {
        return Err(UpdateError::new(
            "Downloaded update package no longer exists",
        ));
    }
    if !request.helper_path.is_file() {
        return Err(UpdateError::new(format!(
            "Updater helper was not found: {}",
            request.helper_path.display()
        )));
    }
    if !request.install_dir.is_dir() || !request.restart_executable.is_file() {
        return Err(UpdateError::new(
            "Portable installation directory is invalid",
        ));
    }

    let updates_dir = downloaded
        .package_path
        .parent()
        .ok_or_else(|| UpdateError::new("Downloaded update has no parent directory"))?
        .to_path_buf();
    let id = unique_suffix();
    let copied_helper = updates_dir.join(format!("helper-{id}.exe"));
    fs::copy(&request.helper_path, &copied_helper)
        .map_err(io_error("Unable to prepare updater helper"))?;

    let plan = ApplyPlan {
        schema_version: 1,
        id: id.clone(),
        parent_pid: std::process::id(),
        package_path: absolute_path(&downloaded.package_path)?,
        install_dir: absolute_path(&request.install_dir)?,
        restart_executable: absolute_path(&request.restart_executable)?,
        layout: request.layout.clone(),
        ack_path: updates_dir.join(format!("ack-{id}")),
        staging_dir: updates_dir.join(format!("staging-{id}")),
        backup_dir: updates_dir.join(format!("backup-{id}")),
        journal_path: updates_dir.join(format!("journal-{id}.json")),
    };
    let plan_path = updates_dir.join(format!("plan-{id}.json"));
    write_json(&plan_path, &plan, "Unable to write update plan")?;

    let mut command = Command::new(&copied_helper);
    command.arg("--desktop-updater-apply-plan").arg(&plan_path);
    hide_command_window(&mut command);
    if let Err(error) = command.spawn() {
        let _ = fs::remove_file(&plan_path);
        let _ = fs::remove_file(&copied_helper);
        return Err(UpdateError::new(format!(
            "Unable to start updater helper: {error}"
        )));
    }

    Ok(PendingUpdate {
        version: downloaded.candidate.manifest.version.clone(),
        plan_path,
    })
}

/// Entry point used by an application-owned helper binary:
///
/// ```no_run
/// fn main() {
///     if let Err(error) = desktop_updater::run_helper_from_args() {
///         eprintln!("{error}");
///         std::process::exit(1);
///     }
/// }
/// ```
pub fn run_helper_from_args() -> Result<()> {
    let mut args = std::env::args_os();
    let _program = args.next();
    let mode = args
        .next()
        .ok_or_else(|| UpdateError::new("Missing updater helper mode"))?;
    if mode != "--desktop-updater-apply-plan" {
        return Err(UpdateError::new("Unsupported updater helper mode"));
    }
    let plan_path = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| UpdateError::new("Missing updater helper plan path"))?;
    if args.next().is_some() {
        return Err(UpdateError::new("Unexpected updater helper arguments"));
    }
    let bytes = fs::read(&plan_path).map_err(io_error("Unable to read update plan"))?;
    let plan: ApplyPlan = serde_json::from_slice(&bytes)
        .map_err(|error| UpdateError::new(format!("Invalid update plan: {error}")))?;
    validate_plan(&plan)?;
    apply_plan(&plan, &plan_path)
}

/// The restarted application must call this as early as practical in its setup
/// path. The helper treats this acknowledgement as proof that it can discard
/// rollback material. It is a no-op during ordinary launches.
pub fn acknowledge_if_requested() -> Result<bool> {
    let Ok(path) = std::env::var(HELPER_ACK_ENV) else {
        return Ok(false);
    };
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error(
            "Unable to create updater acknowledgement directory",
        ))?;
    }
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(b"ok\n")
                .map_err(io_error("Unable to write updater acknowledgement"))?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(true),
        Err(error) => Err(UpdateError::new(format!(
            "Unable to acknowledge applied update: {error}"
        ))),
    }
}

fn apply_plan(plan: &ApplyPlan, plan_path: &Path) -> Result<()> {
    wait_for_process_exit(plan.parent_pid, HELPER_WAIT_TIMEOUT)?;
    let mut journal = UpdateJournal {
        schema_version: 1,
        plan: plan.clone(),
        completed: Vec::new(),
    };
    write_json(
        &plan.journal_path,
        &journal,
        "Unable to write update journal",
    )?;
    let mut rollback_completed_in_branch = false;

    let result = (|| {
        extract_archive(plan)?;
        for relative in &plan.layout.replace_files {
            replace_one_file(plan, relative)?;
            journal.completed.push(relative.clone());
            write_json(
                &plan.journal_path,
                &journal,
                "Unable to update update journal",
            )?;
        }
        let mut child = restart_updated_application(plan)?;
        if wait_for_acknowledgement(plan, &mut child)? {
            cleanup_success(plan, plan_path);
            return Ok(());
        }
        let _ = child.kill();
        let _ = child.wait();
        rollback_completed(plan, &journal.completed)?;
        rollback_completed_in_branch = true;
        let _ = restart_original_application(plan);
        Err(UpdateError::new(
            "Updated application did not acknowledge startup; restored the previous version",
        ))
    })();

    if result.is_err() && !rollback_completed_in_branch {
        let _ = rollback_completed(plan, &journal.completed);
    }
    result
}

/// Verifies that a portable ZIP exactly matches its declared layout without
/// extracting it. Release tooling should call this before publishing metadata.
pub fn validate_release_archive(package_path: &Path, layout: &PortableLayout) -> Result<()> {
    validate_layout(layout)?;
    let file = File::open(package_path).map_err(io_error("Unable to open update package"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| UpdateError::new(format!("Invalid update ZIP archive: {error}")))?;
    let expected = layout_files(layout);
    let mut extracted = HashSet::new();
    let mut total_size = 0_u64;

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            UpdateError::new(format!("Unable to read update ZIP entry: {error}"))
        })?;
        let relative = archive_relative_path(entry.name(), layout.archive_root.as_deref())?;
        if relative.is_empty() {
            continue;
        }
        if entry.is_dir() {
            continue;
        }
        if !expected.contains(relative.as_str()) {
            return Err(UpdateError::new(format!(
                "Update ZIP contains an undeclared file: {relative}"
            )));
        }
        if !extracted.insert(relative.clone()) {
            return Err(UpdateError::new(format!(
                "Update ZIP contains duplicate file: {relative}"
            )));
        }
        total_size = total_size
            .checked_add(entry.size())
            .ok_or_else(|| UpdateError::new("Update ZIP exceeds extraction size limit"))?;
        if total_size > layout.max_extract_bytes {
            return Err(UpdateError::new("Update ZIP exceeds extraction size limit"));
        }
    }
    for relative in expected {
        if !extracted.contains(relative) {
            return Err(UpdateError::new(format!(
                "Update ZIP is missing declared file: {relative}"
            )));
        }
    }
    Ok(())
}

fn extract_archive(plan: &ApplyPlan) -> Result<()> {
    validate_release_archive(&plan.package_path, &plan.layout)?;
    if plan.staging_dir.exists() {
        fs::remove_dir_all(&plan.staging_dir)
            .map_err(io_error("Unable to clear update staging directory"))?;
    }
    fs::create_dir_all(&plan.staging_dir)
        .map_err(io_error("Unable to create update staging directory"))?;
    let file = File::open(&plan.package_path).map_err(io_error("Unable to open update package"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| UpdateError::new(format!("Invalid update ZIP archive: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            UpdateError::new(format!("Unable to read update ZIP entry: {error}"))
        })?;
        let relative = archive_relative_path(entry.name(), plan.layout.archive_root.as_deref())?;
        if relative.is_empty() || entry.is_dir() {
            continue;
        }
        let destination = plan.staging_dir.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(io_error("Unable to create staging subdirectory"))?;
        }
        let mut output =
            File::create(&destination).map_err(io_error("Unable to extract update file"))?;
        io::copy(&mut entry, &mut output).map_err(io_error("Unable to extract update file"))?;
    }
    Ok(())
}

fn layout_files(layout: &PortableLayout) -> HashSet<&str> {
    layout
        .replace_files
        .iter()
        .chain(layout.preserve_files.iter())
        .map(String::as_str)
        .collect()
}

fn replace_one_file(plan: &ApplyPlan, relative: &str) -> Result<()> {
    let relative_path = safe_relative_path(relative)?;
    let source = plan.staging_dir.join(&relative_path);
    let destination = plan.install_dir.join(&relative_path);
    let backup = plan.backup_dir.join(&relative_path);
    if !source.is_file() {
        return Err(UpdateError::new(format!(
            "Staged update file is missing: {relative}"
        )));
    }
    if destination.exists() {
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent)
                .map_err(io_error("Unable to create update backup directory"))?;
        }
        rename_with_retry(&destination, &backup, "Unable to back up installed file")?;
    } else if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(io_error("Unable to create install subdirectory"))?;
    }
    if let Err(error) = rename_with_retry(&source, &destination, "Unable to replace installed file")
    {
        if backup.exists() {
            let _ = rename_with_retry(&backup, &destination, "Unable to restore installed file");
        }
        return Err(error);
    }
    Ok(())
}

fn rollback_completed(plan: &ApplyPlan, completed: &[String]) -> Result<()> {
    let mut rollback_error = None;
    for relative in completed.iter().rev() {
        let relative_path = safe_relative_path(relative)?;
        let destination = plan.install_dir.join(&relative_path);
        let backup = plan.backup_dir.join(&relative_path);
        if destination.exists() {
            if let Err(error) =
                remove_file_with_retry(&destination, "Unable to remove failed update file")
            {
                rollback_error.get_or_insert(error);
                continue;
            }
        }
        if backup.exists() {
            if let Err(error) =
                rename_with_retry(&backup, &destination, "Unable to restore backup file")
            {
                rollback_error.get_or_insert(error);
            }
        }
    }
    match rollback_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn restart_updated_application(plan: &ApplyPlan) -> Result<Child> {
    let mut command = Command::new(&plan.restart_executable);
    command.env(HELPER_ACK_ENV, &plan.ack_path);
    command.current_dir(&plan.install_dir);
    hide_command_window(&mut command);
    command.spawn().map_err(|error| {
        UpdateError::new(format!("Unable to restart updated application: {error}"))
    })
}

fn restart_original_application(plan: &ApplyPlan) -> Result<()> {
    let mut command = Command::new(&plan.restart_executable);
    command.current_dir(&plan.install_dir);
    hide_command_window(&mut command);
    command.spawn().map(|_| ()).map_err(|error| {
        UpdateError::new(format!("Unable to restart restored application: {error}"))
    })
}

fn wait_for_acknowledgement(plan: &ApplyPlan, child: &mut Child) -> Result<bool> {
    let deadline = SystemTime::now() + HELPER_WAIT_TIMEOUT;
    loop {
        if plan.ack_path.exists() {
            return Ok(true);
        }
        if child
            .try_wait()
            .map_err(io_error("Unable to inspect restarted application"))?
            .is_some()
        {
            return Ok(false);
        }
        if SystemTime::now() >= deadline {
            return Ok(false);
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn cleanup_success(plan: &ApplyPlan, plan_path: &Path) {
    let _ = fs::remove_file(&plan.ack_path);
    let _ = fs::remove_file(&plan.journal_path);
    let _ = fs::remove_file(plan_path);
    let _ = fs::remove_dir_all(&plan.staging_dir);
    let _ = fs::remove_dir_all(&plan.backup_dir);
    let _ = fs::remove_file(&plan.package_path);
}

fn build_http_client(config: &UpdateConfig) -> Result<Client> {
    let mut builder = Client::builder().timeout(Duration::from_secs(30));
    if let Some(proxy_url) = config
        .proxy_url
        .as_deref()
        .filter(|url| !url.trim().is_empty())
    {
        let proxy = Proxy::all(proxy_url)
            .map_err(|error| UpdateError::new(format!("Invalid update proxy: {error}")))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|error| UpdateError::new(format!("Unable to create update HTTP client: {error}")))
}

fn fetch_small(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>> {
    require_https_url(url, "Update endpoint")?;
    let response = client
        .get(url)
        .header(CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|error| UpdateError::new(format!("Unable to fetch update metadata: {error}")))?
        .error_for_status()
        .map_err(|error| UpdateError::new(format!("Update metadata request failed: {error}")))?;
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(UpdateError::new("Update metadata response is too large"));
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error("Unable to read update metadata"))?;
    if bytes.len() as u64 > limit {
        return Err(UpdateError::new("Update metadata response is too large"));
    }
    Ok(bytes)
}

fn download_to_path(
    config: &UpdateConfig,
    asset: &UpdateAsset,
    temporary_path: &Path,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<()> {
    require_https_url(&asset.url, "Update asset URL")?;
    let client = build_http_client(config)?;
    let mut response = client
        .get(&asset.url)
        .header(CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|error| UpdateError::new(format!("Unable to download update: {error}")))?
        .error_for_status()
        .map_err(|error| UpdateError::new(format!("Update download request failed: {error}")))?;
    if response
        .content_length()
        .is_some_and(|length| length != asset.size)
    {
        return Err(UpdateError::new(
            "Update download size does not match manifest",
        ));
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary_path)
        .map_err(io_error("Unable to create temporary update package"))?;
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = response
            .read(&mut buffer)
            .map_err(io_error("Unable to read update download"))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| UpdateError::new("Update download exceeds declared size"))?;
        if total > asset.size || total > config.max_download_bytes {
            return Err(UpdateError::new("Update download exceeds allowed size"));
        }
        output
            .write_all(&buffer[..count])
            .map_err(io_error("Unable to write update package"))?;
        hasher.update(&buffer[..count]);
        on_progress(total, asset.size);
    }
    output
        .sync_all()
        .map_err(io_error("Unable to finalize update package"))?;
    if total != asset.size {
        return Err(UpdateError::new(
            "Update download size does not match manifest",
        ));
    }
    if hex_lower(hasher.finalize().as_slice()) != asset.sha256 {
        return Err(UpdateError::new(
            "Update package SHA-256 verification failed",
        ));
    }
    Ok(())
}

fn validate_manifest(config: &UpdateConfig, manifest: &UpdateManifest) -> Result<()> {
    if manifest.schema_version != UPDATE_MANIFEST_SCHEMA_VERSION {
        return Err(UpdateError::new(format!(
            "Unsupported update manifest schema version: {}",
            manifest.schema_version
        )));
    }
    if manifest.app_id != config.app_id
        || manifest.channel != config.channel
        || manifest.target != config.target
    {
        return Err(UpdateError::new(
            "Update manifest is for a different application, channel, or target",
        ));
    }
    Version::parse(&manifest.version)
        .map_err(|error| UpdateError::new(format!("Invalid update version: {error}")))?;
    require_https_url(&manifest.asset.url, "Update asset URL")?;
    if let Some(notes_url) = &manifest.notes_url {
        require_https_url(notes_url, "Update notes URL")?;
    }
    if manifest.asset.size == 0 || manifest.asset.size > config.max_download_bytes {
        return Err(UpdateError::new(
            "Update package size is outside allowed limits",
        ));
    }
    if !is_lowercase_sha256(&manifest.asset.sha256) {
        return Err(UpdateError::new(
            "Update manifest SHA-256 must be 64 lowercase hexadecimal characters",
        ));
    }
    Ok(())
}

fn decode_public_key(base64_key: &str) -> Result<VerifyingKey> {
    let bytes = STANDARD
        .decode(base64_key.trim())
        .map_err(|_| UpdateError::new("Update public key is not valid Base64"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| UpdateError::new("Update public key must contain 32 bytes"))?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| UpdateError::new("Update public key is invalid"))
}

fn validate_layout(layout: &PortableLayout) -> Result<()> {
    if layout.replace_files.is_empty() {
        return Err(UpdateError::new(
            "Portable update layout must declare replacement files",
        ));
    }
    if layout.max_extract_bytes == 0 {
        return Err(UpdateError::new(
            "Portable update extraction limit must be positive",
        ));
    }
    if let Some(root) = &layout.archive_root {
        let root = root.trim_matches('/');
        if root.is_empty() || root.contains('\\') || safe_relative_path(root).is_err() {
            return Err(UpdateError::new("Portable update archive root is unsafe"));
        }
    }
    let mut seen = HashSet::new();
    for file in layout
        .replace_files
        .iter()
        .chain(layout.preserve_files.iter())
    {
        let normalized = safe_relative_path(file)?;
        if normalized.to_string_lossy() != file.replace('/', "\\") {
            return Err(UpdateError::new(format!(
                "Portable update file path is not normalized: {file}"
            )));
        }
        if !seen.insert(file) {
            return Err(UpdateError::new(format!(
                "Portable update file path is duplicated: {file}"
            )));
        }
    }
    Ok(())
}

fn validate_plan(plan: &ApplyPlan) -> Result<()> {
    if plan.schema_version != 1 || plan.parent_pid == 0 || plan.id.is_empty() {
        return Err(UpdateError::new("Invalid update plan metadata"));
    }
    validate_layout(&plan.layout)?;
    if !plan.package_path.is_absolute()
        || !plan.install_dir.is_absolute()
        || !plan.restart_executable.is_absolute()
        || !plan.ack_path.is_absolute()
        || !plan.staging_dir.is_absolute()
        || !plan.backup_dir.is_absolute()
        || !plan.journal_path.is_absolute()
    {
        return Err(UpdateError::new("Update plan paths must be absolute"));
    }
    Ok(())
}

fn archive_relative_path(name: &str, archive_root: Option<&str>) -> Result<String> {
    if name.contains('\\') || name.contains('\0') {
        return Err(UpdateError::new("Update ZIP contains an unsafe path"));
    }
    let name = name.trim_end_matches('/');
    let name = match archive_root {
        Some(root) => {
            let root = root.trim_matches('/');
            if name == root {
                return Ok(String::new());
            }
            name.strip_prefix(&format!("{root}/")).ok_or_else(|| {
                UpdateError::new("Update ZIP entry is outside configured archive root")
            })?
        }
        None => name,
    };
    if name.is_empty() {
        return Ok(String::new());
    }
    let path = safe_relative_path(name)?;
    Ok(path.to_string_lossy().replace('\\', "/"))
}

fn safe_relative_path(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() || value.contains('\0') {
        return Err(UpdateError::new("Update file path is unsafe"));
    }
    let mut normal_count = 0;
    for component in path.components() {
        match component {
            Component::Normal(segment) if !segment.is_empty() => normal_count += 1,
            _ => return Err(UpdateError::new("Update file path is unsafe")),
        }
    }
    if normal_count == 0 {
        return Err(UpdateError::new("Update file path is unsafe"));
    }
    Ok(path.to_path_buf())
}

fn verify_file_hash_and_size(path: &Path, expected_hash: &str, expected_size: u64) -> Result<bool> {
    let metadata = fs::metadata(path).map_err(io_error("Unable to inspect cached update"))?;
    if metadata.len() != expected_size {
        return Ok(false);
    }
    let mut file = File::open(path).map_err(io_error("Unable to read cached update"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(io_error("Unable to read cached update"))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex_lower(hasher.finalize().as_slice()) == expected_hash)
}

fn rename_with_retry(source: &Path, destination: &Path, context: &str) -> Result<()> {
    let deadline = SystemTime::now() + Duration::from_secs(20);
    loop {
        match fs::rename(source, destination) {
            Ok(()) => return Ok(()),
            Err(error) if SystemTime::now() < deadline => {
                let _ = error;
                thread::sleep(Duration::from_millis(250));
            }
            Err(error) => return Err(UpdateError::new(format!("{context}: {error}"))),
        }
    }
}

fn remove_file_with_retry(path: &Path, context: &str) -> Result<()> {
    let deadline = SystemTime::now() + Duration::from_secs(20);
    loop {
        match fs::remove_file(path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) if SystemTime::now() < deadline => {
                let _ = error;
                thread::sleep(Duration::from_millis(250));
            }
            Err(error) => return Err(UpdateError::new(format!("{context}: {error}"))),
        }
    }
}

fn write_json(path: &Path, value: &impl Serialize, context: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error(context))?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| UpdateError::new(format!("{context}: {error}")))?;
    fs::write(path, bytes).map_err(io_error(context))
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|error| UpdateError::new(format!("Unable to resolve {}: {error}", path.display())))
}

fn require_https_url(value: &str, label: &str) -> Result<()> {
    let normalized = value.trim();
    if !normalized.starts_with("https://") || normalized.chars().any(char::is_whitespace) {
        return Err(UpdateError::new(format!("{label} must be an HTTPS URL")));
    }
    Ok(())
}

fn cache_busted_url(value: &str) -> String {
    let bucket = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 3600;
    let separator = if value.contains('?') { '&' } else { '?' };
    format!("{value}{separator}desktop_updater_cache={bucket}")
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn sanitize_file_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '.' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn default_max_extract_bytes() -> u64 {
    DEFAULT_MAX_EXTRACT_BYTES
}

fn io_error(context: impl Into<String>) -> impl FnOnce(io::Error) -> UpdateError {
    let context = context.into();
    move |error| UpdateError::new(format!("{context}: {error}"))
}

fn hide_command_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

#[cfg(windows)]
fn wait_for_process_exit(pid: u32, timeout: Duration) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };
    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    let handle = unsafe { OpenProcess(SYNCHRONIZE_ACCESS, 0, pid) };
    if handle.is_null() {
        return Ok(());
    }
    let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
    let result = unsafe { WaitForSingleObject(handle, timeout_ms) };
    unsafe { CloseHandle(handle) };
    match result {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => Err(UpdateError::new(
            "Timed out waiting for application to exit before update",
        )),
        _ => Err(UpdateError::new(
            "Unable to wait for application process before update",
        )),
    }
}

#[cfg(not(windows))]
fn wait_for_process_exit(_pid: u32, _timeout: Duration) -> Result<()> {
    Err(UpdateError::new(
        "Portable apply is currently supported on Windows only",
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyPlan {
    schema_version: u8,
    id: String,
    parent_pid: u32,
    package_path: PathBuf,
    install_dir: PathBuf,
    restart_executable: PathBuf,
    layout: PortableLayout,
    ack_path: PathBuf,
    staging_dir: PathBuf,
    backup_dir: PathBuf,
    journal_path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateJournal {
    schema_version: u8,
    plan: ApplyPlan,
    completed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::Write;
    use zip::{write::FileOptions, ZipWriter};

    fn test_config(public_key_base64: String) -> UpdateConfig {
        UpdateConfig::new(
            "com.example.test",
            "stable",
            "1.0.0",
            "https://example.test/stable.json",
            "https://example.test/stable.json.sig",
            public_key_base64,
        )
    }

    fn manifest() -> UpdateManifest {
        UpdateManifest {
            schema_version: 1,
            app_id: "com.example.test".to_string(),
            channel: "stable".to_string(),
            version: "1.0.1".to_string(),
            published_at: "2026-07-21T00:00:00Z".to_string(),
            target: "windows-x64".to_string(),
            asset: UpdateAsset {
                url: "https://example.test/release.zip".to_string(),
                sha256: "a".repeat(64),
                size: 123,
            },
            notes_url: None,
        }
    }

    #[test]
    fn signed_manifest_requires_matching_key_and_product() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key = STANDARD.encode(signing_key.verifying_key().as_bytes());
        let manifest_bytes = serde_json::to_vec(&manifest()).expect("manifest JSON");
        let signature = STANDARD.encode(signing_key.sign(&manifest_bytes).to_bytes());
        let config = test_config(public_key);

        let parsed = verify_signed_manifest(&config, &manifest_bytes, signature.as_bytes())
            .expect("signature verifies");
        assert_eq!(parsed.version, "1.0.1");

        let mut changed = manifest_bytes.clone();
        changed[0] = b'[';
        assert!(verify_signed_manifest(&config, &changed, signature.as_bytes()).is_err());
    }

    #[test]
    fn layout_rejects_unsafe_and_duplicate_paths() {
        assert!(validate_layout(&PortableLayout::flat(["app.exe", "app.exe"])).is_err());
        assert!(validate_layout(&PortableLayout::flat(["../app.exe"])).is_err());
        assert!(validate_layout(&PortableLayout::flat(["C:\\app.exe"])).is_err());
        assert!(validate_layout(&PortableLayout::flat(["app.exe", "bin/helper.exe"])).is_ok());
    }

    #[test]
    fn archive_extraction_rejects_paths_outside_allow_list() {
        let temp = tempfile::tempdir().expect("tempdir");
        let package = temp.path().join("package.zip");
        let file = File::create(&package).expect("zip");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("../evil.exe", FileOptions::default())
            .expect("entry");
        writer.write_all(b"evil").expect("data");
        writer.finish().expect("finish");

        let plan = ApplyPlan {
            schema_version: 1,
            id: "test".to_string(),
            parent_pid: 1,
            package_path: package,
            install_dir: temp.path().join("install"),
            restart_executable: temp.path().join("install/app.exe"),
            layout: PortableLayout::flat(["app.exe"]),
            ack_path: temp.path().join("ack"),
            staging_dir: temp.path().join("stage"),
            backup_dir: temp.path().join("backup"),
            journal_path: temp.path().join("journal.json"),
        };
        assert!(extract_archive(&plan).is_err());
        assert!(!temp.path().join("evil.exe").exists());
    }

    #[test]
    fn archive_validation_allows_a_preserved_portable_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let package = temp.path().join("package.zip");
        let file = File::create(&package).expect("zip");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("app.exe", FileOptions::default())
            .expect("app entry");
        writer.write_all(b"app").expect("app data");
        writer
            .start_file("portable.marker", FileOptions::default())
            .expect("marker entry");
        writer.write_all(b"marker").expect("marker data");
        writer.finish().expect("finish");

        let layout = PortableLayout::flat(["app.exe"]).with_preserved_files(["portable.marker"]);
        validate_release_archive(&package, &layout).expect("archive validates");
        assert_eq!(layout.replace_files, ["app.exe"]);
        assert_eq!(layout.preserve_files, ["portable.marker"]);
    }

    #[test]
    fn sha256_validation_is_strictly_lowercase() {
        assert!(is_lowercase_sha256(&"f".repeat(64)));
        assert!(!is_lowercase_sha256(&"F".repeat(64)));
        assert!(!is_lowercase_sha256("abc"));
    }

    #[test]
    fn rollback_restores_the_previous_file_without_touching_user_data() {
        let temp = tempfile::tempdir().expect("tempdir");
        let install = temp.path().join("install");
        let backup = temp.path().join("backup");
        fs::create_dir_all(&install).expect("install dir");
        fs::create_dir_all(&backup).expect("backup dir");
        fs::write(install.join("app.exe"), b"new").expect("new app");
        fs::write(backup.join("app.exe"), b"old").expect("old app");
        fs::write(install.join("config.json"), b"user config").expect("config");
        let plan = ApplyPlan {
            schema_version: 1,
            id: "rollback".to_string(),
            parent_pid: 1,
            package_path: temp.path().join("package.zip"),
            install_dir: install.clone(),
            restart_executable: install.join("app.exe"),
            layout: PortableLayout::flat(["app.exe"]),
            ack_path: temp.path().join("ack"),
            staging_dir: temp.path().join("stage"),
            backup_dir: backup,
            journal_path: temp.path().join("journal.json"),
        };

        rollback_completed(&plan, &["app.exe".to_string()]).expect("rollback");
        assert_eq!(
            fs::read(install.join("app.exe")).expect("restored app"),
            b"old"
        );
        assert_eq!(
            fs::read(install.join("config.json")).expect("config"),
            b"user config"
        );
    }
}
