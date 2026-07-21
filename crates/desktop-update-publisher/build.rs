use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // `desktop-update-publisher.exe` contains "update" in its file name.
    // Without an explicit manifest, Windows Installer Detection can treat it
    // as an installer and Cargo receives ERROR_ELEVATION_REQUIRED (740) when
    // it tries to launch the ordinary CLI. Explicitly opt into the caller's
    // token instead of relying on Windows' heuristic.
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("app.manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
