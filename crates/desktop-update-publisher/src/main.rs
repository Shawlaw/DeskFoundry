use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf, process};

use desktop_updater::{validate_release_archive, PortableLayout};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateManifest {
    schema_version: u8,
    app_id: String,
    channel: String,
    version: String,
    published_at: String,
    target: String,
    asset: UpdateAsset,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateAsset {
    url: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct ProductUpdateLayout {
    app_id: String,
    channel: String,
    #[serde(default)]
    archive_root: String,
    replace_files: Vec<String>,
    #[serde(default)]
    preserve_files: Vec<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("desktop-update-publisher: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(usage)?;
    if command == "keygen" {
        if args.next().is_some() {
            return Err(usage());
        }
        return keygen();
    }
    if command != "create" {
        return Err(usage());
    }
    let options = parse_options(args.collect())?;
    let app_id = required(&options, "app-id")?;
    let channel = required(&options, "channel")?;
    let version = required(&options, "version")?;
    let published_at = required(&options, "published-at")?;
    let target = required(&options, "target")?;
    let asset_path = PathBuf::from(required(&options, "asset-path")?);
    let layout_path = PathBuf::from(required(&options, "layout-path")?);
    let asset_url = required(&options, "asset-url")?;
    let output = PathBuf::from(required(&options, "output")?);
    let signature_output = PathBuf::from(required(&options, "signature-output")?);
    let private_key_env = required(&options, "private-key-env")?;
    let notes_url = options.get("notes-url").cloned();

    require_https(&asset_url, "asset-url")?;
    if let Some(notes_url) = &notes_url {
        require_https(notes_url, "notes-url")?;
    }
    let layout = load_layout(&layout_path, &app_id, &channel)?;
    validate_release_archive(&asset_path, &layout)
        .map_err(|error| format!("Release ZIP does not match update layout: {error}"))?;
    let asset_bytes = fs::read(&asset_path)
        .map_err(|error| format!("Unable to read asset {}: {error}", asset_path.display()))?;
    if asset_bytes.is_empty() {
        return Err("Release asset is empty".to_string());
    }
    let manifest = UpdateManifest {
        schema_version: 1,
        app_id,
        channel,
        version,
        published_at,
        target,
        asset: UpdateAsset {
            url: asset_url,
            sha256: hex_lower(Sha256::digest(&asset_bytes).as_slice()),
            size: asset_bytes.len() as u64,
        },
        notes_url,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("Unable to serialize update manifest: {error}"))?;
    let signing_key = signing_key_from_environment(&private_key_env)?;
    let signature = STANDARD.encode(signing_key.sign(&manifest_bytes).to_bytes());

    write_file(&output, &manifest_bytes)?;
    write_file(&signature_output, format!("{signature}\n").as_bytes())?;
    println!("manifest={}", output.display());
    println!("signature={}", signature_output.display());
    println!(
        "public_key_base64={}",
        STANDARD.encode(signing_key.verifying_key().as_bytes())
    );
    Ok(())
}

fn load_layout(
    path: &PathBuf,
    expected_app_id: &str,
    expected_channel: &str,
) -> Result<PortableLayout, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Unable to read layout {}: {error}", path.display()))?;
    let descriptor: ProductUpdateLayout = toml::from_str(&contents)
        .map_err(|error| format!("Invalid update layout {}: {error}", path.display()))?;
    if descriptor.app_id != expected_app_id || descriptor.channel != expected_channel {
        return Err(
            "Update layout app_id or channel does not match publisher arguments".to_string(),
        );
    }
    Ok(PortableLayout {
        archive_root: (!descriptor.archive_root.trim().is_empty())
            .then_some(descriptor.archive_root),
        replace_files: descriptor.replace_files,
        preserve_files: descriptor.preserve_files,
        max_extract_bytes: desktop_updater::DEFAULT_MAX_EXTRACT_BYTES,
    })
}

fn parse_options(args: Vec<String>) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut options = std::collections::BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let name = args[index]
            .strip_prefix("--")
            .filter(|name| !name.is_empty())
            .ok_or_else(usage)?;
        let value = args.get(index + 1).ok_or_else(usage)?;
        if value.starts_with("--")
            || options
                .insert(name.to_string(), value.to_string())
                .is_some()
        {
            return Err(usage());
        }
        index += 2;
    }
    Ok(options)
}

fn required(
    options: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Result<String, String> {
    options
        .get(name)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(usage)
}

fn signing_key_from_environment(name: &str) -> Result<SigningKey, String> {
    let value =
        env::var(name).map_err(|_| format!("Missing signing-key environment variable {name}"))?;
    let bytes = STANDARD
        .decode(value.trim())
        .map_err(|_| format!("Signing key in {name} is not valid Base64"))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| format!("Signing key in {name} must be a 32-byte Ed25519 seed"))?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn keygen() -> Result<(), String> {
    let mut seed = [0_u8; 32];
    OsRng.fill_bytes(&mut seed);
    let signing_key = SigningKey::from_bytes(&seed);
    println!("private_key_base64={}", STANDARD.encode(seed));
    println!(
        "public_key_base64={}",
        STANDARD.encode(signing_key.verifying_key().as_bytes())
    );
    Ok(())
}

fn write_file(path: &PathBuf, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Unable to create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("Unable to write {}: {error}", path.display()))
}

fn require_https(value: &str, name: &str) -> Result<(), String> {
    if !value.starts_with("https://") || value.chars().any(char::is_whitespace) {
        return Err(format!("{name} must be an HTTPS URL"));
    }
    Ok(())
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

fn usage() -> String {
    "usage: desktop-update-publisher keygen | create --app-id ID --channel stable --version VERSION --published-at RFC3339 --target windows-x64 --asset-path ZIP --layout-path desktop-update.toml --asset-url HTTPS_URL --output updates/stable.json --signature-output updates/stable.json.sig --private-key-env ENV_NAME [--notes-url HTTPS_URL]".to_string()
}
