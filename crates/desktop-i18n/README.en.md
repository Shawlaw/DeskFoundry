# desktop-i18n

`desktop-i18n` is a lightweight localization helper crate for small Rust desktop applications.

It is intentionally simpler than a full enterprise i18n framework and is designed for cases like:

- embedded JSON locale catalogs
- English fallback
- small desktop utilities with a few supported languages
- simple placeholder replacement

## Version

- current version: **0.1.0**

## API overview

### `I18n::from_json_catalogs`

```rust
pub fn from_json_catalogs(
    language: &str,
    fallback_language: &str,
    catalogs: &[(&str, &str)],
) -> Result<I18n, String>
```

Builds an i18n instance from embedded JSON strings.

Each catalog must follow this shape:

```json
{
  "strings": {
    "app.title": "Example App"
  }
}
```

### `I18n::tr`

```rust
pub fn tr(&self, key: &str) -> String
```

Looks up a string in the current language and falls back to the configured fallback language.

### `I18n::tr_args`

```rust
pub fn tr_args(&self, key: &str, args: &[(&str, String)]) -> String
```

Performs simple placeholder replacement such as:

```text
Hello {name}
```

### `detect_system_language`

```rust
pub fn detect_system_language() -> String
```

Returns the OS locale string when available.

### `normalize_language_code`

```rust
pub fn normalize_language_code(language: &str, supported: &[&str], fallback: &str) -> String
```

Normalizes a locale string against a supported-language list.

Examples:

- `en-US` -> `en`
- `zh` -> `zh-CN`
- unsupported locale -> fallback

## Example

```rust
let i18n = desktop_i18n::I18n::from_json_catalogs(
    "zh-CN",
    "en",
    &[
        ("en", r#"{"strings":{"hello":"Hello"}}"#),
        ("zh-CN", r#"{"strings":{"hello":"你好"}}"#),
    ],
)?;

assert_eq!(i18n.tr("hello"), "你好");
```

## Scope

This crate is best suited to small desktop tools with embedded locale files.

It does **not** try to provide:

- ICU message formatting
- plural rules
- runtime external resource loading conventions
- large-scale translation workflow tooling

