use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFont {
    pub family: String,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub cjk: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiFontSelection {
    pub requested: String,
    pub effective: String,
    pub missing: bool,
}

static FONT_CACHE: Mutex<Option<Vec<SystemFont>>> = Mutex::new(None);
const BUNDLED_FONT_FAMILY: &str = "Sarasa UI SC";

fn is_bundled_font(value: &str) -> bool {
    value.eq_ignore_ascii_case(BUNDLED_FONT_FAMILY) || value == "更纱黑体 UI SC"
}

pub fn system_fonts() -> Result<Vec<SystemFont>, String> {
    let mut cache = FONT_CACHE.lock().map_err(|_| "字体缓存不可用")?;
    if let Some(fonts) = cache.as_ref() {
        return Ok(fonts.clone());
    }
    let mut fonts = enumerate_fonts()?;
    fonts.sort_by(|a, b| {
        b.cjk
            .cmp(&a.cjk)
            .then_with(|| {
                a.display_name
                    .to_lowercase()
                    .cmp(&b.display_name.to_lowercase())
            })
            .then_with(|| a.family.cmp(&b.family))
    });
    *cache = Some(fonts.clone());
    Ok(fonts)
}

pub fn resolve_selection(requested: String) -> Result<UiFontSelection, String> {
    // Default startup never scans the system font collection.
    if requested.is_empty() {
        return Ok(UiFontSelection {
            requested,
            effective: String::new(),
            missing: false,
        });
    }
    if is_bundled_font(&requested) {
        return Ok(UiFontSelection {
            requested,
            effective: BUNDLED_FONT_FAMILY.into(),
            missing: false,
        });
    }
    let effective = find_font(&system_fonts()?, &requested)
        .map(|font| font.family.clone())
        .unwrap_or_default();
    let missing = effective.is_empty();
    Ok(UiFontSelection {
        requested,
        effective,
        missing,
    })
}

pub fn validate_selection(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if is_bundled_font(value) {
        return Ok(BUNDLED_FONT_FAMILY.into());
    }
    find_font(&system_fonts()?, value)
        .map(|font| font.family.clone())
        .ok_or_else(|| "该字体在当前系统不可用，请重新选择或恢复默认".into())
}

fn find_font<'a>(fonts: &'a [SystemFont], value: &str) -> Option<&'a SystemFont> {
    fonts.iter().find(|font| {
        font.family.to_lowercase() == value.to_lowercase()
            || font
                .aliases
                .iter()
                .any(|name| name.to_lowercase() == value.to_lowercase())
    })
}

