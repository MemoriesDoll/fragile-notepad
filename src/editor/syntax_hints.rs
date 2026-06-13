use super::word::WordCharacters;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SyntaxHintSet {
    fallback: SyntaxHints,
    languages: Vec<LanguageHints>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LanguageHints {
    tokens: Vec<String>,
    hints: SyntaxHints,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct SyntaxHints {
    pub(super) line_comments: Vec<String>,
    pub(super) block_comments: Vec<BlockCommentHint>,
    pub(super) strings: Vec<StringHint>,
    pub(super) raw_strings: bool,
    pub(super) word_characters: WordCharacters,
    pub(super) outline_rules: Vec<OutlineRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BlockCommentHint {
    pub(super) open: String,
    pub(super) close: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StringHint {
    pub(super) open: String,
    pub(super) close: String,
    pub(super) escape: Option<String>,
    pub(super) requires_closing_on_line: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutlineRule {
    pub(super) kind: OutlineRuleKind,
    pub(super) keyword: Vec<String>,
    pub(super) name_position: OutlineNamePosition,
    pub(super) modifiers: Vec<String>,
    pub(super) method_containers: Vec<String>,
    pub(super) declaration_terminator: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutlineRuleKind {
    KeywordBody,
    IndentKeyword,
    EndKeyword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutlineNamePosition {
    AfterKeyword,
}

impl SyntaxHintSet {
    pub(super) fn load() -> Self {
        parse_syntax_hints(crate::assets::syntax::folding_hints_xml())
            .unwrap_or_else(default_syntax_hint_set)
    }

    pub(super) fn hints_for(&self, syntax_token: &str) -> SyntaxHints {
        let syntax_token = syntax_token.to_ascii_lowercase();

        self.languages
            .iter()
            .find(|language| language.tokens.iter().any(|token| token == &syntax_token))
            .map(|language| language.hints.clone())
            .unwrap_or_else(|| self.fallback.clone())
    }
}

fn parse_syntax_hints(xml: &str) -> Option<SyntaxHintSet> {
    let document = roxmltree::Document::parse(xml).ok()?;
    let root = document.root_element();

    let fallback = root
        .children()
        .find(|node| node.has_tag_name("fallback"))
        .map(parse_hints_section)
        .unwrap_or_else(|| default_syntax_hint_set().fallback);
    let languages = root
        .children()
        .filter(|node| node.has_tag_name("language"))
        .map(|language| LanguageHints {
            tokens: parse_tokens(language),
            hints: parse_hints_section(language),
        })
        .filter(|language| !language.tokens.is_empty())
        .collect::<Vec<_>>();

    Some(SyntaxHintSet {
        fallback,
        languages,
    })
}

fn parse_hints_section(section: roxmltree::Node<'_, '_>) -> SyntaxHints {
    let mut hints = SyntaxHints::default();

    for element in section
        .children()
        .filter(|node| node.has_tag_name("line-comment"))
    {
        if let Some(open) = element.attribute("open") {
            hints.line_comments.push(open.to_owned());
        }
    }

    for element in section
        .children()
        .filter(|node| node.has_tag_name("block-comment"))
    {
        let Some(open) = element.attribute("open") else {
            continue;
        };
        let Some(close) = element.attribute("close") else {
            continue;
        };
        hints.block_comments.push(BlockCommentHint {
            open: open.to_owned(),
            close: close.to_owned(),
        });
    }

    for element in section
        .children()
        .filter(|node| node.has_tag_name("string"))
    {
        let Some(open) = element.attribute("open") else {
            continue;
        };
        hints.strings.push(StringHint {
            open: open.to_owned(),
            close: element.attribute("close").unwrap_or(open).to_owned(),
            escape: element.attribute("escape").map(str::to_owned),
            requires_closing_on_line: element.attribute("requires-closing-on-line") == Some("true"),
        });
    }

    hints.raw_strings = section
        .children()
        .filter(|node| node.has_tag_name("raw-string"))
        .any(|element| element.attribute("kind") == Some("rust"));

    hints.outline_rules = section
        .children()
        .filter(|node| node.has_tag_name("outline"))
        .filter_map(parse_outline_rule)
        .collect();

    hints.word_characters = section
        .children()
        .find(|node| node.has_tag_name("word-characters"))
        .map(parse_word_characters)
        .unwrap_or_default();

    hints
}

fn parse_word_characters(element: roxmltree::Node<'_, '_>) -> WordCharacters {
    WordCharacters::new(
        element.attribute("extra").unwrap_or("_"),
        element.attribute("unicode") == Some("true"),
    )
}

fn parse_tokens(language: roxmltree::Node<'_, '_>) -> Vec<String> {
    language
        .children()
        .filter(|node| node.has_tag_name("token"))
        .filter_map(|element| element.attribute("value"))
        .map(str::to_ascii_lowercase)
        .collect()
}

fn parse_outline_rule(element: roxmltree::Node<'_, '_>) -> Option<OutlineRule> {
    let kind = match element.attribute("kind")? {
        "keyword-body" => OutlineRuleKind::KeywordBody,
        "indent-keyword" => OutlineRuleKind::IndentKeyword,
        "end-keyword" => OutlineRuleKind::EndKeyword,
        _ => return None,
    };
    let keyword = parse_keyword_sequence(element.attribute("keyword")?);
    if keyword.is_empty() {
        return None;
    }
    let name_position = match element
        .attribute("name-position")
        .unwrap_or("after-keyword")
    {
        "after-keyword" => OutlineNamePosition::AfterKeyword,
        _ => return None,
    };

    Some(OutlineRule {
        kind,
        keyword,
        name_position,
        modifiers: parse_csv_attribute(element.attribute("modifiers")),
        method_containers: parse_csv_attribute(element.attribute("method-containers")),
        declaration_terminator: element
            .attribute("declaration-terminator")
            .map(str::to_owned),
    })
}

fn parse_keyword_sequence(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_csv_attribute(value: Option<&str>) -> Vec<String> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn default_syntax_hint_set() -> SyntaxHintSet {
    SyntaxHintSet {
        fallback: SyntaxHints {
            line_comments: vec!["//".to_owned()],
            block_comments: vec![BlockCommentHint {
                open: "/*".to_owned(),
                close: "*/".to_owned(),
            }],
            strings: vec![
                StringHint {
                    open: "\"".to_owned(),
                    close: "\"".to_owned(),
                    escape: Some("\\".to_owned()),
                    requires_closing_on_line: false,
                },
                StringHint {
                    open: "'".to_owned(),
                    close: "'".to_owned(),
                    escape: Some("\\".to_owned()),
                    requires_closing_on_line: true,
                },
                StringHint {
                    open: "`".to_owned(),
                    close: "`".to_owned(),
                    escape: Some("\\".to_owned()),
                    requires_closing_on_line: false,
                },
            ],
            raw_strings: true,
            word_characters: WordCharacters::default(),
            outline_rules: Vec::new(),
        },
        languages: Vec::new(),
    }
}
