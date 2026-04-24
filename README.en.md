# DeskFoundry

DeskFoundry is a Windows-first Rust desktop application monorepo.

It exists to collect the reusable infrastructure extracted from real desktop tools such as **LogcatX** and **clipImg**:

- reusable SDK crates
- release/build workflow patterns
- packaging scripts
- template project structure
- design notes and reuse documentation

## What is inside

### Reusable crates

- [`desktop-logger`](./crates/desktop-logger/README.en.md) — file + console logging, log rotation, panic hook, timestamps
- [`desktop-config`](./crates/desktop-config/README.en.md) — portable/AppData config path strategy, JSON config IO, writable-dir helpers
- [`desktop-i18n`](./crates/desktop-i18n/README.en.md) — lightweight locale catalogs, fallback translation, system locale detection
- [`desktop-fs`](./crates/desktop-fs/README.en.md) — user-facing path normalization, open-path helpers, byte formatting, safe path components

### Template

- [`desktop-app-template`](./templates/desktop-app-template/README.en.md) — a starter structure for new Windows-first desktop apps

### Docs

- [`docs/desktop-app-reuse-guide.md`](./docs/desktop-app-reuse-guide.md) — the comparison and reuse summary for LogcatX / clipImg

## Repository strategy

DeskFoundry is intentionally a **monorepo**:

- each crate has its own folder and README
- each crate is designed to be publishable independently
- apps can reuse crates by:
  1. local `path` dependency during extraction
  2. GitHub `git` dependency during cross-repo validation
  3. crates.io dependency after API stabilization

## Versioning

Current initial SDK versions:

- `desktop-logger` — `0.1.0`
- `desktop-config` — `0.1.0`
- `desktop-i18n` — `0.1.0`
- `desktop-fs` — `0.1.0`

## Local development

```bash
cargo test
```

## License

[MIT](./LICENSE)