#[cfg(windows)]
fn enumerate_fonts() -> Result<Vec<SystemFont>, String> {
    use std::collections::BTreeMap;
    use windows::Win32::Graphics::DirectWrite::*;
    // All COM objects remain on this background thread and are released before returning.
    let read = || -> windows::core::Result<Vec<SystemFont>> {
        unsafe {
            let factory: IDWriteFactory3 = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut collection = None;
            // Exclude downloadable cloud fonts: only actually installed local families.
            factory.GetSystemFontCollection(false, &mut collection, true)?;
            let collection = collection.ok_or_else(|| {
                windows::core::Error::new(
                    windows::core::HRESULT(0x80004005u32 as i32),
                    "系统字体集合为空",
                )
            })?;
            let mut result = BTreeMap::new();
            for index in 0..collection.GetFontFamilyCount() {
                let family = collection.GetFontFamily(index)?;
                let font = family.GetFirstMatchingFont(
                    DWRITE_FONT_WEIGHT_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    DWRITE_FONT_STYLE_NORMAL,
                )?;
                if font.IsSymbolFont().as_bool() {
                    continue;
                }
                let has = |c: char| {
                    font.HasCharacter(c as u32)
                        .map(|value| value.as_bool())
                        .unwrap_or(false)
                };
                let cjk = has('中') || has('あ') || has('한');
                // Pure icon families cannot render ordinary UI text. No family-name whitelist.
                if !cjk && !has('A') && !has('а') && !has('ا') && !has('अ') {
                    continue;
                }
                let names = family.GetFamilyNames()?;
                let mut localized = Vec::new();
                for name_index in 0..names.GetCount() {
                    let mut name = vec![0u16; names.GetStringLength(name_index)? as usize + 1];
                    let mut locale =
                        vec![0u16; names.GetLocaleNameLength(name_index)? as usize + 1];
                    names.GetString(name_index, &mut name)?;
                    names.GetLocaleName(name_index, &mut locale)?;
                    name.pop();
                    locale.pop();
                    let name = String::from_utf16_lossy(&name);
                    if !name.is_empty() {
                        localized.push((String::from_utf16_lossy(&locale).to_lowercase(), name));
                    }
                }
                let Some(first) = localized.first() else {
                    continue;
                };
                let canonical = localized
                    .iter()
                    .find(|(locale, _)| locale == "en-us")
                    .unwrap_or(first)
                    .1
                    .clone();
                let display = localized
                    .iter()
                    .find(|(locale, _)| locale == "zh-cn")
                    .or_else(|| {
                        localized
                            .iter()
                            .find(|(locale, _)| locale.starts_with("zh"))
                    })
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| canonical.clone());
                let mut aliases = localized
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>();
                aliases.sort();
                aliases.dedup();
                result
                    .entry(canonical.to_lowercase())
                    .or_insert(SystemFont {
                        family: canonical,
                        display_name: display,
                        aliases,
                        cjk,
                    });
            }
            Ok(result.into_values().collect())
        }
    };
    read().map_err(|error| format!("无法读取系统字体：{error}"))
}

#[cfg(not(windows))]
fn enumerate_fonts() -> Result<Vec<SystemFont>, String> {
    Err("当前版本的系统字体选择支持 Windows；请使用默认字体".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_font_is_available_without_system_enumeration() {
        for alias in ["Sarasa UI SC", "sarasa ui sc", "更纱黑体 UI SC"] {
            assert_eq!(validate_selection(alias).unwrap(), BUNDLED_FONT_FAMILY);
            let selected = resolve_selection(alias.into()).unwrap();
            assert_eq!(selected.effective, BUNDLED_FONT_FAMILY);
            assert!(!selected.missing);
            assert_eq!(selected.requested, alias);
        }
    }
    #[test]
    fn aliases_and_missing_selection_are_safe() {
        let fonts = vec![SystemFont {
            family: "Example UI".into(),
            display_name: "示例字体".into(),
            aliases: vec!["Example UI".into(), "示例字体".into()],
            cjk: true,
        }];
        assert_eq!(
            find_font(&fonts, "example ui").unwrap().family,
            "Example UI"
        );
        assert!(find_font(&fonts, "示例字体").is_some());
        assert!(find_font(&fonts, "Missing Font").is_none());
        assert_eq!(resolve_selection(String::new()).unwrap().effective, "");
    }
    #[cfg(windows)]
    #[test]
    fn installed_fonts_are_real_cached_and_cjk_first() {
        let started = std::time::Instant::now();
        let fonts = system_fonts().unwrap();
        eprintln!(
            "DirectWrite: {} families in {:?}",
            fonts.len(),
            started.elapsed()
        );
        assert!(!fonts.is_empty());
        assert!(fonts.iter().any(|font| font.cjk));
        assert!(fonts.iter().any(|font| !font.cjk));
        assert!(fonts.windows(2).all(|pair| pair[0].cjk || !pair[1].cjk));
        assert!(fonts
            .iter()
            .all(|font| !font.family.is_empty() && font.aliases.contains(&font.family)));
        assert_eq!(fonts.len(), system_fonts().unwrap().len());
        let font = &fonts[0];
        assert_eq!(validate_selection(&font.family).unwrap(), font.family);
        let missing = resolve_selection("In-Line nonexistent font 93d826".into()).unwrap();
        assert!(missing.missing);
        assert!(missing.effective.is_empty());
    }
}
