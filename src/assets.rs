//! Embedded non-UI application assets.

pub mod syntax {
    pub fn folding_hints_xml() -> &'static str {
        include_str!("../assets/syntax/folding-hints.xml")
    }

    pub fn outline_parsers_xml() -> &'static str {
        include_str!("../assets/syntax/outline-parsers.xml")
    }
}
