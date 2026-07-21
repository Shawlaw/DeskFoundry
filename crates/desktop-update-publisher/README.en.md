# desktop-update-publisher

`desktop-update-publisher` is the release-side CLI for `desktop-updater`. Given an already-built portable ZIP, it computes SHA-256 and exact size, writes `updates/stable.json`, and creates the detached Base64 Ed25519 signature `updates/stable.json.sig`.

`create --expected-public-key <Base64>` verifies the public key derived from the private key before it writes either update file. The shared GitHub Action requires this check.

```powershell
# Run only in a controlled terminal. The private key belongs only in a GitHub Environment secret.
cargo run -p desktop-update-publisher -- keygen
```

See the [portable-update guide](../../docs/portable-update-guide.md) for arguments, GitHub Action use, and key-management requirements.
