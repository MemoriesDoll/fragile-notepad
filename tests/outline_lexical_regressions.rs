use fragile_notepad::editor::outline::{OutlineEngine, OutlineParseResult, OutlineRegistry};
use fragile_notepad::editor::{EditorBuffer, EditorPosition, containing_function};

fn parse(source: &str, syntax: &str) -> OutlineParseResult {
    let registry = OutlineRegistry::shared();
    OutlineEngine::new(
        registry.plan_for_syntax(syntax).unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(source), syntax)
}

fn names(result: &OutlineParseResult) -> Vec<&str> {
    result
        .functions
        .iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

#[test]
fn javascript_regex_contents_never_create_functions_or_containers() {
    for source in [
        "const re = /function fake() {}/g; function real() {}",
        "const re = /[\\/]function fake() {}/; function real() {}",
        "const re = /* explanation */ /class Fake { function fake() {} }/; function real() {}",
        "function real() { return /function fake() {}/; }",
        "if (ready()) /function fake() {}/.test(value); function real() {}",
        "const re = /function fake() {}\nfunction real() {}",
    ] {
        let result = parse(source, "js");
        assert_eq!(names(&result), ["real"], "{source}: {:?}", result.tree);
        assert_eq!(result.tree.roots.len(), 1, "{source}");
    }
}

#[test]
fn regex_shielding_preserves_division_and_enclosing_body_ranges() {
    let source = "function outer() {\n const re = /[}]/;\n const quotient = value / count / divisor;\n return quotient;\n}\nfunction after() {}";
    let result = parse(source, "js");
    assert_eq!(names(&result), ["outer", "after"]);
    assert_eq!(result.functions[0].range.end, EditorPosition::new(4, 1));
    assert_eq!(
        containing_function(&result.functions, EditorPosition::new(3, 4))
            .unwrap()
            .name,
        "outer"
    );
    for source in [
        "const quotient = 'text' / denominator; function real() {}",
        "const quotient = count++ / denominator; function real() {}",
        "const quotient = fn() / denominator; function real() {}",
    ] {
        assert_eq!(names(&parse(source, "js")), ["real"], "{source}");
    }
}

#[test]
fn ruby_heredocs_preserve_header_code_and_following_declarations() {
    for source in [
        "text = <<~TEXT\n def fake\n end\nTEXT\ndef real\nend",
        "text = <<-'TEXT'\n def fake\n end\n  TEXT\ndef real\nend",
        "text = <<\"TEXT\"\n def fake\n end\nTEXT\ndef real\nend",
        "text = <<TEXT\n  TEXT\ndef fake\nend\nTEXT\ndef real\nend",
        "texts = [<<A, <<B]\ndef fake_a\nend\nA\ndef fake_b\nend\nB\ndef real\nend",
        "text = <<~'END TEXT'\n def fake\n end\n  END TEXT\ndef real\nend",
    ] {
        assert_eq!(names(&parse(source, "rb")), ["real"], "{source}");
    }
    let source = "text = <<~TEXT; def header; end\ndef fake\nend\nTEXT\ndef real\nend";
    assert_eq!(names(&parse(source, "rb")), ["header", "real"]);
}

#[test]
fn ruby_shift_expressions_are_not_unterminated_heredocs() {
    for source in [
        "value = 1 <<COUNT\ndef real\nend",
        "value = 'text' <<COUNT\ndef real\nend",
        "value = 1 <<-COUNT\ndef real\nend",
        "value = 1 <<~COUNT\ndef real\nend",
    ] {
        assert_eq!(names(&parse(source, "rb")), ["real"], "{source}");
    }
}

#[test]
fn configured_literal_syntax_round_trips_and_changes_shielding() {
    let xml = fragile_notepad::assets::syntax::outline_parsers_xml().replace(
        "<regex-literal open=\"/\" close=\"/\"",
        "<regex-literal open=\"§\" close=\"§\"",
    );
    let registry = OutlineRegistry::from_xml(&xml);
    assert!(
        registry.diagnostics().is_empty(),
        "{:?}",
        registry.diagnostics()
    );
    let result = OutlineEngine::new(
        registry.plan_for_syntax("js").unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(
        &EditorBuffer::from_text("const re = §function fake() {}§; function real() {}"),
        "js",
    );
    assert_eq!(names(&result), ["real"]);
}

#[test]
fn preprocessor_directives_do_not_create_functions_but_keep_branch_items() {
    let source = "#define FAKE(x) void phantom() {}\n#define OTHER(x) \\\n void also_phantom() {}\n#if FEATURE\nvoid first() {}\n#else\nvoid second() {}\n#endif\n";
    assert_eq!(names(&parse(source, "cpp")), ["first", "second"]);
}

#[test]
fn unused_rust_macro_templates_do_not_create_symbols() {
    for (open, close) in [("{", "}"), ("(", ")"), ("[", "]")] {
        let source = format!(
            "macro_rules! unused {open} () => {{ fn generated() {{}} mod generated_module {{}} }} {close}; fn real() {{}}"
        );
        let result = parse(&source, "rs");
        assert_eq!(names(&result), ["real"], "{source}");
        assert_eq!(result.tree.roots.len(), 1);
    }
    let source = "macro_rules! unused { () => { const TEXT: &str = \"} fn fake() {}\"; /* } */ fn generated() {} } }\nfn real() {}\n";
    assert_eq!(names(&parse(source, "rs")), ["real"]);
    // Item-producing macro invocations retain their source declarations.
    assert_eq!(
        names(&parse("items! { fn visible() {} }", "rs")),
        ["visible"]
    );
}
