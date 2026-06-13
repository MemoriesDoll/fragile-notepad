use iced::widget::{button, container, text, text_input};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

pub const RADIUS: f32 = 6.0;
const CONTROL_RADIUS: f32 = 5.0;
const TAB_RADIUS: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDragVisual {
    Idle,
    Dragged,
    ValidTarget,
    InvalidTarget,
}

#[derive(Debug, Clone, Copy)]
struct VisualPalette {
    app: Color,
    chrome: Color,
    chrome_high: Color,
    surface: Color,
    surface_low: Color,
    surface_high: Color,
    overlay: Color,
    text: Color,
    muted_text: Color,
    faint_text: Color,
    border: Color,
    border_soft: Color,
    accent: Color,
    accent_soft: Color,
    accent_text: Color,
    success: Color,
    success_soft: Color,
    danger: Color,
    danger_soft: Color,
    shadow: Color,
    selection: Color,
    is_dark: bool,
}

impl VisualPalette {
    fn from_theme(theme: &Theme) -> Self {
        if theme.palette().is_dark {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn light() -> Self {
        Self {
            app: Color::from_rgb8(246, 247, 249),
            chrome: Color::from_rgb8(238, 241, 245),
            chrome_high: Color::from_rgb8(248, 249, 251),
            surface: Color::from_rgb8(255, 255, 255),
            surface_low: Color::from_rgb8(243, 245, 248),
            surface_high: Color::from_rgb8(255, 255, 255),
            overlay: Color::from_rgb8(255, 255, 255),
            text: Color::from_rgb8(28, 31, 36),
            muted_text: Color::from_rgb8(88, 96, 107),
            faint_text: Color::from_rgb8(128, 137, 150),
            border: Color::from_rgb8(196, 204, 216),
            border_soft: Color::from_rgb8(222, 227, 235),
            accent: Color::from_rgb8(0, 103, 192),
            accent_soft: Color::from_rgb8(224, 239, 255),
            accent_text: Color::WHITE,
            success: Color::from_rgb8(27, 128, 79),
            success_soft: Color::from_rgb8(221, 244, 232),
            danger: Color::from_rgb8(190, 45, 65),
            danger_soft: Color::from_rgb8(255, 229, 233),
            shadow: Color::from_rgba(30.0 / 255.0, 41.0 / 255.0, 59.0 / 255.0, 0.16),
            selection: Color::from_rgba(0.0, 103.0 / 255.0, 192.0 / 255.0, 0.26),
            is_dark: false,
        }
    }

    fn dark() -> Self {
        Self {
            app: Color::from_rgb8(25, 28, 33),
            chrome: Color::from_rgb8(31, 35, 41),
            chrome_high: Color::from_rgb8(39, 44, 52),
            surface: Color::from_rgb8(22, 25, 30),
            surface_low: Color::from_rgb8(28, 32, 38),
            surface_high: Color::from_rgb8(43, 49, 58),
            overlay: Color::from_rgb8(38, 43, 51),
            text: Color::from_rgb8(235, 238, 242),
            muted_text: Color::from_rgb8(177, 184, 194),
            faint_text: Color::from_rgb8(130, 140, 154),
            border: Color::from_rgb8(76, 86, 101),
            border_soft: Color::from_rgb8(52, 59, 70),
            accent: Color::from_rgb8(96, 174, 255),
            accent_soft: Color::from_rgb8(34, 62, 94),
            accent_text: Color::from_rgb8(7, 19, 33),
            success: Color::from_rgb8(93, 214, 145),
            success_soft: Color::from_rgb8(31, 71, 51),
            danger: Color::from_rgb8(255, 121, 137),
            danger_soft: Color::from_rgb8(86, 39, 48),
            shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.34),
            selection: Color::from_rgba(96.0 / 255.0, 174.0 / 255.0, 255.0 / 255.0, 0.30),
            is_dark: true,
        }
    }
}

pub fn modern_theme(appearance: crate::core::AppearanceMode) -> Theme {
    match appearance {
        crate::core::AppearanceMode::Dark => Theme::custom(
            "Fragile Modern Dark",
            iced::theme::palette::Seed {
                background: Color::from_rgb8(25, 28, 33),
                text: Color::from_rgb8(235, 238, 242),
                primary: Color::from_rgb8(96, 174, 255),
                success: Color::from_rgb8(93, 214, 145),
                warning: Color::from_rgb8(245, 190, 91),
                danger: Color::from_rgb8(255, 121, 137),
            },
        ),
        crate::core::AppearanceMode::System | crate::core::AppearanceMode::Light => Theme::custom(
            "Fragile Modern Light",
            iced::theme::palette::Seed {
                background: Color::from_rgb8(246, 247, 249),
                text: Color::from_rgb8(28, 31, 36),
                primary: Color::from_rgb8(0, 103, 192),
                success: Color::from_rgb8(27, 128, 79),
                warning: Color::from_rgb8(181, 118, 20),
                danger: Color::from_rgb8(190, 45, 65),
            },
        ),
    }
}

fn border(width: f32, color: Color, radius: f32) -> Border {
    Border {
        radius: radius.into(),
        width,
        color,
    }
}

fn hairline(color: Color) -> Border {
    border(1.0, color, 0.0)
}

fn elevation(palette: VisualPalette, y: f32, blur: f32) -> Shadow {
    Shadow {
        color: palette.shadow,
        offset: Vector::new(0.0, y),
        blur_radius: blur,
    }
}

pub fn app_shell(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.app)),
        text_color: Some(palette.text),
        ..container::Style::default()
    }
}

