use iced::widget::{button, column, container, opaque, row, rule, scrollable, space, stack, text};
use iced::{Alignment, Center, Element, Fill, Length};

use crate::message::{AboutTab, Message};
use crate::ui::{centered_button_label, motion, styles};

const APP_NAME: &str = "Fragile Notepad";
const AUTHOR: &str = "Rachel Fragile <rabbit0w0@outlook.com>";

// Fade the actual paint colors so the editor remains visible behind the modal.
// A solid veil (used by docked panels) would hide that backdrop instead.
fn fade_container(mut style: container::Style, opacity: f32) -> container::Style {
    style.background = style.background.map(|color| color.scale_alpha(opacity));
    style.text_color = style.text_color.map(|color| color.scale_alpha(opacity));
    style.border.color = style.border.color.scale_alpha(opacity);
    style.shadow.color = style.shadow.color.scale_alpha(opacity);
    style
}

fn fade_button(mut style: button::Style, opacity: f32) -> button::Style {
    style.background = style.background.map(|color| color.scale_alpha(opacity));
    style.text_color = style.text_color.scale_alpha(opacity);
    style.border.color = style.border.color.scale_alpha(opacity);
    style.shadow.color = style.shadow.color.scale_alpha(opacity);
    style
}

fn fade_scrollable(mut style: scrollable::Style, opacity: f32) -> scrollable::Style {
    style.container = fade_container(style.container, opacity);
    for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
        rail.background = rail.background.map(|color| color.scale_alpha(opacity));
        rail.border.color = rail.border.color.scale_alpha(opacity);
        rail.scroller.background = rail.scroller.background.scale_alpha(opacity);
        rail.scroller.border.color = rail.scroller.border.color.scale_alpha(opacity);
    }
    style.gap = style.gap.map(|color| color.scale_alpha(opacity));
    style.auto_scroll.background = style.auto_scroll.background.scale_alpha(opacity);
    style.auto_scroll.border.color = style.auto_scroll.border.color.scale_alpha(opacity);
    style.auto_scroll.shadow.color = style.auto_scroll.shadow.color.scale_alpha(opacity);
    style.auto_scroll.icon = style.auto_scroll.icon.scale_alpha(opacity);
    style
}

#[derive(Debug, Clone)]
pub struct RenderingDebugInfo {
    pub current_renderer: String,
    pub rendering_policy: String,
}

struct LicenseEntry {
    name: &'static str,
    version: &'static str,
    license: &'static str,
    notes: &'static str,
}

const LICENSES: &[LicenseEntry] = &[
    LicenseEntry {
        name: "iced",
        version: "0.15.0-dev",
        license: "MIT",
        notes: "GUI toolkit, vendored under vendor/iced/LICENSE.",
    },
    LicenseEntry {
        name: "encoding_rs",
        version: "0.8.35",
        license: "MIT OR Apache-2.0, with WHATWG encoding data terms",
        notes: "Text encoding support, vendored under vendor/encoding_rs/.",
    },
    LicenseEntry {
        name: "rfd",
        version: "0.16.0",
        license: "MIT OR Apache-2.0",
        notes: "Native file dialogs.",
    },
    LicenseEntry {
        name: "tokio",
        version: "1.52.3",
        license: "MIT",
        notes: "Async runtime used by filesystem tasks.",
    },
    LicenseEntry {
        name: "unicode-segmentation",
        version: "1.13.3",
        license: "MIT OR Apache-2.0",
        notes: "Unicode grapheme segmentation.",
    },
    LicenseEntry {
        name: "unicode-width",
        version: "0.2.2",
        license: "MIT OR Apache-2.0",
        notes: "Display width calculations.",
    },
    LicenseEntry {
        name: "tiny-skia",
        version: "0.11.4",
        license: "BSD-3-Clause",
        notes: "Software raster rendering path used by iced and tests.",
    },
    LicenseEntry {
        name: "Heroicons",
        version: "24px outline icons",
        license: "MIT",
        notes: "Text-icon replacements, bundled under assets/icons/heroicons/LICENSE.",
    },
    LicenseEntry {
        name: "Bootstrap Icons",
        version: "1.x SVG icons",
        license: "MIT",
        notes: "Shortcut modifier icons, bundled under assets/icons/bootstrap/LICENSE.",
    },
    LicenseEntry {
        name: "Tango Icon Theme",
        version: "22x22 icons",
        license: "Public Domain",
        notes: "Toolbar icons, bundled under assets/icons/tango/LICENSE.",
    },
];

