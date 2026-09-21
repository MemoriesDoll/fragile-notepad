//! Embedded non-UI application assets.

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
