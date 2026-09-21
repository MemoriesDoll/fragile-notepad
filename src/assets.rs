//! Embedded non-UI application assets.

pub const APP_ICON_RGBA: &[u8] = include_bytes!("../assets/illustrations/bunny/app.rgba");

pub fn app_icon_handle() -> iced::advanced::image::Handle {
    static ICON: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(256, 256, APP_ICON_RGBA)
        });
    ICON.clone()
}

pub fn title_icon_handle() -> iced::advanced::image::Handle {
    static ICON: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(
                64,
                64,
                include_bytes!("../assets/illustrations/bunny/title-bar.rgba").as_slice(),
            )
        });
    ICON.clone()
}

pub fn app_blink_handle(closed: bool) -> iced::advanced::image::Handle {
    static HALF: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(
                256,
                256,
                include_bytes!("../assets/illustrations/bunny/app-half.rgba").as_slice(),
            )
        });
    static CLOSED: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(
                256,
                256,
                include_bytes!("../assets/illustrations/bunny/app-closed.rgba").as_slice(),
            )
        });
    if closed { CLOSED.clone() } else { HALF.clone() }
}

pub fn quill_handle() -> iced::advanced::image::Handle {
    static QUILL: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(
                400,
                440,
                include_bytes!("../assets/illustrations/macaw-quill.rgba").as_slice(),
            )
        });
    QUILL.clone()
}

pub mod syntax {
    pub fn folding_hints_xml() -> &'static str {
        include_str!("../assets/syntax/folding-hints.xml")
    }

    pub fn outline_parsers_xml() -> &'static str {
        include_str!("../assets/syntax/outline-parsers.xml")
    }
}