pub fn view(
    active_tab: AboutTab,
    rendering: RenderingDebugInfo,
    progress: f32,
    interactive: bool,
) -> Element<'static, Message> {
    let progress = progress.clamp(0.0, 1.0);
    let content = stack![
        opaque(
            container(space::vertical())
                .width(Fill)
                .height(Fill)
                .style(move |theme| fade_container(styles::modal_scrim(theme), progress))
        ),
        container(dialog(active_tab, rendering, progress))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    ];
    // Keep the modal input barrier in place throughout the closing fade.
    opaque(motion::fade(
        content,
        1.0,
        styles::editor_background,
        interactive,
    ))
    .into()
}

fn dialog(
    active_tab: AboutTab,
    rendering: RenderingDebugInfo,
    progress: f32,
) -> Element<'static, Message> {
    container(
        column![
            header(progress),
            tabs(active_tab, progress),
            rule::horizontal(1).style(move |theme| {
                let mut style = rule::default(theme);
                style.color = style.color.scale_alpha(progress);
                style
            }),
            match active_tab {
                AboutTab::About => about_content(),
                AboutTab::Debug => debug_content(rendering, progress),
                AboutTab::Licenses => licenses_content(progress),
            },
            row![
                space::horizontal(),
                button(centered_button_label("Close", 13))
                    .padding([7, 18])
                    .style(move |theme, status| fade_button(
                        styles::primary_command_button(theme, status),
                        progress
                    ))
                    .on_press(Message::AboutClosed),
            ]
            .align_y(Center)
            .width(Fill),
        ]
        .spacing(14)
        .align_x(Alignment::Start),
    )
    .width(Length::Fixed(560.0))
    .height(Length::Fixed(470.0))
    .padding(20)
    .style(move |theme| fade_container(styles::modal_dialog(theme), progress))
    .into()
}

fn header(progress: f32) -> Element<'static, Message> {
    row![
        container(text("FN").size(22))
            .width(Length::Fixed(56.0))
            .height(Length::Fixed(56.0))
            .center_x(Length::Fixed(56.0))
            .center_y(Length::Fixed(56.0))
            .style(move |theme| fade_container(styles::logo_placeholder(theme), progress)),
        column![
            text(APP_NAME).size(22),
            text(format!("Version {}", env!("CARGO_PKG_VERSION"))).size(13),
        ]
        .spacing(4),
    ]
    .spacing(14)
    .align_y(Center)
    .into()
}

fn tabs(active_tab: AboutTab, progress: f32) -> Element<'static, Message> {
    row![
        tab_button("About", AboutTab::About, active_tab, progress),
        tab_button("Debug", AboutTab::Debug, active_tab, progress),
        tab_button("Licenses", AboutTab::Licenses, active_tab, progress),
        space::horizontal(),
    ]
    .spacing(8)
    .width(Fill)
    .into()
}

fn tab_button(
    label: &'static str,
    tab: AboutTab,
    active_tab: AboutTab,
    progress: f32,
) -> Element<'static, Message> {
    button(centered_button_label(label, 13))
        .padding([6, 14])
        .style(move |theme, status| {
            let style = if tab == active_tab {
                styles::primary_command_button(theme, status)
            } else {
                styles::command_button(theme, status)
            };
            fade_button(style, progress)
        })
        .on_press(Message::AboutTabSelected(tab))
        .into()
}

fn about_content() -> Element<'static, Message> {
    column![
        text("Author").size(13),
        text(AUTHOR).size(15),
        space::vertical().height(8),
        text("A lightweight notepad-style editor focused on fast local text editing.").size(13),
    ]
    .spacing(6)
    .width(Fill)
    .into()
}

