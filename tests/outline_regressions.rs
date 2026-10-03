use fragile_notepad::editor::outline::{
    OutlineEngine, OutlineNodeKind, OutlineParseResult, OutlineRegistry,
};
use fragile_notepad::editor::{EditorBuffer, EditorPosition};

fn parse(text: &str, syntax: &str) -> OutlineParseResult {
    let registry = OutlineRegistry::shared();
    assert!(
        registry.diagnostics().is_empty(),
        "{:?}",
        registry.diagnostics()
    );
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
fn rust_impl_types_are_not_implementation_blocks() {
    let text = "fn first(value: impl Clone) -> impl Iterator<Item=u8> { fn nested() {} [] .into_iter() }\nfn second() -> Option<impl Clone> { None::<u8> }";
    let result = parse(text, "rs");
    assert_eq!(names(&result), ["first", "nested", "second"]);
    assert!(
        result
            .tree
            .roots
            .iter()
            .all(|node| node.kind == OutlineNodeKind::Function)
    );
    assert_eq!(result.functions[1].depth, 1);
}

#[test]
fn rust_implementation_labels_preserve_target_and_trait() {
    let result = parse(
        "impl<T: Into<Vec<u8>>> path::Holder<T> {}\nimpl Display for path::Widget {}\nunsafe impl Send for path::Widget {}",
        "rs",
    );
    assert_eq!(
        result
            .tree
            .roots
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        [
            "path::Holder<T>",
            "Display for path::Widget",
            "Send for path::Widget"
        ]
    );
}

#[test]
fn patterned_declarations_keep_comments_and_attributes() {
    let result = parse(
        "#[cfg(feature=\"test\")]\nimpl /* target */ Widget {}",
        "rs",
    );
    assert_eq!(result.tree.roots.len(), 1);
    assert_eq!(result.tree.roots[0].name, "Widget");
    let result = parse("fun /* name */ greet() {}", "kt");
    assert_eq!(names(&result), ["greet"]);
    let text = "class Item { Item() : n /* initializer */ {1} {} int n; };";
    let result = parse(text, "cpp");
    assert_eq!(names(&result), ["Item"]);
    let body = result.functions[0].body_range.unwrap();
    assert_eq!(body.start.column, text.find("{} int").unwrap());
}

#[test]
fn const_parameter_comparisons_and_shifts_do_not_hide_rust_functions() {
    let result = parse(
        "fn compared(_: [u8; (1 < 2) as usize]) {}\nfn shifted(_: [u8; 1 << 2]) {}",
        "rs",
    );
    assert_eq!(names(&result), ["compared", "shifted"]);
}

#[test]
fn c_pointer_and_reference_returns_and_conversion_operators_are_named() {
    let result = parse(
        "int *pointer(void) { return 0; }\nint* joined() { return 0; }\nint& reference() { return value; }\nclass Item { operator bool() const { return true; } operator path::Type*() { return 0; } };",
        "cpp",
    );
    assert_eq!(
        names(&result),
        [
            "pointer",
            "joined",
            "reference",
            "operator bool",
            "operator path::Type*"
        ]
    );
}

#[test]
fn type_literal_braces_and_generic_methods_preserve_actual_bodies() {
    let text = "function create(): { value: number } {\n return { value: 1 };\n}\nclass Item { identity<T>(value: T): T { return value; } }\nconst make = (): { value: number } => {\n return { value: 2 };\n};";
    let result = parse(text, "ts");
    assert_eq!(names(&result), ["create", "identity", "make"]);
    assert_eq!(result.functions[0].range.end, EditorPosition::new(2, 1));
    assert_eq!(
        result.functions[0].body_range.unwrap().start,
        EditorPosition::new(0, 37)
    );
    assert_eq!(result.functions[2].range.end, EditorPosition::new(6, 1));
}

#[test]
fn ctor_braced_initializers_and_annotation_defaults_are_signature_groups() {
    let text = "class Item { Item() : n{1}, m{2} {\n work();\n} int n, m; };";
    let result = parse(text, "cpp");
    assert_eq!(names(&result), ["Item"]);
    assert_eq!(result.functions[0].range.end, EditorPosition::new(2, 1));
    assert_eq!(
        result.functions[0].body_range.unwrap().start,
        EditorPosition::new(0, 33)
    );
    let result = parse(
        "@interface Options { String[] value() default {\"a\", \"b\"}; }",
        "java",
    );
    assert_eq!(names(&result), ["value"]);
    assert!(result.functions[0].body_range.is_none());
}

#[test]
fn kotlin_function_names_and_next_line_body_are_preserved() {
    let text = "fun String.greet() {}\nfun <T> identity(value: T): T { return value }\nfun List<String>.`some name`() {}\nfun split(): Int\n{ return 1 }\nfun expression() = 1\nfun later() {}";
    let result = parse(text, "kt");
    assert_eq!(
        names(&result),
        [
            "greet",
            "identity",
            "some name",
            "split",
            "expression",
            "later"
        ]
    );
    assert_eq!(result.functions[3].range.end, EditorPosition::new(4, 12));
    assert!(result.functions[3].body_range.is_some());
    assert!(result.functions[4].body_range.is_none());
}

#[test]
fn ruby_special_names_and_endless_definitions_do_not_absorb_following_methods() {
    let text = "class Item\n def self.save\n end\n def value=(value)\n end\n def +(other)\n end\n def first = 1\n def later\n end\nend";
    let result = parse(text, "rb");
    assert_eq!(
        names(&result),
        ["self.save", "value=", "+", "first", "later"]
    );
    assert_eq!(result.functions[3].range.end, EditorPosition::new(7, 14));
    assert_eq!(result.functions[4].depth, 1);
    assert_eq!(result.tree.roots[0].range.end, EditorPosition::new(10, 3));
    let result = parse("def regular; value = 1; end\ndef later; end", "rb");
    assert_eq!(names(&result), ["regular", "later"]);
    assert_eq!(result.functions[1].depth, 0);
}

#[test]
fn inline_python_function_body_is_nonempty() {
    let result = parse("def inline(): return 1\ndef later(): pass", "py");
    assert_eq!(names(&result), ["inline", "later"]);
    let body = result.functions[0].body_range.unwrap();
    assert_eq!(body.start, EditorPosition::new(0, 13));
    assert_eq!(body.end, EditorPosition::new(0, 22));
}

#[test]
fn nondeclaration_class_keywords_do_not_create_containers() {
    for (syntax, text, expected) in [
        (
            "java",
            "class Real { Class<?> type = String.class; void work() {} }",
            "Real",
        ),
        ("js", "const object = { class: {} }; class Real {}", "Real"),
        (
            "cpp",
            "template<class T, class U> class Real { void work() {} };",
            "Real",
        ),
    ] {
        let result = parse(text, syntax);
        assert_eq!(
            result
                .tree
                .roots
                .iter()
                .map(|node| node.name.as_str())
                .collect::<Vec<_>>(),
            [expected],
            "{syntax}"
        );
    }
}

#[test]
fn java_compact_record_constructors_have_complete_ranges() {
    let text = "record Item(int value) {\n public Item {\n if (value < 0) throw new IllegalArgumentException();\n }\n int doubled() { return value * 2; }\n}";
    let result = parse(text, "java");
    assert_eq!(names(&result), ["Item", "doubled"]);
    assert_eq!(result.functions[0].range.start, EditorPosition::new(1, 1));
    assert_eq!(result.functions[0].range.end, EditorPosition::new(3, 2));
    assert_eq!(
        result.tree.roots[0].children[0].kind,
        OutlineNodeKind::Constructor
    );
    assert_eq!(result.functions[1].depth, 1);
}

#[test]
fn javascript_quoted_and_computed_methods_keep_names_and_nested_arrows() {
    let text = "class Item {\n 'some name'() { const nested = () => {}; }\n \"double\"() {}\n [Symbol.iterator]() {}\n}\nconst obj = { 'quoted'() {}, [key]() {}, ordinary() {} };";
    let result = parse(text, "js");
    assert_eq!(
        names(&result),
        [
            "some name",
            "nested",
            "double",
            "Symbol.iterator",
            "quoted",
            "key",
            "ordinary"
        ]
    );
    assert_eq!(result.functions[1].depth, 2);
    assert_eq!(
        result.functions[0].range.end,
        EditorPosition::new(1, text.lines().nth(1).unwrap().len())
    );
}
