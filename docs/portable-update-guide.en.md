# Portable Update Specification

[中文版](./portable-update-guide.md)

DeskFoundry's first shared updater supports **Windows x64 portable ZIPs, GitHub Raw manifests, and GitHub Release assets**. MSI, NSIS, macOS, and Linux are intentionally outside version 1.

- The SDK is independent of Tauri, egui, and product UI.
- Raw hosts only a small manifest plus detached Ed25519 signature; binaries are downloaded from GitHub Releases.
- Every product owns a distinct signing key. Keep the private key in a protected GitHub Environment/Secret and compile only its public key into the client.
- An allow-list controls exactly which release files may be replaced; user data and arbitrary files beside the app are preserved.

See the Chinese guide for the full product descriptor, helper-binary contract, GitHub Action example, signing workflow, and required tests. The public API is `check`, `download`, `apply_and_restart`, and `acknowledge_if_requested`.