fn debug_content(rendering: RenderingDebugInfo, progress: f32) -> Element<'static, Message> {
    let build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let panic_strategy = if cfg!(panic = "abort") {
        "abort"
    } else {
        "unwind"
    };
    let startup_probe = if std::env::var_os(crate::startup::STARTUP_PROBE_ENV).is_some() {
        "enabled"
    } else {
        "disabled"
    };

    scrollable(
        column![
            debug_section(
                "Application",
                &[
                    ("Name", env!("CARGO_PKG_NAME").to_owned()),
                    ("Version", env!("CARGO_PKG_VERSION").to_owned()),
                    ("Authors", env!("CARGO_PKG_AUTHORS").to_owned()),
                    ("Build profile", build_profile.to_owned()),
                    ("Panic strategy", panic_strategy.to_owned()),
                ],
            ),
            debug_section(
                "Runtime",
                &[
                    ("Operating system", std::env::consts::OS.to_owned()),
                    ("Architecture", std::env::consts::ARCH.to_owned()),
                    ("Platform family", std::env::consts::FAMILY.to_owned()),
                    (
                        "Startup probe",
                        format!("{startup_probe} ({})", crate::startup::STARTUP_PROBE_ENV),
                    ),
                    (
                        "First-view budget",
                        format!("{} ms", crate::startup::UI_READY_BUDGET.as_millis()),
                    ),
                ],
            ),
            debug_section(
                "Rendering",
                &[
                    ("Current renderer", rendering.current_renderer),
                    ("Rendering policy", rendering.rendering_policy),
                    ("Iced startup backend", "software".to_owned()),
                    ("Startup renderer", "tiny-skia".to_owned()),
                    ("Antialiasing", "disabled at startup".to_owned()),
                    ("VSync", "disabled at startup".to_owned()),
                ],
            ),
            debug_section(
                "Bundled Data",
                &[
                    (
                        "Outline parsers",
                        "assets/syntax/outline-parsers.xml".to_owned()
                    ),
                    (
                        "Folding hints",
                        "assets/syntax/folding-hints.xml".to_owned()
                    ),
                    ("Toolbar icons", "assets/icons/tango".to_owned()),
                    ("Shortcut icons", "assets/icons/bootstrap".to_owned()),
                    ("Dialog icons", "assets/icons/heroicons".to_owned()),
                ],
            ),
        ]
        .spacing(14)
        .width(Fill),
    )
    .style(move |theme, status| fade_scrollable(scrollable::default(theme, status), progress))
    .height(Fill)
    .width(Fill)
    .into()
}

fn debug_section(
    heading: &'static str,
    rows: &[(&'static str, String)],
) -> Element<'static, Message> {
    let rows = rows
        .iter()
        .fold(column![].spacing(4), |column, (label, value)| {
            column.push(
                row![
                    text(*label).size(12).width(Length::Fixed(142.0)),
                    text(value.clone()).size(12).width(Fill),
                ]
                .spacing(10)
                .align_y(Center),
            )
        });

    column![text(heading).size(13), rows]
        .spacing(6)
        .width(Fill)
        .into()
}

fn licenses_content(progress: f32) -> Element<'static, Message> {
    scrollable(
        column(
            LICENSES
                .iter()
                .map(license_entry)
                .collect::<Vec<Element<'static, Message>>>(),
        )
        .spacing(12)
        .width(Fill),
    )
    .style(move |theme, status| fade_scrollable(scrollable::default(theme, status), progress))
    .height(Fill)
    .width(Fill)
    .into()
}

