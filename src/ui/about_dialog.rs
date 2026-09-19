use iced::widget::{button, column, container, opaque, row, rule, scrollable, space, stack, text};
use iced::{Alignment, Center, Element, Fill, Length};

use crate::message::{AboutTab, Message};
use crate::ui::{centered_button_label, styles};

const APP_NAME: &str = "Fragile Notepad";
const AUTHOR: &str = "Rachel Fragile <rabbit0w0@outlook.com>";

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

pub fn view(active_tab: AboutTab, rendering: RenderingDebugInfo) -> Element<'static, Message> {
    stack![
        opaque(
            container(space::vertical())
                .width(Fill)
                .height(Fill)
                .style(styles::modal_scrim)
        ),
        container(dialog(active_tab, rendering))
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    ]
    .into()
}

fn dialog(active_tab: AboutTab, rendering: RenderingDebugInfo) -> Element<'static, Message> {
    container(
        column![
            header(),
            tabs(active_tab),
            rule::horizontal(1),
            match active_tab {
                AboutTab::About => about_content(),
                AboutTab::Debug => debug_content(rendering),
                AboutTab::Licenses => licenses_content(),
            },
            row![
                space::horizontal(),
                button(centered_button_label("Close", 13))
                    .padding([7, 18])
                    .style(styles::primary_command_button)
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
    .style(styles::modal_dialog)
    .into()
}

fn header() -> Element<'static, Message> {
    row![
        container(text("FN").size(22))
            .width(Length::Fixed(56.0))
            .height(Length::Fixed(56.0))
            .center_x(Length::Fixed(56.0))
            .center_y(Length::Fixed(56.0))
            .style(styles::logo_placeholder),
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

fn tabs(active_tab: AboutTab) -> Element<'static, Message> {
    row![
        tab_button("About", AboutTab::About, active_tab),
        tab_button("Debug", AboutTab::Debug, active_tab),
        tab_button("Licenses", AboutTab::Licenses, active_tab),
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
) -> Element<'static, Message> {
    button(centered_button_label(label, 13))
        .padding([6, 14])
        .style(if tab == active_tab {
            styles::primary_command_button
        } else {
            styles::command_button
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

fn debug_content(rendering: RenderingDebugInfo) -> Element<'static, Message> {
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

fn licenses_content() -> Element<'static, Message> {
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