pub fn menu_bar(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome_high)),
        text_color: Some(palette.text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn tool_bar(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome)),
        text_color: Some(palette.text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn tab_strip(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome)),
        text_color: Some(palette.text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn tab_container(
    is_active: bool,
    drag_visual: TabDragVisual,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| tab_container_style(theme, is_active, drag_visual)
}

pub fn tab_container_style(
    theme: &Theme,
    is_active: bool,
    drag_visual: TabDragVisual,
) -> container::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match drag_visual {
        TabDragVisual::Dragged => palette.accent_soft,
        TabDragVisual::ValidTarget => palette.success_soft,
        TabDragVisual::InvalidTarget => palette.danger_soft,
        TabDragVisual::Idle if is_active => palette.surface,
        TabDragVisual::Idle => palette.chrome,
    };
    let border_color = match drag_visual {
        TabDragVisual::Dragged => palette.accent,
        TabDragVisual::ValidTarget => palette.success,
        TabDragVisual::InvalidTarget => palette.danger,
        TabDragVisual::Idle if is_active => palette.border,
        TabDragVisual::Idle => palette.border_soft,
    };

    container::Style {
        background: Some(Background::Color(background)),
        text_color: Some(palette.text),
        border: Border {
            width: if matches!(drag_visual, TabDragVisual::Idle) {
                1.0
            } else {
                2.0
            },
            color: border_color,
            radius: TAB_RADIUS.into(),
        },
        shadow: if is_active {
            elevation(palette, 1.0, 5.0)
        } else {
            Shadow::default()
        },
        ..container::Style::default()
    }
}

pub fn tab_top_bar_style(theme: &Theme, is_active: bool, is_dragged: bool) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(if is_dragged {
            palette.accent
        } else if is_active {
            palette.accent
        } else {
            Color::TRANSPARENT
        })),
        ..container::Style::default()
    }
}

pub fn tab_top_bar(is_active: bool, is_dragged: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| tab_top_bar_style(theme, is_active, is_dragged)
}

pub fn tab_title_area(is_active: bool, is_dragged: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let palette = VisualPalette::from_theme(theme);
        let background = if is_dragged {
            palette.accent_soft
        } else if is_active {
            palette.surface
        } else {
            palette.chrome
        };

        container::Style {
            background: Some(Background::Color(background)),
            text_color: Some(if is_active {
                palette.text
            } else {
                palette.muted_text
            }),
            border: border(0.0, Color::TRANSPARENT, TAB_RADIUS),
            ..container::Style::default()
        }
    }
}