fn license_entry(entry: &LicenseEntry) -> Element<'static, Message> {
    column![
        text(format!(
            "{} {} - {}",
            entry.name, entry.version, entry.license
        ))
        .size(13),
        text(entry.notes).size(12).width(Fill),
    ]
    .spacing(3)
    .width(Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::graphics::core::shell::Waker;
    use iced::advanced::renderer::{self, Headless};
    use iced::advanced::widget::Tree;
    use iced::advanced::{Layout, Renderer as _, Shell, layout, mouse};
    use iced::{Color, Event, Point, Rectangle, Renderer, Size, Theme, window};

    const VIEWPORT: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    fn renderer() -> Renderer {
        futures::executor::block_on(<Renderer as Headless>::new(
            renderer::Settings::default(),
            Some("tiny-skia"),
        ))
        .expect("CPU headless renderer must be available")
    }

    fn rendering_info() -> RenderingDebugInfo {
        RenderingDebugInfo {
            current_renderer: String::from("Software"),
            rendering_policy: String::from("Software only"),
        }
    }

    fn mount(content: &mut Element<'_, Message>, renderer: &Renderer) -> (Tree, layout::Node) {
        let mut tree = Tree::empty();
        tree.diff(content.as_widget_mut());
        let node = content.as_widget_mut().layout(
            &mut tree,
            renderer,
            &layout::Limits::new(Size::ZERO, VIEWPORT.size()),
        );
        (tree, node)
    }

    #[test]
    fn all_tabs_fade_their_paint_without_replacing_the_backdrop() {
        let mut renderer = renderer();
        let backdrop = Color::from_rgb8(23, 61, 97);
        renderer.reset(VIEWPORT);
        let background = renderer.screenshot(Size::new(800, 600), 1.0, backdrop);

        for theme in [Theme::Light, Theme::Dark] {
            for tab in [AboutTab::About, AboutTab::Debug, AboutTab::Licenses] {
                let mut snapshots = Vec::new();
                for progress in [0.0, 0.5, 1.0] {
                    let mut content = view(tab, rendering_info(), progress, false);
                    let (tree, node) = mount(&mut content, &renderer);
                    renderer.reset(VIEWPORT);
                    content.as_widget().draw(
                        &tree,
                        &mut renderer,
                        &theme,
                        &renderer::Style::default(),
                        Layout::new(&node),
                        mouse::Cursor::Unavailable,
                        &VIEWPORT,
                    );
                    snapshots.push(renderer.screenshot(Size::new(800, 600), 1.0, backdrop));
                }

                assert!(
                    snapshots[0] == background,
                    "{theme:?}/{tab:?}: zero opacity must preserve every backdrop pixel"
                );
                assert!(
                    snapshots[1] != background,
                    "{theme:?}/{tab:?}: intermediate opacity must be visible"
                );
                assert!(
                    snapshots[1] != snapshots[2],
                    "{theme:?}/{tab:?}: intermediate opacity must differ from the settled dialog"
                );
                assert!(
                    snapshots[2] != background,
                    "{theme:?}/{tab:?}: settled dialog must remain visible"
                );
            }
        }
    }

    #[test]
    fn closing_modal_blocks_background_clicks_and_disables_its_buttons() {
        let renderer = renderer();
        for interactive in [true, false] {
            for (point, on_tab) in [
                (Point::new(10.0, 10.0), false),
                (Point::new(155.0, 165.0), true),
            ] {
                let mut content: Element<'_, Message> = stack![
                    button(space::vertical().width(Fill).height(Fill))
                        .width(Fill)
                        .height(Fill)
                        .on_press(Message::NewFile),
                    view(AboutTab::About, rendering_info(), 0.5, interactive),
                ]
                .into();
                let (mut tree, node) = mount(&mut content, &renderer);
                let mut messages = Vec::new();
                for event in [
                    mouse::Event::ButtonPressed(mouse::Button::Left),
                    mouse::Event::ButtonReleased(mouse::Button::Left),
                ] {
                    let mut shell = Shell::new(&window::Headless, Waker::noop(), &mut messages);
                    content.as_widget_mut().update(
                        &mut tree,
                        &Event::Mouse(event),
                        Layout::new(&node),
                        mouse::Cursor::Available(point),
                        &renderer,
                        &mut shell,
                        &VIEWPORT,
                    );
                }
                assert!(
                    !messages
                        .iter()
                        .any(|message| matches!(message, Message::NewFile)),
                    "modal must block background clicks while fading"
                );
                if interactive && on_tab {
                    assert!(
                        messages.iter().any(|message| matches!(
                            message,
                            Message::AboutTabSelected(AboutTab::About)
                        )),
                        "control must be clickable before closing"
                    );
                } else {
                    assert!(
                        messages.is_empty(),
                        "closing controls must not publish actions: {messages:?}"
                    );
                }
            }
        }
    }
}
