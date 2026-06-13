use super::OutlinePlan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutlineLanguageAdapter {
    Rust,
    Python,
    JavaScript,
    Ruby,
    GenericBrace,
    GenericIndent,
    GenericEndKeyword,
    Unknown,
}

impl OutlineLanguageAdapter {
    pub(super) fn for_plan(plan: &OutlinePlan) -> Self {
        Self::from_name(&plan.adapter_name)
    }

    fn from_name(name: &str) -> Self {
        match name {
            "rust" => Self::Rust,
            "python" => Self::Python,
            "javascript" => Self::JavaScript,
            "ruby" => Self::Ruby,
            "generic-brace" => Self::GenericBrace,
            "generic-indent" => Self::GenericIndent,
            "generic-end-keyword" => Self::GenericEndKeyword,
            _ => Self::Unknown,
        }
    }

    pub(super) fn is_rust(self) -> bool {
        self == Self::Rust
    }

    pub(super) fn opens_end_keyword_block(self, token: &str) -> bool {
        match self {
            Self::Ruby => matches!(token, "def" | "class" | "module"),
            Self::GenericEndKeyword => token != "end",
            _ => false,
        }
    }
}

pub(super) fn is_rust_modifier_token(token: &str) -> bool {
    matches!(
        token,
        "pub" | "async" | "const" | "unsafe" | "extern" | "default" | "pub(crate)" | "pub(super)"
    )
}
