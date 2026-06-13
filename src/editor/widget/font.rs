use iced::Font;
use iced::advanced::text;

pub const EDITOR_TEXT_SHAPING: text::Shaping = text::Shaping::Auto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditorFontRoute {
    pub primary: Font,
    pub cjk_fallback_families: &'static [&'static str],
}

#[cfg(target_os = "windows")]
pub const EDITOR_FONT_ROUTE: EditorFontRoute = EditorFontRoute {
    primary: Font::new("Consolas"),
    cjk_fallback_families: &[
        "Yu Gothic",
        "Malgun Gothic",
        "MingLiU_HKSCS",
        "Microsoft JhengHei UI",
        "Microsoft YaHei UI",
        "Microsoft YaHei",
    ],
};

#[cfg(target_os = "macos")]
pub const EDITOR_FONT_ROUTE: EditorFontRoute = EditorFontRoute {
    primary: Font::new("Menlo"),
    cjk_fallback_families: &[
        "Hiragino Sans",
        "Apple SD Gothic Neo",
        "PingFang HK",
        "PingFang TC",
        "PingFang SC",
    ],
};

#[cfg(all(unix, not(target_os = "macos")))]
pub const EDITOR_FONT_ROUTE: EditorFontRoute = EditorFontRoute {
    primary: Font::MONOSPACE,
    cjk_fallback_families: &[
        "Noto Sans CJK SC",
        "Noto Sans CJK TC",
        "Noto Sans CJK HK",
        "Noto Sans CJK JP",
        "Noto Sans CJK KR",
    ],
};

#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
pub const EDITOR_FONT_ROUTE: EditorFontRoute = EditorFontRoute {
    primary: Font::MONOSPACE,
    cjk_fallback_families: &[],
};

pub const EDITOR_FONT: Font = EDITOR_FONT_ROUTE.primary;
