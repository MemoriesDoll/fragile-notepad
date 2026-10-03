use fragile_notepad::editor::EditorBuffer;
use fragile_notepad::editor::outline::{
    OutlineEngine, OutlineNodeKind, OutlineParseResult, OutlineRegistry,
};

fn parse(text: &str, syntax: &str) -> OutlineParseResult {
    let registry = OutlineRegistry::shared();
    OutlineEngine::new(
        registry.plan_for_syntax(syntax).unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(text), syntax)
}

#[test]
fn rust_external_modules_keep_separate_ranges_before_later_function_bodies() {
    let result = parse("mod one;\nmod two;\npub mod three;\nfn later() {}", "rs");
    let roots = &result.tree.roots;
    assert_eq!(
        roots
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["one", "two", "three", "later"]
    );
    for (line, module) in roots[..3].iter().enumerate() {
        assert_eq!(module.kind, OutlineNodeKind::Module);
        assert_eq!(module.range.start.line, line);
        assert_eq!(module.range.end.line, line);
        assert!(module.body_range.is_none());
    }
    assert_eq!(
        result
            .functions
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["later"]
    );
    let modules_only = parse("mod one; mod two;", "rs");
    assert_eq!(modules_only.tree.roots.len(), 2);
}

#[test]
fn prefixed_identifiers_are_atomic_tokens_without_keyword_containers() {
    let result = parse(
        "fn r#mod() {}\nfn r#impl() {}\nfn r#enum() {}\nmod r#type;",
        "rs",
    );
    assert_eq!(
        result
            .functions
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["mod", "impl", "enum"]
    );
    assert_eq!(result.tree.roots.len(), 4);
    assert_eq!(result.tree.roots[3].name, "type");
    assert_eq!(result.tree.roots[3].kind, OutlineNodeKind::Module);
    assert!(
        result
            .tree
            .roots
            .iter()
            .all(|node| node.name != "(anonymous)")
    );
}

#[test]
fn unicode_combining_marks_continue_identifiers_across_parser_families() {
    for (syntax, text) in [
        ("rs", "fn re\u{301}el() {}"),
        ("py", "def re\u{301}el():\n    pass"),
        ("js", "function re\u{301}el() {}"),
        ("kt", "fun re\u{301}el() {}"),
        ("rb", "def re\u{301}el\nend"),
    ] {
        let result = parse(text, syntax);
        assert_eq!(
            result
                .functions
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["re\u{301}el"],
            "{syntax}: {text}"
        );
    }
}

#[test]
fn rust_structs_unions_and_tuple_declarations_are_visible() {
    let result = parse(
        "struct Item { value: usize }\nstruct Tuple(usize);\nstruct Empty;\nunion Storage { number: usize }\nfn later() {}",
        "rs",
    );
    assert_eq!(
        result
            .tree
            .roots
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["Item", "Tuple", "Empty", "Storage", "later"]
    );
    assert!(
        result.tree.roots[..4]
            .iter()
            .all(|node| node.kind == OutlineNodeKind::Class)
    );
    assert!(result.tree.roots[1].body_range.is_none());
    assert!(result.tree.roots[2].body_range.is_none());
}

#[test]
fn invalid_declarative_name_and_literal_patterns_produce_diagnostics() {
    for attribute in [
        "name-pattern=\"[\"",
        "name-pattern=\"ordinary_without_capture\"",
        "signature-brace-prefix-pattern=\"[\"",
    ] {
        let xml = format!(
            "<outline-parsers schema-version=\"1\"><family id=\"brace\"><adapter name=\"rust\"/><body kind=\"brace\" open=\"{{\" close=\"}}\"/></family><language name=\"Custom\"><token value=\"custom\"/><use-family id=\"brace\" adapter=\"rust\"/><declaration kind=\"function\" keyword=\"fn\" body=\"brace\" {attribute}/></language></outline-parsers>"
        );
        let registry = OutlineRegistry::from_xml(&xml);
        assert!(!registry.diagnostics().is_empty(), "{attribute}");
    }
    let xml = "<outline-parsers schema-version=\"1\"><family id=\"brace\"><adapter name=\"rust\"/><body kind=\"brace\" open=\"{\" close=\"}\"/></family><language name=\"Custom\"><token value=\"custom\"/><use-family id=\"brace\" adapter=\"rust\"/><lexical><heredoc prefix-pattern=\"without_delimiter_capture\"/><regex-literal open=\"/\" close=\"/\" prefix-pattern=\"[\"/><line-skip-pattern value=\"[\"/></lexical><declaration kind=\"function\" keyword=\"fn\" body=\"brace\"/></language></outline-parsers>";
    let registry = OutlineRegistry::from_xml(xml);
    assert!(registry.diagnostics().len() >= 3);
    assert!(registry.plan_for_syntax("custom").is_none());
}
