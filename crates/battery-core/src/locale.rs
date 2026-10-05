//! Resolve the Windows UI language once; the app and native tray use the same preference.
pub fn resolve(preference: &str) -> &'static str {
    match preference {
        "zh-CN" => "zh-CN",
        "ja" => "ja",
        "en" => "en",
        _ => system_language(),
    }
}

#[cfg(windows)]
fn system_language() -> &'static str {
    use std::sync::OnceLock;
    static LANGUAGE: OnceLock<&'static str> = OnceLock::new();
    LANGUAGE.get_or_init(|| {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
        let mut name = [0_u16; 85];
        // SAFETY: the output buffer is writable for the specified 85 UTF-16 elements.
        let length = unsafe { GetUserDefaultLocaleName(name.as_mut_ptr(), name.len() as i32) };
        if length <= 1 {
            return "en";
        }
        from_tag(&String::from_utf16_lossy(&name[..length as usize - 1]))
    })
}
#[cfg(not(windows))]
fn system_language() -> &'static str {
    "en"
}

pub fn from_tag(tag: &str) -> &'static str {
    let normalized = tag.to_ascii_lowercase();
    if normalized.starts_with("zh") {
        "zh-CN"
    } else if normalized.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

pub fn text<'a>(locale: &str, english: &'a str, chinese: &'a str, japanese: &'a str) -> &'a str {
    match locale {
        "zh-CN" => chinese,
        "ja" => japanese,
        _ => english,
    }
}
