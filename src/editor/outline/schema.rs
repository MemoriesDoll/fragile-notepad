use super::diagnostics;
use super::types::OutlineDiagnostic;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawOutlineSchema {
    pub schema_version: Option<String>,
    pub families: Vec<RawFamily>,
    pub languages: Vec<RawLanguage>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawFamily {
    pub id: Option<String>,
    pub adapters: Vec<String>,
    pub bodies: Vec<RawBody>,
    pub delimiters: Vec<RawDelimiter>,
    pub syntax_tokens: Vec<RawSyntaxToken>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawBody {
    pub kind: Option<String>,
    pub open: Option<String>,
    pub close: Option<String>,
    pub end_keyword: Option<String>,
    pub block_openers: Vec<String>,
    pub conditional_openers: Vec<String>,
    pub loop_openers: Vec<String>,
    pub loop_body_keyword: Option<String>,
    pub statement_boundaries: Vec<String>,
    pub member_prefixes: Vec<String>,
    pub header_end: Option<String>,
    pub line_continuation: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawLanguage {
    pub name: Option<String>,
    pub tokens: Vec<String>,
    pub family: Option<RawUseFamily>,
    pub lexical: RawLexical,
    pub signature_modifiers: Vec<String>,
    pub containers: Vec<RawRule>,
    pub declarations: Vec<RawRule>,
    pub members: Vec<RawMemberRule>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawMemberRule {
    pub kind: String,
    pub within: String,
    pub separator: String,
    pub terminator: Option<String>,
    pub prefix_pattern: Option<String>,
    pub name_pattern: Option<String>,
    pub line_skip_pattern: Option<String>,
    pub generic_open_pattern: Option<String>,
    pub generic_suffix_pattern: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawUseFamily {
    pub id: Option<String>,
    pub adapter: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawLexical {
    pub line_comments: Vec<String>,
    pub block_comments: Vec<RawBlockComment>,
    pub strings: Vec<RawString>,
    pub raw_strings: Vec<RawRawString>,
    pub identifier_prefix: Option<String>,
    pub word_character_extra: Option<String>,
    pub unicode_word_characters: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawBlockComment {
    pub open: String,
    pub close: String,
    pub nested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawString {
    pub open: String,
    pub close: String,
    pub escape: Option<String>,
    pub requires_closing_on_line: bool,
    pub single_quote_literals: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawRule {
    pub kind: Option<String>,
    pub keyword: Option<String>,
    pub name: Option<String>,
    pub body: Option<String>,
    pub scan: Option<String>,
    pub reject_names: Vec<String>,
    pub reject_previous: Vec<String>,
    pub reject_prefixes: Vec<String>,
    pub require_previous: Vec<String>,
    pub require_non_container_previous: Vec<String>,
    pub require_non_container_previous_kind: Vec<String>,
    pub start_boundaries: Vec<String>,
    pub operator_tokens: Vec<String>,
    pub name_prefixes: Vec<String>,
    pub qualified_separators: Vec<String>,
    pub compound_prefixes: Vec<String>,
    pub container_name_previous: Vec<String>,
    pub assignment_continuations: Vec<String>,
    pub control_headers: Vec<String>,
    pub assignment_arrow: Option<String>,
    pub method_containers: Vec<String>,
    pub declaration_terminator: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawDelimiter {
    pub open: String,
    pub close: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawSyntaxToken {
    pub role: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawRawString {
    pub prefixes: Vec<String>,
    pub repeat: Option<String>,
    pub open: String,
    pub close: String,
    pub suffix: String,
    pub max_delimiter_length: Option<usize>,
    pub forbidden_delimiter_characters: String,
}

pub fn parse_outline_schema(xml: &str) -> (RawOutlineSchema, Vec<OutlineDiagnostic>) {
    let mut diagnostics = Vec::new();
    let document = match roxmltree::Document::parse(xml) {
        Ok(document) => document,
        Err(error) => {
            return (
                RawOutlineSchema::default(),
                vec![diagnostics::error(format!(
                    "outline parser XML could not be parsed: {error}"
                ))],
            );
        }
    };

    let root = document.root_element();
    if !root.has_tag_name("outline-parsers") {
        return (
            RawOutlineSchema::default(),
            vec![diagnostics::error(
                "outline parser XML root must be <outline-parsers>",
            )],
        );
    }

    let schema_version = root.attribute("schema-version").map(str::to_owned);
    if schema_version.as_deref() != Some("1") {
        diagnostics.push(diagnostics::warning(format!(
            "unsupported outline parser schema version {:?}",
            schema_version.as_deref().unwrap_or("")
        )));
    }

    let families = root
        .children()
        .filter(|node| node.has_tag_name("family"))
        .map(parse_family)
        .collect();
    let languages = root
        .children()
        .filter(|node| node.has_tag_name("language"))
        .map(parse_language)
        .collect();

    (
        RawOutlineSchema {
            schema_version,
            families,
            languages,
        },
        diagnostics,
    )
}

fn parse_family(element: roxmltree::Node<'_, '_>) -> RawFamily {
    RawFamily {
        syntax_tokens: element
            .children()
            .filter(|node| node.has_tag_name("syntax-token"))
            .map(|node| RawSyntaxToken {
                role: node.attribute("role").unwrap_or("").to_owned(),
                value: node.attribute("value").unwrap_or("").to_owned(),
            })
            .collect(),
        delimiters: element
            .children()
            .filter(|node| node.has_tag_name("delimiter"))
            .map(|node| RawDelimiter {
                open: node.attribute("open").unwrap_or("").to_owned(),
                close: node.attribute("close").unwrap_or("").to_owned(),
            })
            .collect(),
        id: element.attribute("id").map(str::to_owned),
        adapters: element
            .children()
            .filter(|node| node.has_tag_name("adapter"))
            .filter_map(|node| node.attribute("name"))
            .map(str::to_owned)
            .collect(),
        bodies: element
            .children()
            .filter(|node| node.has_tag_name("body"))
            .map(parse_body)
            .collect(),
    }
}

fn parse_body(element: roxmltree::Node<'_, '_>) -> RawBody {
    RawBody {
        kind: element.attribute("kind").map(str::to_owned),
        open: element.attribute("open").map(str::to_owned),
        close: element.attribute("close").map(str::to_owned),
        end_keyword: element.attribute("end-keyword").map(str::to_owned),
        block_openers: parse_csv_attribute(element.attribute("block-openers")),
        conditional_openers: parse_csv_attribute(element.attribute("conditional-openers")),
        loop_openers: parse_csv_attribute(element.attribute("loop-openers")),
        loop_body_keyword: element.attribute("loop-body-keyword").map(str::to_owned),
        statement_boundaries: parse_csv_attribute(element.attribute("statement-boundaries")),
        member_prefixes: parse_csv_attribute(element.attribute("member-prefixes")),
        header_end: element.attribute("header-end").map(str::to_owned),
        line_continuation: element.attribute("line-continuation").map(str::to_owned),
    }
}

fn parse_language(element: roxmltree::Node<'_, '_>) -> RawLanguage {
    let lexical = element
        .children()
        .find(|node| node.has_tag_name("lexical"))
        .map(parse_lexical)
        .unwrap_or_default();

    RawLanguage {
        signature_modifiers: parse_csv_attribute(element.attribute("signature-modifiers")),
        name: element.attribute("name").map(str::to_owned),
        tokens: element
            .children()
            .filter(|node| node.has_tag_name("token"))
            .filter_map(|node| node.attribute("value"))
            .map(normalize_token)
            .collect(),
        family: element
            .children()
            .find(|node| node.has_tag_name("use-family"))
            .map(parse_use_family),
        lexical,
        members: element
            .children()
            .filter(|node| node.has_tag_name("members"))
            .map(|node| RawMemberRule {
                kind: node.attribute("kind").unwrap_or("").to_owned(),
                within: node.attribute("within").unwrap_or("").to_owned(),
                separator: node.attribute("separator").unwrap_or("").to_owned(),
                terminator: node.attribute("terminator").map(str::to_owned),
                prefix_pattern: node.attribute("prefix-pattern").map(str::to_owned),
                name_pattern: node.attribute("name-pattern").map(str::to_owned),
                line_skip_pattern: node.attribute("line-skip-pattern").map(str::to_owned),
                generic_open_pattern: node.attribute("generic-open-pattern").map(str::to_owned),
                generic_suffix_pattern: node.attribute("generic-suffix-pattern").map(str::to_owned),
            })
            .collect(),
        containers: element
            .children()
            .filter(|node| node.has_tag_name("container"))
            .map(parse_rule)
            .collect(),
        declarations: element
            .children()
            .filter(|node| node.has_tag_name("declaration"))
            .map(parse_rule)
            .collect(),
    }
}

fn parse_use_family(element: roxmltree::Node<'_, '_>) -> RawUseFamily {
    RawUseFamily {
        id: element.attribute("id").map(str::to_owned),
        adapter: element.attribute("adapter").map(str::to_owned),
    }
}

fn parse_lexical(element: roxmltree::Node<'_, '_>) -> RawLexical {
    let word_characters = element
        .children()
        .find(|node| node.has_tag_name("word-characters"));

    RawLexical {
        line_comments: element
            .children()
            .filter(|node| node.has_tag_name("line-comment"))
            .filter_map(|node| node.attribute("open"))
            .map(str::to_owned)
            .collect(),
        block_comments: element
            .children()
            .filter(|node| node.has_tag_name("block-comment"))
            .filter_map(parse_block_comment)
            .collect(),
        strings: element
            .children()
            .filter(|node| node.has_tag_name("string"))
            .filter_map(parse_string)
            .collect(),
        identifier_prefix: element.attribute("identifier-prefix").map(str::to_owned),
        raw_strings: element
            .children()
            .filter(|node| node.has_tag_name("raw-string"))
            .map(parse_raw_string)
            .collect(),
        word_character_extra: word_characters
            .and_then(|node| node.attribute("extra"))
            .map(str::to_owned),
        unicode_word_characters: word_characters
            .is_some_and(|node| node.attribute("unicode") == Some("true")),
    }
}

fn parse_block_comment(element: roxmltree::Node<'_, '_>) -> Option<RawBlockComment> {
    Some(RawBlockComment {
        open: element.attribute("open")?.to_owned(),
        close: element.attribute("close")?.to_owned(),
        nested: element.attribute("nested") == Some("true"),
    })
}

pub(super) fn parse_raw_string(element: roxmltree::Node<'_, '_>) -> RawRawString {
    RawRawString {
        prefixes: parse_csv_attribute(element.attribute("prefixes")),
        repeat: element.attribute("repeat").map(str::to_owned),
        open: element.attribute("open").unwrap_or("").to_owned(),
        close: element.attribute("close").unwrap_or("").to_owned(),
        suffix: element.attribute("suffix").unwrap_or("").to_owned(),
        max_delimiter_length: element
            .attribute("max-delimiter-length")
            .and_then(|value| value.parse().ok()),
        forbidden_delimiter_characters: element
            .attribute("forbidden-delimiter-characters")
            .unwrap_or("")
            .to_owned(),
    }
}

fn parse_string(element: roxmltree::Node<'_, '_>) -> Option<RawString> {
    let open = element.attribute("open")?;
    Some(RawString {
        open: open.to_owned(),
        close: element.attribute("close").unwrap_or(open).to_owned(),
        escape: element.attribute("escape").map(str::to_owned),
        requires_closing_on_line: element.attribute("requires-closing-on-line") == Some("true"),
        single_quote_literals: element.attribute("single-quote-literals") == Some("true"),
    })
}

fn parse_rule(element: roxmltree::Node<'_, '_>) -> RawRule {
    RawRule {
        kind: element.attribute("kind").map(str::to_owned),
        keyword: element.attribute("keyword").map(str::to_owned),
        name: element.attribute("name").map(str::to_owned),
        body: element.attribute("body").map(str::to_owned),
        scan: element.attribute("scan").map(str::to_owned),
        reject_names: parse_csv_attribute(element.attribute("reject-names")),
        reject_previous: parse_csv_attribute(element.attribute("reject-previous")),
        reject_prefixes: parse_csv_attribute(element.attribute("reject-prefixes")),
        require_previous: parse_csv_attribute(element.attribute("require-previous")),
        require_non_container_previous: parse_csv_attribute(
            element.attribute("require-non-container-previous"),
        ),
        require_non_container_previous_kind: parse_csv_attribute(
            element.attribute("require-non-container-previous-kind"),
        ),
        start_boundaries: parse_csv_attribute(element.attribute("start-boundaries")),
        operator_tokens: parse_csv_attribute(element.attribute("operator-tokens")),
        name_prefixes: parse_csv_attribute(element.attribute("name-prefixes")),
        qualified_separators: parse_csv_attribute(element.attribute("qualified-separators")),
        compound_prefixes: parse_csv_attribute(element.attribute("compound-prefixes")),
        container_name_previous: parse_csv_attribute(element.attribute("container-name-previous")),
        assignment_continuations: parse_csv_attribute(
            element.attribute("assignment-continuations"),
        ),
        control_headers: parse_csv_attribute(element.attribute("control-headers")),
        assignment_arrow: element
            .attribute("assignment-arrow")
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        method_containers: parse_csv_attribute(element.attribute("method-containers")),
        declaration_terminator: element
            .attribute("declaration-terminator")
            .map(str::to_owned),
    }
}

fn parse_csv_attribute(value: Option<&str>) -> Vec<String> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn normalize_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_party_outline_schema() {
        let (schema, diagnostics) =
            parse_outline_schema(crate::assets::syntax::outline_parsers_xml());

        assert!(diagnostics.is_empty());
        assert_eq!(schema.schema_version.as_deref(), Some("1"));
        assert!(
            schema
                .families
                .iter()
                .any(|family| family.id.as_deref() == Some("brace"))
        );
        assert!(schema.languages.iter().any(|language| {
            language.name.as_deref() == Some("Rust")
                && language.tokens.iter().any(|token| token == "rs")
        }));
    }

    #[test]
    fn malformed_xml_returns_diagnostic_instead_of_panicking() {
        let (schema, diagnostics) = parse_outline_schema("<outline-parsers>");

        assert!(schema.languages.is_empty());
        assert_eq!(diagnostics.len(), 1);
    }
}