pub fn tab_active_edge(is_active: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let palette = VisualPalette::from_theme(theme);

        container::Style {
            background: Some(Background::Color(if is_active {
                palette.accent
            } else {
                Color::TRANSPARENT
            })),
            ..container::Style::default()
        }
    }
}

pub fn utility_bar(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome_high)),
        text_color: Some(palette.text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn settings_panel(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.app)),
        text_color: Some(palette.text),
        border: border(0.0, Color::TRANSPARENT, 0.0),
        ..container::Style::default()
    }
}

pub fn settings_category_list(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome)),
        text_color: Some(palette.muted_text),
        ..container::Style::default()
    }
}

pub fn settings_content(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text),
        ..container::Style::default()
    }
}

pub fn editor_surface(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text),
        ..container::Style::default()
    }
}

pub fn function_list_panel(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.surface)),
        text_color: Some(palette.text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn function_list_header(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome_high)),
        text_color: Some(palette.text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn function_list_kind_label(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.surface_low)),
        text_color: Some(palette.muted_text),
        border: border(1.0, palette.border_soft, CONTROL_RADIUS),
        ..container::Style::default()
    }
}

pub fn function_list_empty(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: None,
        text_color: Some(palette.muted_text),
        ..container::Style::default()
    }
}

pub fn find_status(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.surface_low)),
        text_color: Some(palette.muted_text),
        border: border(1.0, palette.border_soft, CONTROL_RADIUS),
        ..container::Style::default()
    }
}

pub fn modal_scrim(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.38,
        ))),
        ..container::Style::default()
    }
}

pub fn modal_dialog(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.overlay)),
        text_color: Some(palette.text),
        border: border(1.0, palette.border_soft, RADIUS),
        shadow: elevation(palette, 8.0, 24.0),
        ..container::Style::default()
    }
}

pub fn logo_placeholder(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.accent_soft)),
        text_color: Some(palette.accent),
        border: border(1.0, palette.accent.scale_alpha(0.38), RADIUS),
        ..container::Style::default()
    }
}

pub fn tooltip(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(if palette.is_dark {
            palette.surface_high
        } else {
            Color::from_rgb8(39, 46, 56)
        })),
        text_color: Some(if palette.is_dark {
            palette.text
        } else {
            Color::WHITE
        }),
        border: border(1.0, palette.border_soft.scale_alpha(0.7), CONTROL_RADIUS),
        shadow: elevation(palette, 3.0, 12.0),
        ..container::Style::default()
    }
}

pub fn status_bar(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.chrome_high)),
        text_color: Some(palette.muted_text),
        border: hairline(palette.border_soft),
        ..container::Style::default()
    }
}

pub fn status_segment(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.surface_low)),
        text_color: Some(palette.muted_text),
        border: border(1.0, palette.border_soft, 4.0),
        ..container::Style::default()
    }
}

pub fn status_path(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: None,
        text_color: Some(palette.muted_text),
        ..container::Style::default()
    }
}

pub fn separator(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.border_soft)),
        border: border(0.0, Color::TRANSPARENT, 0.0),
        ..container::Style::default()
    }
}

pub fn menu_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = VisualPalette::from_theme(theme);
        let background = match (is_active, status) {
            (true, _) | (_, button::Status::Hovered) | (_, button::Status::Pressed) => {
                Some(Background::Color(palette.surface_low))
            }
            _ => None,
        };

        button::Style {
            background,
            text_color: palette.text,
            border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
            ..button::Style::default()
        }
    }
}

pub fn menu_label(is_active: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let palette = VisualPalette::from_theme(theme);

        container::Style {
            background: if is_active {
                Some(Background::Color(palette.surface_low))
            } else {
                None
            },
            text_color: Some(palette.text),
            border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
            ..container::Style::default()
        }
    }
}

pub fn transparent(_theme: &Theme) -> container::Style {
    container::Style {
        background: None,
        text_color: Some(iced::Color::TRANSPARENT),
        ..container::Style::default()
    }
}

