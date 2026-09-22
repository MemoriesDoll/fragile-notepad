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

pub fn about_background_handle() -> iced::advanced::image::Handle {
    static BACKGROUND: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(
                384,
                384,
                include_bytes!("../assets/illustrations/bunny/about-background.rgba").as_slice(),
            )
        });
    BACKGROUND.clone()
}

pub fn about_bunny_handle(frame: u8) -> iced::advanced::image::Handle {
    static FRAMES: std::sync::LazyLock<[iced::advanced::image::Handle; 3]> =
        std::sync::LazyLock::new(|| {
            [
                include_bytes!("../assets/illustrations/bunny/about-bunny.rgba").as_slice(),
                include_bytes!("../assets/illustrations/bunny/about-bunny-half.rgba").as_slice(),
                include_bytes!("../assets/illustrations/bunny/about-bunny-closed.rgba").as_slice(),
            ]
            .map(|pixels| iced::advanced::image::Handle::from_rgba(384, 384, pixels))
        });
    FRAMES[usize::from(frame).min(2)].clone()
}

pub fn about_paper_handle() -> iced::advanced::image::Handle {
    static PAPER: std::sync::LazyLock<iced::advanced::image::Handle> =
        std::sync::LazyLock::new(|| {
            iced::advanced::image::Handle::from_rgba(
                384,
                384,
                include_bytes!("../assets/illustrations/bunny/about-paper.rgba").as_slice(),
            )
        });
    PAPER.clone()
}

pub mod syntax {
    pub fn folding_hints_xml() -> &'static str {
        include_str!("../assets/syntax/folding-hints.xml")
    }

    pub fn outline_parsers_xml() -> &'static str {
        include_str!("../assets/syntax/outline-parsers.xml")
    }
}
