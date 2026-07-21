use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // Windows Installer Detection can classify any executable whose name
    // contains "update" as an installer. The updater crate also produces test
    // executables with that name, so embed an explicit asInvoker manifest for
    // normal, non-elevated execution.
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join("app.manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
}
