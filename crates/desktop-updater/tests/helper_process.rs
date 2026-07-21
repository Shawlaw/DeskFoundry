#![cfg(windows)]

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use desktop_updater::{
    apply_and_restart_with_parent_pid, ApplyRequest, DownloadedUpdate, PortableLayout, UpdateAsset,
    UpdateCandidate, UpdateManifest, UPDATE_MANIFEST_SCHEMA_VERSION,
};
use zip::{write::FileOptions, ZipWriter};

const MODE_ENV: &str = "DESKTOP_UPDATER_INTEGRATION_MODE";
const PORTABLE_MARKER: &str = "quotabarwin.portable";

#[test]
fn helper_process_replaces_files_acknowledges_and_rolls_back() {
    run_scenario("ack", true);
    run_scenario("no-ack", false);
}

fn run_scenario(mode: &str, should_succeed: bool) {
    let temp = tempfile::tempdir().expect("tempdir");
    let install_dir = temp.path().join("install");
    let updates_dir = temp.path().join("updates");
    fs::create_dir_all(&install_dir).expect("install directory");
    fs::create_dir_all(&updates_dir).expect("updates directory");

    let helper = PathBuf::from(env!("CARGO_BIN_EXE_df-test-helper"));
    let app = PathBuf::from(env!("CARGO_BIN_EXE_df-test-app"));
    let installed_app = install_dir.join("QuotaBarWin.exe");
    let installed_helper = install_dir.join("QuotaBarWin.Updater.exe");
    fs::write(&installed_app, b"old application").expect("old app");
    fs::copy(&helper, &installed_helper).expect("installed helper");
    fs::write(install_dir.join(PORTABLE_MARKER), b"user portable marker").expect("marker");
    fs::write(install_dir.join("config.json"), b"user config").expect("config");

    let package = updates_dir.join(format!("{mode}.zip"));
    write_release_zip(&package, &app, &helper);
    let expected_app = fs::read(&app).expect("fixture app bytes");

    std::env::set_var(MODE_ENV, mode);
    let pending = apply_and_restart_with_parent_pid(
        &downloaded_update(package.clone()),
        &ApplyRequest {
            helper_path: installed_helper,
            install_dir: install_dir.clone(),
            restart_executable: installed_app.clone(),
            layout: release_layout(),
        },
        u32::MAX,
    )
    .expect("start helper");

    if should_succeed {
        wait_until(Duration::from_secs(10), || !pending.plan_path.exists());
        assert_eq!(fs::read(&installed_app).expect("updated app"), expected_app);
        assert!(!package.exists(), "successful update removes package");
    } else {
        wait_until(Duration::from_secs(10), || {
            fs::read(&installed_app).ok().as_deref() == Some(b"old application")
        });
        assert!(
            package.exists(),
            "failed update retains package for diagnostics"
        );
    }

    assert_eq!(
        fs::read(install_dir.join(PORTABLE_MARKER)).expect("portable marker"),
        b"user portable marker"
    );
    assert_eq!(
        fs::read(install_dir.join("config.json")).expect("user config"),
        b"user config"
    );
    std::env::remove_var(MODE_ENV);
}

fn release_layout() -> PortableLayout {
    PortableLayout::flat(["QuotaBarWin.exe", "QuotaBarWin.Updater.exe"])
        .with_preserved_files([PORTABLE_MARKER])
}

fn downloaded_update(package_path: PathBuf) -> DownloadedUpdate {
    DownloadedUpdate {
        candidate: UpdateCandidate {
            manifest: UpdateManifest {
                schema_version: UPDATE_MANIFEST_SCHEMA_VERSION,
                app_id: "com.example.integration".to_string(),
                channel: "test".to_string(),
                version: "1.0.1".to_string(),
                published_at: "2026-07-22T00:00:00Z".to_string(),
                target: "windows-x64".to_string(),
                asset: UpdateAsset {
                    url: "https://example.invalid/update.zip".to_string(),
                    sha256: "f".repeat(64),
                    size: 1,
                },
                notes_url: None,
            },
        },
        package_path,
    }
}

fn write_release_zip(package: &Path, app: &Path, helper: &Path) {
    let file = fs::File::create(package).expect("package");
    let mut writer = ZipWriter::new(file);
    for (name, source) in [
        ("QuotaBarWin.exe", app),
        ("QuotaBarWin.Updater.exe", helper),
    ] {
        writer
            .start_file(name, FileOptions::default())
            .expect("zip entry");
        writer
            .write_all(&fs::read(source).expect("fixture bytes"))
            .expect("zip bytes");
    }
    writer
        .start_file(PORTABLE_MARKER, FileOptions::default())
        .expect("portable marker entry");
    writer.write_all(b"release marker").expect("marker bytes");
    writer.finish().expect("finish package");
}

fn wait_until(timeout: Duration, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        predicate(),
        "helper process did not reach the expected state"
    );
}
