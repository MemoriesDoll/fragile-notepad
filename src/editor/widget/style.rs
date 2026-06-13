use iced::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorStyle {
    pub surface: Color,
    pub gutter: Color,
    pub text: Color,
    pub line_numbers: Color,
    pub fold_controls: Color,
    pub fold_control_background: Color,
    pub active_line: Color,
    pub selection: Color,
    pub indent_guides: Color,
    pub whitespace_markers: Color,
    pub hidden_line_indicators: Color,
    pub caret: Color,
    pub syntax_fallback_text: Color,
}

impl EditorStyle {
    pub fn from_theme(theme: &Theme) -> Self {
        let palette = theme.palette();
        let is_dark = palette.is_dark;
        let (surface, gutter, text, muted, faint, active_line, selection, guide, fold_background) =
            if is_dark {
                (
                    Color::from_rgb8(22, 25, 30),
                    Color::from_rgb8(28, 32, 38),
                    Color::from_rgb8(235, 238, 242),
                    Color::from_rgb8(177, 184, 194),
                    Color::from_rgb8(130, 140, 154),
                    Color::from_rgba(96.0 / 255.0, 174.0 / 255.0, 255.0 / 255.0, 0.08),
                    Color::from_rgba(96.0 / 255.0, 174.0 / 255.0, 255.0 / 255.0, 0.30),
                    Color::from_rgba(177.0 / 255.0, 184.0 / 255.0, 194.0 / 255.0, 0.18),
                    Color::from_rgb8(38, 43, 51),
                )
            } else {
                (
                    Color::from_rgb8(255, 255, 255),
                    Color::from_rgb8(243, 245, 248),
                    Color::from_rgb8(28, 31, 36),
                    Color::from_rgb8(88, 96, 107),
                    Color::from_rgb8(128, 137, 150),
                    Color::from_rgba(0.0, 103.0 / 255.0, 192.0 / 255.0, 0.06),
                    Color::from_rgba(0.0, 103.0 / 255.0, 192.0 / 255.0, 0.26),
                    Color::from_rgba(88.0 / 255.0, 96.0 / 255.0, 107.0 / 255.0, 0.20),
                    Color::from_rgb8(248, 249, 251),
                )
            };

        Self {
            surface,
            gutter,
            text,
            line_numbers: faint,
            fold_controls: muted,
            fold_control_background: fold_background,
            active_line,
            selection,
            indent_guides: guide,
            whitespace_markers: faint,
            hidden_line_indicators: muted,
            caret: text,
            syntax_fallback_text: text,
        }
    }
}