pub fn menu_dropdown_band(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: Some(Background::Color(palette.overlay)),
        text_color: Some(palette.text),
        border: border(1.0, palette.border_soft, RADIUS),
        shadow: elevation(palette, 5.0, 18.0),
        ..container::Style::default()
    }
}

pub fn menu_dropdown_item(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match status {
        button::Status::Hovered => Some(Background::Color(palette.accent_soft)),
        button::Status::Pressed => Some(Background::Color(
            palette.accent_soft.mix(palette.accent, 0.16),
        )),
        _ => None,
    };

    button::Style {
        background,
        text_color: palette.text,
        border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
        ..button::Style::default()
    }
}

pub fn menu_shortcut_hint(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(menu_shortcut_hint_color(theme)),
    }
}

pub fn menu_shortcut_hint_color(theme: &Theme) -> Color {
    let palette = VisualPalette::from_theme(theme);

    palette.faint_text
}

pub fn shortcut_text_color(theme: &Theme) -> Color {
    let palette = VisualPalette::from_theme(theme);

    palette.text
}

pub fn menu_submenu_item(is_active: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let palette = VisualPalette::from_theme(theme);

        container::Style {
            background: if is_active {
                Some(Background::Color(palette.accent_soft))
            } else {
                None
            },
            text_color: Some(palette.text),
            border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
            ..container::Style::default()
        }
    }
}

pub fn menu_dropdown_disabled(theme: &Theme) -> container::Style {
    let palette = VisualPalette::from_theme(theme);

    container::Style {
        background: None,
        text_color: Some(palette.faint_text),
        ..container::Style::default()
    }
}

pub fn tool_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match status {
        button::Status::Active => palette.surface_low,
        button::Status::Hovered => palette.surface_high,
        button::Status::Pressed => palette.accent_soft,
        button::Status::Disabled => palette.surface_low.scale_alpha(0.55),
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.text,
        border: border(1.0, palette.border_soft, CONTROL_RADIUS),
        ..button::Style::default()
    }
}

pub fn icon_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match status {
        button::Status::Active => Color::TRANSPARENT,
        button::Status::Hovered => palette.surface_high,
        button::Status::Pressed => palette.accent_soft,
        button::Status::Disabled => Color::TRANSPARENT,
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: if matches!(status, button::Status::Disabled) {
            palette.faint_text
        } else {
            palette.text
        },
        border: border(
            1.0,
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                palette.border
            } else {
                Color::TRANSPARENT
            },
            CONTROL_RADIUS,
        ),
        ..button::Style::default()
    }
}

pub fn settings_category_button(
    is_active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = VisualPalette::from_theme(theme);
        let background = match (is_active, status) {
            (true, _) => palette.surface,
            (_, button::Status::Hovered) => palette.surface_high,
            (_, button::Status::Pressed) => palette.accent_soft,
            _ => Color::TRANSPARENT,
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: if is_active {
                palette.accent
            } else {
                palette.muted_text
            },
            border: border(
                1.0,
                if is_active {
                    palette.border_soft
                } else {
                    Color::TRANSPARENT
                },
                CONTROL_RADIUS,
            ),
            ..button::Style::default()
        }
    }
}

pub fn command_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match status {
        button::Status::Active => palette.surface_low,
        button::Status::Hovered => palette.surface_high,
        button::Status::Pressed => palette.accent_soft,
        button::Status::Disabled => palette.surface_low.scale_alpha(0.55),
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: if matches!(status, button::Status::Disabled) {
            palette.faint_text
        } else {
            palette.text
        },
        border: border(1.0, palette.border_soft, CONTROL_RADIUS),
        ..button::Style::default()
    }
}

pub fn primary_command_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match status {
        button::Status::Active => palette.accent,
        button::Status::Hovered => palette.accent.mix(palette.surface_high, 0.12),
        button::Status::Pressed => palette
            .accent
            .mix(Color::BLACK, if palette.is_dark { 0.08 } else { 0.16 }),
        button::Status::Disabled => palette.accent.scale_alpha(0.5),
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: palette.accent_text,
        border: border(1.0, palette.accent, CONTROL_RADIUS),
        ..button::Style::default()
    }
}

