use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Catalog {
    strings: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct I18n {
    language: String,
    fallback_language: String,
    catalogs: HashMap<String, HashMap<String, String>>,
}

impl I18n {
    pub fn from_json_catalogs(
        language: &str,
        fallback_language: &str,
        catalogs: &[(&str, &str)],
    ) -> Result<Self, String> {
        let parsed = catalogs
            .iter()
            .map(|(code, json)| {
                parse_catalog(code, json).map(|catalog| ((*code).to_owned(), catalog))
            })
            .collect::<Result<HashMap<_, _>, _>>()?;

        let supported: Vec<&str> = catalogs.iter().map(|(code, _)| *code).collect();
        let fallback = normalize_language_code(fallback_language, &supported, fallback_language);

        Ok(Self {
            language: normalize_language_code(language, &supported, &fallback),
            fallback_language: fallback,
            catalogs: parsed,
        })
    }

    pub fn set_language(&mut self, language: &str) {
        let supported: Vec<&str> = self.catalogs.keys().map(String::as_str).collect();
        self.language = normalize_language_code(language, &supported, &self.fallback_language);
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn tr(&self, key: &str) -> String {
        self.catalogs
            .get(&self.language)
            .and_then(|catalog| catalog.get(key))
            .or_else(|| {
                self.catalogs
                    .get(&self.fallback_language)
                    .and_then(|catalog| catalog.get(key))
            })
            .cloned()
            .unwrap_or_else(|| key.to_owned())
    }

    pub fn tr_args(&self, key: &str, args: &[(&str, String)]) -> String {
        let mut text = self.tr(key);
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

pub fn detect_system_language() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".to_owned())
}

pub fn normalize_language_code(language: &str, supported: &[&str], fallback: &str) -> String {
    if supported.is_empty() {
        return fallback.to_owned();
    }

    let trimmed = language.trim();
    if trimmed.is_empty() {
        return fallback.to_owned();
    }

    if let Some(code) = supported
        .iter()
        .find(|code| code.eq_ignore_ascii_case(trimmed))
    {
        return (*code).to_owned();
    }

    let lower = trimmed.to_ascii_lowercase();

    if lower.starts_with("zh") {
        if let Some(code) = supported
            .iter()
            .find(|code| code.to_ascii_lowercase().starts_with("zh"))
        {
            return (*code).to_owned();
        }
    }

    let base = lower.split(['-', '_']).next().unwrap_or(&lower);
    if let Some(code) = supported
        .iter()
        .find(|code| code.to_ascii_lowercase().split(['-', '_']).next() == Some(base))
    {
        return (*code).to_owned();
    }

    fallback.to_owned()
}

fn parse_catalog(language: &str, json: &str) -> Result<HashMap<String, String>, String> {
    serde_json::from_str::<Catalog>(json)
        .map(|catalog| catalog.strings)
        .map_err(|err| format!("Invalid locale catalog for {language}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{normalize_language_code, I18n};

    const EN: &str = r#"{"strings":{"hello":"Hello","user":"Hello {name}"}}"#;
    const ZH: &str = r#"{"strings":{"hello":"你好"}}"#;

    #[test]
    fn normalization_matches_supported_languages() {
        assert_eq!(
            normalize_language_code("en-US", &["en", "zh-CN"], "en"),
            "en"
        );
        assert_eq!(
            normalize_language_code("zh", &["en", "zh-CN"], "en"),
            "zh-CN"
        );
        assert_eq!(
            normalize_language_code("fr-FR", &["en", "zh-CN"], "en"),
            "en"
        );
    }

    #[test]
    fn translation_uses_fallback_and_args() {
        let i18n = I18n::from_json_catalogs("zh-CN", "en", &[("en", EN), ("zh-CN", ZH)])
            .expect("catalogs");
        assert_eq!(i18n.tr("hello"), "你好");
        assert_eq!(
            i18n.tr_args("user", &[("name", "DeskFoundry".to_owned())]),
            "Hello DeskFoundry"
        );
    }
}
