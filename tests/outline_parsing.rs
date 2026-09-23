use fragile_notepad::editor::outline::{OutlineEngine, OutlineParseResult, OutlineRegistry};
use fragile_notepad::editor::{EditorBuffer, EditorPosition, containing_function};

fn parse(text: &str, syntax: &str) -> OutlineParseResult {
    let registry = OutlineRegistry::shared();
    OutlineEngine::new(
        registry.plan_for_syntax(syntax).unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(text), syntax)
}

fn names(result: &OutlineParseResult) -> Vec<&str> {
    result
        .functions
        .iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

#[test]
fn opaque_literals_do_not_hide_following_declarations_or_create_fake_ones() {
    for (syntax, text) in [
        ("py", "\"\"\"a docstring\"\"\"\ndef real():\n    pass"),
        ("py", "'''a docstring'''\ndef real():\n    pass"),
        (
            "kt",
            "val text = \"\"\"a \" quote\nfun fake() {}\n\"\"\"\nfun real() {}",
        ),
        (
            "java",
            "class Sample { String text = \"\"\"\n a \" quote\n void fake() {}\n\"\"\"; void real() {} }",
        ),
        (
            "rs",
            "const TEXT: &[u8] = br##\"a \" quote fn fake() {}\"##; fn real() {}",
        ),
        (
            "rs",
            "const TEXT: &CStr = cr#\"a \" quote fn fake() {}\"#; fn real() {}",
        ),
        (
            "cpp",
            "auto text = u8R\"tag(a \" quote; void fake() {})tag\"; void real() {}",
        ),
        (
            "js",
            "const text = `a ' quote function fake() {}`; function real() {}",
        ),
        ("rb", "text = 'a \\' quote; def fake; end'; def real; end"),
    ] {
        let result = parse(text, syntax);
        assert_eq!(names(&result), ["real"], "{syntax}: {text}");
    }
}

#[test]
fn modifiers_and_comments_preserve_signature_and_identifier_boundaries() {
    let result = parse("pub(in crate::scope) async fn réel() {}", "rs");
    assert_eq!(names(&result), ["réel"]);
    assert_eq!(result.functions[0].range.start, EditorPosition::new(0, 0));
    let result = parse(
        "class Item { public /* type */ void /* name */ work() {} }",
        "java",
    );
    assert_eq!(names(&result), ["work"]);
    let result = parse("int /* type */ work() { return 1; }", "cpp");
    assert_eq!(names(&result), ["work"]);
}

#[test]
fn python_multiline_headers_literals_comments_and_continuations_preserve_ownership() {
    let text = "class Service(\n    Base,\n):\n    def load(\n        self,\n        value: dict[str, int],\n    ):\n        \"\"\"doc\nunindented documentation\n\"\"\"\n# unindented comment\n        values = [\n0,\n]\n        return values\n\ndef later():\n    pass";
    let result = parse(text, "py");
    assert_eq!(names(&result), ["load", "later"]);
    assert_eq!(result.functions[0].depth, 1);
    assert_eq!(result.functions[1].depth, 0);
    assert_eq!(result.tree.roots[0].children[0].name, "load");
    assert_eq!(
        containing_function(&result.functions, EditorPosition::new(14, 10))
            .unwrap()
            .name,
        "load"
    );
}

#[test]
fn ruby_block_conditions_and_postfix_conditions_have_distinct_ends() {
    let text = "def outer\n  value if ready?\n  if ready?\n    items.each do |item|\n      item.run\n    end\n  end\n  while ready? do\n    run\n  end\n  finish\nend\ndef later\nend";
    let result = parse(text, "rb");
    assert_eq!(names(&result), ["outer", "later"]);
    assert_eq!(result.functions[0].range.end, EditorPosition::new(11, 3));
    assert_eq!(result.functions[1].range.end, EditorPosition::new(13, 3));
    assert_eq!(result.functions[1].depth, 0);
    assert_eq!(
        containing_function(&result.functions, EditorPosition::new(10, 4))
            .unwrap()
            .name,
        "outer"
    );
}

#[test]
fn ruby_loop_header_arguments_do_not_introduce_an_extra_block() {
    let text = "def outer\n  while ready?(item) do\n    run\n  end\nend\ndef later\nend";
    let result = parse(text, "rb");
    assert_eq!(names(&result), ["outer", "later"]);
    assert_eq!(result.functions[0].range.end, EditorPosition::new(4, 3));
    assert_eq!(result.functions[1].depth, 0);
}

#[test]
fn custom_delimiters_preserve_callable_ranges_and_generic_return_types() {
    let xml = include_str!("../assets/syntax/outline-parsers.xml")
        .replace(
            "kind=\"brace\" open=\"{\" close=\"}\"",
            "kind=\"brace\" open=\"{{\" close=\"}}\"",
        )
        .replace(
            "role=\"generics-open\" value=\"&lt;\"",
            "role=\"generics-open\" value=\"«\"",
        )
        .replace(
            "role=\"generics-close\" value=\"&gt;\"",
            "role=\"generics-close\" value=\"»\"",
        );
    let registry = OutlineRegistry::from_xml(&xml);
    assert!(registry.diagnostics().is_empty());
    let engine = OutlineEngine::new(
        registry.plan_for_syntax("cpp").unwrap(),
        registry.registry_hash(),
    );
    let result = engine.parse_buffer(
        &EditorBuffer::from_text("void first() {{}}\nvoid second() {{}}"),
        "cpp",
    );
    assert_eq!(names(&result), ["first", "second"]);
    assert_eq!(result.functions[1].range.start, EditorPosition::new(1, 0));
    let result = engine.parse_buffer(
        &EditorBuffer::from_text("Vector « int » work() {{}}"),
        "cpp",
    );
    assert_eq!(names(&result), ["work"]);
    let xml = xml.replace("open=\"{{\" close=\"}}\"", "open=\"{=\" close=\"=}\"");
    let registry = OutlineRegistry::from_xml(&xml);
    let result = OutlineEngine::new(
        registry.plan_for_syntax("cpp").unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(
        &EditorBuffer::from_text("class Item {= void work() {= =} =}"),
        "cpp",
    );
    assert_eq!(names(&result), ["work"]);
}

#[test]
fn unicode_and_all_line_endings_keep_ranges_inside_the_document() {
    for ending in ["\n", "\r\n", "\r", "\n\r"] {
        let text = ["def café", "  你好", "end"].join(ending);
        let result = parse(&text, "rb");
        assert_eq!(names(&result), ["café"]);
        assert_eq!(result.functions[0].range.end, EditorPosition::new(2, 3));
    }
}

#[test]
fn containers_survive_without_function_entries() {
    let result = parse("class Empty {}", "java");
    assert!(result.functions.is_empty());
    assert_eq!(result.tree.roots[0].name, "Empty");
}

#[test]
fn nesting_counts_declarations_instead_of_indentation_widths_or_overlapping_rules() {
    let text = "async def outer():\n    if ready:\n        async def inner():\n            def leaf():\n                pass\n\nclass Service:\n  def work(self):\n    pass";
    let result = parse(text, "py");
    assert_eq!(names(&result), ["outer", "inner", "leaf", "work"]);
    assert_eq!(
        result
            .functions
            .iter()
            .map(|entry| entry.depth)
            .collect::<Vec<_>>(),
        [0, 1, 2, 1]
    );
    assert_eq!(
        result
            .tree
            .roots
            .iter()
            .find(|node| node.name == "outer")
            .unwrap()
            .children[0]
            .children[0]
            .name,
        "leaf"
    );
    assert_eq!(
        result
            .tree
            .roots
            .iter()
            .find(|node| node.name == "Service")
            .unwrap()
            .children[0]
            .name,
        "work"
    );
}

#[test]
fn custom_xml_controls_keywords_modifiers_word_characters_and_raw_literals() {
    let xml = r#"<outline-parsers schema-version="1">
      <family id="custom"><adapter name="generic-end-keyword" />
        <body kind="end-keyword" end-keyword="done" block-openers="routine,choose,repeat" conditional-openers="choose" statement-boundaries=";" />
      </family>
      <language name="Custom" signature-modifiers="visible">
        <token value="custom" /><use-family id="custom" adapter="generic-end-keyword" />
        <lexical><word-characters extra="_-" unicode="true" />
          <raw-string prefixes="q" repeat="~" open="«" close="»" />
          <string open="&quot;&quot;&quot;" close="&quot;&quot;&quot;" escape="!!" />
        </lexical>
        <declaration kind="function" keyword="routine" name="after-keyword" body="end-keyword" />
      </language>
    </outline-parsers>"#;
    let registry = OutlineRegistry::from_xml(xml);
    assert!(
        registry.diagnostics().is_empty(),
        "{:?}",
        registry.diagnostics()
    );
    let text = "q~~«routine fake done»~~\n\"\"\"escaped !!\"\"\" routine hidden done\"\"\"\nvisible routine real-name\nchoose condition\nrepeat\ndone\ndone\ndone";
    let result = OutlineEngine::new(
        registry.plan_for_syntax("custom").unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(text), "custom");
    assert_eq!(names(&result), ["real-name"]);
    assert_eq!(result.functions[0].range.start, EditorPosition::new(2, 0));
    assert_eq!(result.functions[0].range.end, EditorPosition::new(7, 4));
}

#[test]
fn custom_xml_body_delimiters_are_used_for_matching_and_ranges() {
    let xml = r#"<outline-parsers schema-version="1">
      <family id="custom"><adapter name="generic-brace" /><body kind="brace" open="«" close="»" /></family>
      <language name="Custom"><token value="custom" /><use-family id="custom" adapter="generic-brace" />
        <lexical><word-characters extra="_" unicode="true" /></lexical>
        <declaration kind="function" keyword="routine" name="after-keyword" body="brace" />
      </language>
    </outline-parsers>"#;
    let registry = OutlineRegistry::from_xml(xml);
    assert!(
        registry.diagnostics().is_empty(),
        "{:?}",
        registry.diagnostics()
    );
    let text = "routine outer « routine inner « » »";
    let result = OutlineEngine::new(
        registry.plan_for_syntax("custom").unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(text), "custom");
    assert_eq!(names(&result), ["outer", "inner"]);
    assert_eq!(result.functions[1].depth, 1);
    assert_eq!(
        result.functions[0].range.end,
        EditorPosition::new(0, text.len())
    );
}

#[test]
fn invalid_xml_lexical_rules_produce_diagnostics_instead_of_nonprogressing_scans() {
    for lexical in [
        r#"<line-comment open="" />"#,
        r#"<string open="" close="x" />"#,
        r#"<raw-string kind="unknown" />"#,
    ] {
        let xml = format!(
            r#"<outline-parsers schema-version="1">
          <family id="brace"><adapter name="generic-brace" /><body kind="brace" open="{{" close="}}" /></family>
          <language name="Invalid"><token value="invalid" /><use-family id="brace" adapter="generic-brace" />
          <lexical>{lexical}</lexical><declaration kind="function" keyword="fn" name="after-keyword" body="brace" /></language>
        </outline-parsers>"#
        );
        let registry = OutlineRegistry::from_xml(&xml);
        assert!(registry.plan_for_syntax("invalid").is_none());
        assert!(!registry.diagnostics().is_empty());
    }
}

#[test]
fn invalid_xml_member_rules_are_rejected() {
    for rule in [
        r#"kind="invalid-kind" within="enum" separator=",""#,
        r#"kind="enum-member" within="invalid-kind" separator=",""#,
        r#"kind="enum-member" within="enum" separator="""#,
        r#"kind="enum-member" within="enum" separator="," terminator="""#,
        r#"kind="enum-member" within="enum" separator="," terminator=",""#,
        r#"kind="enum-member" within="enum" separator="," prefix-pattern="[""#,
        r#"kind="enum-member" within="enum" separator="," prefix-pattern="a*""#,
    ] {
        let xml = format!(
            r#"<outline-parsers schema-version="1">
          <family id="brace"><adapter name="generic-brace" /><body kind="brace" open="{{" close="}}" /></family>
          <language name="Invalid"><token value="invalid" /><use-family id="brace" adapter="generic-brace" />
          <container kind="enum" keyword="enum" name="after-keyword" body="brace" />
          <members {rule} /></language>
        </outline-parsers>"#
        );
        let registry = OutlineRegistry::from_xml(&xml);
        assert!(registry.plan_for_syntax("invalid").is_none(), "{rule}");
        assert!(!registry.diagnostics().is_empty(), "{rule}");
    }
}

#[test]
fn callable_punctuation_and_assignment_arrows_are_selected_by_xml() {
    let xml = r#"<outline-parsers schema-version="1">
      <family id="custom"><adapter name="generic-brace" />
        <syntax-token role="parameters-open" value="«" />
        <syntax-token role="parameters-close" value="»" />
        <syntax-token role="assignment" value="←" />
        <syntax-token role="statement-end" value="§" />
        <delimiter open="«" close="»" />
        <body kind="brace" open="⟦" close="⟧" />
      </family>
      <language name="Custom"><token value="custom" /><use-family id="custom" adapter="generic-brace" />
        <lexical><word-characters extra="_" unicode="true" /></lexical>
        <declaration kind="function" scan="callable" name="before-parameters" body="brace"
                     assignment-arrow="⇒" declaration-terminator="§" />
      </language>
    </outline-parsers>"#;
    let registry = OutlineRegistry::from_xml(xml);
    assert!(
        registry.diagnostics().is_empty(),
        "{:?}",
        registry.diagnostics()
    );
    let text = "value named«» ⟦⟧\nconst lambda ← «» ⇒ ⟦⟧§\nvalue decl«»§";
    let result = OutlineEngine::new(
        registry.plan_for_syntax("custom").unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(text), "custom");
    assert_eq!(names(&result), ["named", "lambda", "decl"]);
    assert_eq!(
        result.functions[2].range.end,
        EditorPosition::new(2, "value decl«»§".len())
    );
}

#[test]
fn xml_enum_containers_keep_names_kinds_and_methods_without_duplicate_classes() {
    use fragile_notepad::editor::FunctionKind;
    use fragile_notepad::editor::outline::OutlineNodeKind;
    for (syntax, source, has_method) in [
        ("rs", "enum CloseGoal { KeepOpen, ExitApp }", false),
        ("ts", "enum CloseGoal { KeepOpen, ExitApp }", false),
        (
            "java",
            "enum CloseGoal { KeepOpen, ExitApp; void update() {} }",
            true,
        ),
        (
            "kt",
            "enum class CloseGoal { KeepOpen, ExitApp; fun update() {} }",
            true,
        ),
        ("c", "enum CloseGoal { KeepOpen, ExitApp };", false),
        ("cpp", "enum CloseGoal { KeepOpen, ExitApp };", false),
        (
            "cpp",
            "enum class CloseGoal : int { KeepOpen, ExitApp };",
            false,
        ),
        (
            "cpp",
            "enum /* comment */ struct CloseGoal { KeepOpen, ExitApp };",
            false,
        ),
    ] {
        let result = parse(source, syntax);
        assert_eq!(
            result.tree.roots.len(),
            1,
            "{syntax}: {source}: {:?}",
            result.tree
        );
        let root = &result.tree.roots[0];
        assert_eq!(root.name, "CloseGoal", "{syntax}: {source}");
        assert_eq!(root.kind, OutlineNodeKind::Enum);
        assert_eq!(root.depth, 0);
        assert_eq!(
            root.children
                .iter()
                .filter(|node| node.kind == OutlineNodeKind::EnumMember)
                .map(|node| (node.name.as_str(), node.depth))
                .collect::<Vec<_>>(),
            [("KeepOpen", 1), ("ExitApp", 1)]
        );
        assert_eq!(
            result.functions.len(),
            usize::from(has_method),
            "{syntax}: {source}"
        );
        if has_method {
            assert!(root.children.iter().any(|node| node.name == "update"));
            assert_eq!(result.functions[0].kind, FunctionKind::Method);
            assert_eq!(result.functions[0].depth, 1);
        }
    }
}

#[test]
fn enum_forward_declarations_do_not_take_the_following_type_body() {
    use fragile_notepad::editor::outline::OutlineNodeKind;
    let result = parse("enum class Goal;\nclass App { void update() {} };", "cpp");
    assert_eq!(
        result
            .tree
            .roots
            .iter()
            .map(|node| (node.name.as_str(), node.kind))
            .collect::<Vec<_>>(),
        [
            ("Goal", OutlineNodeKind::Enum),
            ("App", OutlineNodeKind::Class)
        ]
    );
    assert_eq!(result.tree.roots[0].range.end, EditorPosition::new(0, 16));
    assert_eq!(result.tree.roots[1].children[0].name, "update");
}

#[test]
fn java_enum_constant_arguments_are_not_functions() {
    let result = parse(
        "enum Goal { KeepOpen(1), ExitApp(2); Goal(int value) {} void update() {} String[] labels() { return null; } java.util.List<String> names() { return null; } }",
        "java",
    );
    assert_eq!(names(&result), ["Goal", "update", "labels", "names"]);
}

#[test]
fn enum_members_ignore_payload_fields_attributes_and_initializer_arguments() {
    use fragile_notepad::editor::outline::OutlineNodeKind;
    let source = "enum Message {\n    // Fake,\n    #[cfg(any(feature = \"a,b\", feature = \"c\"))]\n    Empty,\n    Pair(Result<A, B>, usize),\n    Record { first: A, second: B },\n    Value = compute(1, (2, 3)),\n    r#type,\n    Über,\n}\n";
    let result = parse(source, "rs");
    assert!(result.functions.is_empty());
    let members = &result.tree.roots[0].children;
    assert_eq!(
        members
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["Empty", "Pair", "Record", "Value", "type", "Über"]
    );
    assert!(
        members
            .iter()
            .all(|node| node.kind == OutlineNodeKind::EnumMember
                && node.depth == 1
                && node.children.is_empty())
    );
    assert_eq!(members[0].range.start, EditorPosition::new(3, 4));
    assert_eq!(
        members[2].range.end,
        EditorPosition::new(5, "    Record { first: A, second: B }".len())
    );
    assert_eq!(members[4].range.start, EditorPosition::new(7, 6));
    assert_eq!(
        members[5].range.end,
        EditorPosition::new(8, "    Über".len())
    );
}

#[test]
fn annotated_enum_constants_own_their_methods_and_stop_before_regular_methods() {
    use fragile_notepad::editor::outline::OutlineNodeKind;
    let result = parse(
        "enum Goal { @pkg.Label(\"a,b\") KeepOpen(1) { void act() {} }, @Deprecated ExitApp(2); Goal(int value) {} void update() {} }",
        "java",
    );
    let root = &result.tree.roots[0];
    assert_eq!(
        root.children
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["KeepOpen", "ExitApp", "Goal", "update"]
    );
    assert_eq!(root.children[0].kind, OutlineNodeKind::EnumMember);
    assert_eq!(root.children[0].children[0].name, "act");
    assert_eq!(root.children[0].children[0].depth, 2);
    assert_eq!(names(&result), ["act", "Goal", "update"]);
}

#[test]
fn enum_member_boundaries_and_prefixes_come_from_xml() {
    use fragile_notepad::editor::outline::OutlineNodeKind;
    let xml = fragile_notepad::assets::syntax::outline_parsers_xml()
        .replace("keyword=\"enum\"", "keyword=\"choice\"")
        .replace(
            "separator=\",\" terminator=\";\"",
            "separator=\"|\" terminator=\"!\"",
        )
        .replace("prefix-pattern=\"#\\s*!?\"", "prefix-pattern=\"~\"");
    let registry = OutlineRegistry::from_xml(&xml);
    assert!(
        registry.diagnostics().is_empty(),
        "{:?}",
        registry.diagnostics()
    );
    let source = "choice Palette { ~[doc(a,b)] Red(A, B) | Blue = call(1, 2) ! fn update() {} }";
    let result = OutlineEngine::new(
        registry.plan_for_syntax("rs").unwrap(),
        registry.registry_hash(),
    )
    .parse_buffer(&EditorBuffer::from_text(source), "rs");
    assert_eq!(
        result.tree.roots[0]
            .children
            .iter()
            .filter(|node| node.kind == OutlineNodeKind::EnumMember)
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["Red", "Blue"]
    );
    assert_eq!(names(&result), ["update"]);
}
