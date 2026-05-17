use egui::{Context, FontData, FontDefinitions, FontFamily};
use std::{fs, path::PathBuf};

const CJK_FONT_NAME: &str = "desktop-egui-cjk";

pub fn install_cjk_fallback_fonts(ctx: &Context) -> Result<Option<String>, String> {
    let Some(source) = find_system_cjk_font() else {
        return Ok(None);
    };

    let bytes = fs::read(&source.path)
        .map_err(|err| format!("Failed to read font {}: {err}", source.path.display()))?;

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        CJK_FONT_NAME.to_owned(),
        FontData {
            font: bytes.into(),
            index: source.index,
            tweak: Default::default(),
        }
        .into(),
    );

    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, CJK_FONT_NAME.to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.push(CJK_FONT_NAME.to_owned());
    }

    ctx.set_fonts(fonts);
    Ok(Some(source.description))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FontSource {
    path: PathBuf,
    index: u32,
    description: String,
}

fn find_system_cjk_font() -> Option<FontSource> {
    font_candidates()
        .into_iter()
        .find(|candidate| candidate.path.is_file())
}

fn font_candidates() -> Vec<FontSource> {
    #[cfg(target_os = "windows")]
    {
        windows_font_candidates()
    }

    #[cfg(target_os = "macos")]
    {
        macos_font_candidates()
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        linux_font_candidates()
    }
}

#[cfg(target_os = "windows")]
fn windows_font_candidates() -> Vec<FontSource> {
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts_dir = windir.join("Fonts");

    vec![
        ("msyh.ttc", 0, "Microsoft YaHei"),
        ("msyhbd.ttc", 0, "Microsoft YaHei Bold"),
        ("msyh.ttc", 1, "Microsoft YaHei UI"),
        ("simhei.ttf", 0, "SimHei"),
        ("simsun.ttc", 0, "SimSun"),
        ("Deng.ttf", 0, "DengXian"),
    ]
    .into_iter()
    .map(|(name, index, description)| FontSource {
        path: fonts_dir.join(name),
        index,
        description: description.to_owned(),
    })
    .collect()
}

#[cfg(target_os = "macos")]
fn macos_font_candidates() -> Vec<FontSource> {
    vec![
        (
            "/System/Library/Fonts/PingFang.ttc",
            0,
            "PingFang SC".to_owned(),
        ),
        (
            "/System/Library/Fonts/STHeiti Light.ttc",
            0,
            "STHeiti".to_owned(),
        ),
        (
            "/System/Library/Fonts/Hiragino Sans GB.ttc",
            0,
            "Hiragino Sans GB".to_owned(),
        ),
    ]
    .into_iter()
    .map(|(path, index, description)| FontSource {
        path: PathBuf::from(path),
        index,
        description,
    })
    .collect()
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn linux_font_candidates() -> Vec<FontSource> {
    vec![
        (
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            0,
            "Noto Sans CJK".to_owned(),
        ),
        (
            "/usr/share/fonts/opentype/noto/NotoSansCJKSC-Regular.otf",
            0,
            "Noto Sans CJK SC".to_owned(),
        ),
        (
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            0,
            "WenQuanYi Zen Hei".to_owned(),
        ),
        (
            "/usr/share/fonts/truetype/arphic/ukai.ttc",
            0,
            "AR PL UKai".to_owned(),
        ),
    ]
    .into_iter()
    .map(|(path, index, description)| FontSource {
        path: PathBuf::from(path),
        index,
        description,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::font_candidates;

    #[test]
    fn font_candidates_are_defined() {
        assert!(!font_candidates().is_empty());
    }
}
