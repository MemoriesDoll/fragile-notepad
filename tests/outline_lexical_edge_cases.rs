use fragile_notepad::editor::EditorBuffer;
use fragile_notepad::editor::outline::{OutlineEngine, OutlineParseResult, OutlineRegistry};

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
fn javascript_keyword_properties_are_operands_before_division() {
    for property in [
        "return", "throw", "in", "of", "void", "delete", "typeof", "await",
    ] {
        let source = format!("const ratio = obj.{property} / denominator; function real() {{}}");
        assert_eq!(names(&parse(&source, "js")), ["real"], "{source}");
    }
}

#[test]
fn javascript_expression_bodies_are_operands_before_division() {
    for (source, expected) in [
        (
            "const ratio = { value: 1 } / denominator; function real() {}",
            vec!["real"],
        ),
        (
            "const ratio = function named() {} / denominator; function real() {}",
            vec!["named", "real"],
        ),
        (
            "consume(function named() {} / denominator); function real() {}",
            vec!["named", "real"],
        ),
        (
            "function outer() { return function named() {} / denominator; } function real() {}",
            vec!["outer", "named", "real"],
        ),
        (
            "const ratio = (() => {}) / denominator; function real() {}",
            vec!["real"],
        ),
        (
            "const ratio = class Item {} / denominator; function real() {}",
            vec!["real"],
        ),
    ] {
        assert_eq!(names(&parse(source, "js")), expected, "{source}");
    }
}

#[test]
fn javascript_statement_regex_literals_do_not_create_symbols() {
    for source in [
        "if (ready) {} else /function fake() {}/.test(value); function real() {}",
        "do /function fake() {}/.test(value); while (ready); function real() {}",
        "const matches = value instanceof /function fake() {}/; function real() {}",
        "function before() {} /function fake() {}/.test(value); function real() {}",
        "if (ready) {} /function fake() {}/.test(value); function real() {}",
    ] {
        let expected: &[&str] = if source.starts_with("function before") {
            &["before", "real"]
        } else {
            &["real"]
        };
        let result = parse(source, "js");
        assert_eq!(names(&result), expected, "{source}: {:?}", result.tree);
    }
}

#[test]
fn ruby_operand_shifts_remain_code_with_matching_constant_lines() {
    for operand in ["1", "'text'"] {
        let source = format!(
            "COUNT = 1\nvalue = {operand} <<COUNT\ndef between\nend\nCOUNT\ndef after\nend"
        );
        assert_eq!(
            names(&parse(&source, "rb")),
            ["between", "after"],
            "{source}"
        );
    }
}

#[test]
fn ruby_many_shifts_preserve_following_functions() {
    let source = format!("{}def real\nend", "value = 1 <<COUNT\n".repeat(1_800),);
    let started = std::time::Instant::now();
    assert_eq!(names(&parse(&source, "rb")), ["real"]);
    eprintln!("1800 Ruby shifts: {:?}", started.elapsed());
}

#[test]
fn javascript_nested_divisions_preserve_function_ranges() {
    for count in [900, 1_800] {
        let source = format!(
            "function outer() {{ return {}value{}; }} function after() {{}}",
            "(".repeat(count),
            ") / denominator".repeat(count),
        );
        let started = std::time::Instant::now();
        let result = parse(&source, "js");
        assert_eq!(names(&result), ["outer", "after"]);
        assert_eq!(result.tree.roots.len(), 2);
        assert_eq!(
            result.functions[0].range.end.column,
            source.find(" function after").unwrap()
        );
        eprintln!("{count} nested JS divisions: {:?}", started.elapsed());
    }
}