pub fn danger_command_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let background = match status {
        button::Status::Active => palette.danger_soft,
        button::Status::Hovered => palette.danger,
        button::Status::Pressed => palette.danger.mix(Color::BLACK, 0.16),
        button::Status::Disabled => palette.danger_soft.scale_alpha(0.45),
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            Color::WHITE
        } else {
            palette.danger
        },
        border: border(1.0, palette.danger.scale_alpha(0.52), CONTROL_RADIUS),
        ..button::Style::default()
    }
}

pub fn text_button(theme: &Theme, status: button::Status) -> button::Style {
    let palette = VisualPalette::from_theme(theme);
    let text_color = match status {
        button::Status::Active | button::Status::Pressed => palette.muted_text,
        button::Status::Hovered => palette.accent,
        button::Status::Disabled => palette.faint_text,
    };

    button::Style {
        text_color,
        border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
        ..button::Style::default()
    }
}

pub fn tab_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = VisualPalette::from_theme(theme);
        let background = if is_active {
            palette.surface
        } else {
            match status {
                button::Status::Hovered => palette.surface_high,
                button::Status::Pressed => palette.accent_soft,
                _ => palette.chrome,
            }
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: palette.text,
            border: border(
                1.0,
                if is_active {
                    palette.border
                } else {
                    palette.border_soft
                },
                TAB_RADIUS,
            ),
            ..button::Style::default()
        }
    }
}

pub fn tab_close_button(is_active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = VisualPalette::from_theme(theme);
        let background = match (is_active, status) {
            (_, button::Status::Hovered) => palette.danger_soft,
            (_, button::Status::Pressed) => palette.danger,
            (true, _) => palette.surface,
            (false, _) => palette.chrome,
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: if matches!(status, button::Status::Pressed) {
                Color::WHITE
            } else if is_active || matches!(status, button::Status::Hovered) {
                palette.text
            } else {
                palette.faint_text
            },
            border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
            ..button::Style::default()
        }
    }
}

pub fn tab_pin_button(
    is_active: bool,
    is_pinned: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = VisualPalette::from_theme(theme);
        let background = match (is_active, status) {
            (_, button::Status::Hovered) => palette.surface_high,
            (_, button::Status::Pressed) => palette.accent_soft,
            (true, _) => palette.surface,
            (false, _) => palette.chrome,
        };

        let text_alpha = match (is_pinned, is_active, status) {
            (true, _, _) => 0.9,
            (false, _, button::Status::Hovered | button::Status::Pressed) => 0.74,
            (false, true, _) => 0.45,
            (false, false, _) => 0.24,
        };

        button::Style {
            background: Some(Background::Color(background)),
            text_color: palette.text.scale_alpha(text_alpha),
            border: border(0.0, Color::TRANSPARENT, CONTROL_RADIUS),
            ..button::Style::default()
        }
    }
}

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let palette = VisualPalette::from_theme(theme);
    let border_color = match status {
        text_input::Status::Focused { .. } => palette.accent,
        text_input::Status::Hovered => palette.border,
        text_input::Status::Active => palette.border_soft,
        text_input::Status::Disabled => palette.border_soft.scale_alpha(0.55),
    };

    text_input::Style {
        background: Background::Color(if matches!(status, text_input::Status::Disabled) {
            palette.surface_low
        } else {
            palette.surface
        }),
        border: border(1.0, border_color, CONTROL_RADIUS),
        icon: palette.muted_text,
        placeholder: palette.faint_text,
        value: palette.text,
        selection: palette.selection,
    }
}

#[cfg(test)]
mod tests {
    use super::tab_top_bar;
    use iced::{Background, Theme};

    #[test]
    fn active_tab_top_bar_keeps_original_color() {
        let style = tab_top_bar(true, false)(&Theme::Light);

        assert!(matches!(style.background, Some(Background::Color(_))));
    }

    #[test]
    fn inactive_tab_top_bar_stays_transparent() {
        let style = tab_top_bar(false, false)(&Theme::Light);

        assert_eq!(
            style.background,
            Some(Background::Color(iced::Color::TRANSPARENT))
        );
    }
}
