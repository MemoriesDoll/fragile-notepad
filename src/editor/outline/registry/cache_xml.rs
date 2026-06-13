use super::{CACHE_VERSION, OutlineRegistry};
use crate::editor::outline::compiler::{
    CompiledOutlineRegistry, OutlineBlockCommentPlan, OutlineBodyKind, OutlineBodyPlan,
    OutlineCallablePlan, OutlineLexicalPlan, OutlineNameCapture, OutlinePlan, OutlineRulePlan,
    OutlineScanMode, OutlineStringPlan, OutlineStructurePlan,
};
use crate::editor::outline::types::{
    OutlineDiagnostic, OutlineDiagnosticSeverity, OutlineNodeKind,
};

pub(super) fn registry_to_cache_xml(registry: &OutlineRegistry) -> String {
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    push_open(
        &mut xml,
        0,
        "compiled-outline-cache",
        &[
            ("version", CACHE_VERSION.to_owned()),
            ("hash", registry.hash.to_string()),
        ],
    );

    push_diagnostics(&mut xml, 1, "diagnostics", &registry.diagnostics);
    for plan in &registry.plans {
        push_plan(&mut xml, plan);
    }

    push_close(&mut xml, 0, "compiled-outline-cache");
    xml
}

fn push_plan(xml: &mut String, plan: &OutlinePlan) {
    push_open(
        xml,
        1,
        "plan",
        &[
            ("language-name", plan.language_name.clone()),
            ("family-id", plan.family_id.clone()),
            ("adapter-name", plan.adapter_name.clone()),
        ],
    );
    push_values(
        xml,
        2,
        "syntax-tokens",
        "token",
        "value",
        &plan.syntax_tokens,
    );
    push_rules(xml, 2, "containers", &plan.containers);
    push_rules(xml, 2, "declarations", &plan.declarations);
    push_lexical(xml, &plan.lexical);
    push_structure(xml, &plan.structure);
    push_diagnostics(xml, 2, "diagnostics", &plan.diagnostics);
    push_close(xml, 1, "plan");
}

fn push_rules(xml: &mut String, depth: usize, tag: &str, rules: &[OutlineRulePlan]) {
    push_open(xml, depth, tag, &[]);
    for rule in rules {
        push_open(
            xml,
            depth + 1,
            "rule",
            &[
                ("node-kind", node_kind_to_str(rule.node_kind).to_owned()),
                ("name", name_capture_to_str(rule.name).to_owned()),
                ("scan", scan_mode_to_str(rule.scan).to_owned()),
                ("body", body_kind_to_str(rule.body).to_owned()),
            ],
        );
        push_values(xml, depth + 2, "keyword", "part", "value", &rule.keyword);
        push_callable(xml, depth + 2, &rule.callable);
        push_node_kinds(xml, depth + 2, "method-containers", &rule.method_containers);
        if let Some(terminator) = &rule.declaration_terminator {
            push_empty(
                xml,
                depth + 2,
                "declaration-terminator",
                &[("value", terminator.clone())],
            );
        }
        push_close(xml, depth + 1, "rule");
    }
    push_close(xml, depth, tag);
}

fn push_callable(xml: &mut String, depth: usize, callable: &OutlineCallablePlan) {
    push_open(xml, depth, "callable", &[]);
    push_values(
        xml,
        depth + 1,
        "reject-names",
        "value",
        "text",
        &callable.reject_names,
    );
    push_values(
        xml,
        depth + 1,
        "reject-previous",
        "value",
        "text",
        &callable.reject_previous,
    );
    push_values(
        xml,
        depth + 1,
        "reject-prefixes",
        "value",
        "text",
        &callable.reject_prefixes,
    );
    push_values(
        xml,
        depth + 1,
        "require-previous",
        "value",
        "text",
        &callable.require_previous,
    );
    push_values(
        xml,
        depth + 1,
        "require-non-container-previous",
        "value",
        "text",
        &callable.require_non_container_previous,
    );
    push_values(
        xml,
        depth + 1,
        "require-non-container-previous-kind",
        "value",
        "text",
        &callable.require_non_container_previous_kind,
    );
    push_values(
        xml,
        depth + 1,
        "start-boundaries",
        "value",
        "text",
        &callable.start_boundaries,
    );
    push_values(
        xml,
        depth + 1,
        "operator-tokens",
        "value",
        "text",
        &callable.operator_tokens,
    );
    push_values(
        xml,
        depth + 1,
        "name-prefixes",
        "value",
        "text",
        &callable.name_prefixes,
    );
    push_values(
        xml,
        depth + 1,
        "qualified-separators",
        "value",
        "text",
        &callable.qualified_separators,
    );
    push_values(
        xml,
        depth + 1,
        "compound-prefixes",
        "value",
        "text",
        &callable.compound_prefixes,
    );
    push_values(
        xml,
        depth + 1,
        "container-name-previous",
        "value",
        "text",
        &callable.container_name_previous,
    );
    push_values(
        xml,
        depth + 1,
        "assignment-continuations",
        "value",
        "text",
        &callable.assignment_continuations,
    );
    push_values(
        xml,
        depth + 1,
        "control-headers",
        "value",
        "text",
        &callable.control_headers,
    );
    push_close(xml, depth, "callable");
}

fn push_lexical(xml: &mut String, lexical: &OutlineLexicalPlan) {
    push_open(
        xml,
        2,
        "lexical",
        &[
            ("word-character-extra", lexical.word_character_extra.clone()),
            (
                "unicode-word-characters",
                lexical.unicode_word_characters.to_string(),
            ),
        ],
    );
    push_values(
        xml,
        3,
        "line-comments",
        "comment",
        "open",
        &lexical.line_comments,
    );
    push_open(xml, 3, "block-comments", &[]);
    for comment in &lexical.block_comments {
        push_empty(
            xml,
            4,
            "comment",
            &[
                ("open", comment.open.clone()),
                ("close", comment.close.clone()),
            ],
        );
    }
    push_close(xml, 3, "block-comments");
    push_open(xml, 3, "strings", &[]);
    for string in &lexical.strings {
        let mut attributes = vec![
            ("open", string.open.clone()),
            ("close", string.close.clone()),
            (
                "requires-closing-on-line",
                string.requires_closing_on_line.to_string(),
            ),
            (
                "single-quote-literals",
                string.single_quote_literals.to_string(),
            ),
        ];
        if let Some(escape) = &string.escape {
            attributes.push(("escape", escape.clone()));
        }
        push_empty(xml, 4, "string", &attributes);
    }
    push_close(xml, 3, "strings");
    push_values(
        xml,
        3,
        "raw-strings",
        "raw-string",
        "kind",
        &lexical.raw_strings,
    );
    push_close(xml, 2, "lexical");
}

fn push_structure(xml: &mut String, structure: &OutlineStructurePlan) {
    push_open(xml, 2, "structure", &[]);
    for body in &structure.bodies {
        let mut attributes = vec![("kind", body_kind_to_str(body.kind).to_owned())];
        if let Some(open) = &body.open {
            attributes.push(("open", open.clone()));
        }
        if let Some(close) = &body.close {
            attributes.push(("close", close.clone()));
        }
        if let Some(end_keyword) = &body.end_keyword {
            attributes.push(("end-keyword", end_keyword.clone()));
        }
        push_empty(xml, 3, "body", &attributes);
    }
    push_close(xml, 2, "structure");
}

fn push_diagnostics(xml: &mut String, depth: usize, tag: &str, diagnostics: &[OutlineDiagnostic]) {
    push_open(xml, depth, tag, &[]);
    for diagnostic in diagnostics {
        push_empty(
            xml,
            depth + 1,
            "diagnostic",
            &[
                (
                    "severity",
                    diagnostic_severity_to_str(diagnostic.severity).to_owned(),
                ),
                ("message", diagnostic.message.clone()),
            ],
        );
    }
    push_close(xml, depth, tag);
}

fn push_values(
    xml: &mut String,
    depth: usize,
    tag: &str,
    value_tag: &str,
    attribute: &str,
    values: &[String],
) {
    push_open(xml, depth, tag, &[]);
    for value in values {
        push_empty(xml, depth + 1, value_tag, &[(attribute, value.clone())]);
    }
    push_close(xml, depth, tag);
}

fn push_node_kinds(xml: &mut String, depth: usize, tag: &str, values: &[OutlineNodeKind]) {
    push_open(xml, depth, tag, &[]);
    for value in values {
        push_empty(
            xml,
            depth + 1,
            "node-kind",
            &[("value", node_kind_to_str(*value).to_owned())],
        );
    }
    push_close(xml, depth, tag);
}

fn push_open(xml: &mut String, depth: usize, tag: &str, attributes: &[(&str, String)]) {
    push_indent(xml, depth);
    xml.push('<');
    xml.push_str(tag);
    push_attributes(xml, attributes);
    xml.push_str(">\n");
}

fn push_close(xml: &mut String, depth: usize, tag: &str) {
    push_indent(xml, depth);
    xml.push_str("</");
    xml.push_str(tag);
    xml.push_str(">\n");
}

fn push_empty(xml: &mut String, depth: usize, tag: &str, attributes: &[(&str, String)]) {
    push_indent(xml, depth);
    xml.push('<');
    xml.push_str(tag);
    push_attributes(xml, attributes);
    xml.push_str(" />\n");
}

fn push_attributes(xml: &mut String, attributes: &[(&str, String)]) {
    for (name, value) in attributes {
        xml.push(' ');
        xml.push_str(name);
        xml.push_str("=\"");
        xml.push_str(&escape_xml_attribute(value));
        xml.push('"');
    }
}

fn push_indent(xml: &mut String, depth: usize) {
    for _ in 0..depth {
        xml.push_str("  ");
    }
}

fn escape_xml_attribute(value: &str) -> String {
    let mut escaped = String::new();

    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }

    escaped
}

pub(super) fn registry_from_cache_xml(xml: &str) -> Option<OutlineRegistry> {
    let document = roxmltree::Document::parse(xml).ok()?;
    let root = document.root_element();

    if !root.has_tag_name("compiled-outline-cache")
        || root.attribute("version") != Some(CACHE_VERSION)
    {
        return None;
    }

    let hash = root.attribute("hash")?.parse::<u64>().ok()?;
    let plans = root
        .children()
        .filter(|node| node.has_tag_name("plan"))
        .map(parse_cached_plan)
        .collect::<Option<Vec<_>>>()?;
    let diagnostics = root
        .children()
        .find(|node| node.has_tag_name("diagnostics"))
        .map(parse_cached_diagnostics)
        .unwrap_or_else(|| Some(Vec::new()))?;

    Some(OutlineRegistry::from_compiled(
        hash,
        CompiledOutlineRegistry { plans, diagnostics },
    ))
}

fn parse_cached_plan(node: roxmltree::Node<'_, '_>) -> Option<OutlinePlan> {
    Some(OutlinePlan {
        language_name: node.attribute("language-name")?.to_owned(),
        syntax_tokens: parse_cached_values(node, "syntax-tokens", "token", "value"),
        family_id: node.attribute("family-id")?.to_owned(),
        adapter_name: node.attribute("adapter-name")?.to_owned(),
        containers: parse_cached_rules(node, "containers")?,
        declarations: parse_cached_rules(node, "declarations")?,
        lexical: parse_cached_lexical(
            node.children()
                .find(|child| child.has_tag_name("lexical"))?,
        )?,
        structure: parse_cached_structure(
            node.children()
                .find(|child| child.has_tag_name("structure"))?,
        )?,
        diagnostics: node
            .children()
            .find(|child| child.has_tag_name("diagnostics"))
            .map(parse_cached_diagnostics)
            .unwrap_or_else(|| Some(Vec::new()))?,
    })
}

fn parse_cached_rules(
    node: roxmltree::Node<'_, '_>,
    group_tag: &str,
) -> Option<Vec<OutlineRulePlan>> {
    let group = node
        .children()
        .find(|child| child.has_tag_name(group_tag))?;

    group
        .children()
        .filter(|child| child.has_tag_name("rule"))
        .map(parse_cached_rule)
        .collect()
}

fn parse_cached_rule(node: roxmltree::Node<'_, '_>) -> Option<OutlineRulePlan> {
    let callable = node.children().find(|child| child.has_tag_name("callable"));
    let terminator = node
        .children()
        .find(|child| child.has_tag_name("declaration-terminator"))
        .and_then(|child| child.attribute("value"))
        .map(str::to_owned);

    Some(OutlineRulePlan {
        node_kind: parse_node_kind(node.attribute("node-kind")?)?,
        keyword: parse_cached_values(node, "keyword", "part", "value"),
        name: parse_name_capture(node.attribute("name")?)?,
        scan: parse_scan_mode(node.attribute("scan")?)?,
        callable: callable
            .map(parse_cached_callable)
            .unwrap_or_else(|| Some(OutlineCallablePlan::default()))?,
        body: parse_body_kind(node.attribute("body")?)?,
        method_containers: parse_cached_node_kinds(node, "method-containers")?,
        declaration_terminator: terminator,
    })
}

fn parse_cached_callable(node: roxmltree::Node<'_, '_>) -> Option<OutlineCallablePlan> {
    Some(OutlineCallablePlan {
        reject_names: parse_cached_values(node, "reject-names", "value", "text"),
        reject_previous: parse_cached_values(node, "reject-previous", "value", "text"),
        reject_prefixes: parse_cached_values(node, "reject-prefixes", "value", "text"),
        require_previous: parse_cached_values(node, "require-previous", "value", "text"),
        require_non_container_previous: parse_cached_values(
            node,
            "require-non-container-previous",
            "value",
            "text",
        ),
        require_non_container_previous_kind: parse_cached_values(
            node,
            "require-non-container-previous-kind",
            "value",
            "text",
        ),
        start_boundaries: parse_cached_values(node, "start-boundaries", "value", "text"),
        operator_tokens: parse_cached_values(node, "operator-tokens", "value", "text"),
        name_prefixes: parse_cached_values(node, "name-prefixes", "value", "text"),
        qualified_separators: parse_cached_values(node, "qualified-separators", "value", "text"),
        compound_prefixes: parse_cached_values(node, "compound-prefixes", "value", "text"),
        container_name_previous: parse_cached_values(
            node,
            "container-name-previous",
            "value",
            "text",
        ),
        assignment_continuations: parse_cached_values(
            node,
            "assignment-continuations",
            "value",
            "text",
        ),
        control_headers: parse_cached_values(node, "control-headers", "value", "text"),
    })
}

fn parse_cached_lexical(node: roxmltree::Node<'_, '_>) -> Option<OutlineLexicalPlan> {
    let block_comments = node
        .children()
        .find(|child| child.has_tag_name("block-comments"))
        .into_iter()
        .flat_map(|group| group.children())
        .filter(|child| child.has_tag_name("comment"))
        .map(|child| {
            Some(OutlineBlockCommentPlan {
                open: child.attribute("open")?.to_owned(),
                close: child.attribute("close")?.to_owned(),
            })
        })
        .collect::<Option<Vec<_>>>()?;

    let strings = node
        .children()
        .find(|child| child.has_tag_name("strings"))
        .into_iter()
        .flat_map(|group| group.children())
        .filter(|child| child.has_tag_name("string"))
        .map(|child| {
            Some(OutlineStringPlan {
                open: child.attribute("open")?.to_owned(),
                close: child.attribute("close")?.to_owned(),
                escape: child.attribute("escape").map(str::to_owned),
                requires_closing_on_line: parse_bool(child.attribute("requires-closing-on-line"))?,
                single_quote_literals: parse_bool(child.attribute("single-quote-literals"))?,
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(OutlineLexicalPlan {
        line_comments: parse_cached_values(node, "line-comments", "comment", "open"),
        block_comments,
        strings,
        raw_strings: parse_cached_values(node, "raw-strings", "raw-string", "kind"),
        word_character_extra: node.attribute("word-character-extra")?.to_owned(),
        unicode_word_characters: parse_bool(node.attribute("unicode-word-characters"))?,
    })
}

fn parse_cached_structure(node: roxmltree::Node<'_, '_>) -> Option<OutlineStructurePlan> {
    let bodies = node
        .children()
        .filter(|child| child.has_tag_name("body"))
        .map(|child| {
            Some(OutlineBodyPlan {
                kind: parse_body_kind(child.attribute("kind")?)?,
                open: child.attribute("open").map(str::to_owned),
                close: child.attribute("close").map(str::to_owned),
                end_keyword: child.attribute("end-keyword").map(str::to_owned),
            })
        })
        .collect::<Option<Vec<_>>>()?;

    Some(OutlineStructurePlan { bodies })
}

fn parse_cached_diagnostics(node: roxmltree::Node<'_, '_>) -> Option<Vec<OutlineDiagnostic>> {
    node.children()
        .filter(|child| child.has_tag_name("diagnostic"))
        .map(|child| {
            Some(OutlineDiagnostic::new(
                parse_diagnostic_severity(child.attribute("severity")?)?,
                child.attribute("message")?.to_owned(),
                None,
            ))
        })
        .collect()
}

fn parse_cached_values(
    node: roxmltree::Node<'_, '_>,
    group_tag: &str,
    value_tag: &str,
    attribute: &str,
) -> Vec<String> {
    node.children()
        .find(|child| child.has_tag_name(group_tag))
        .into_iter()
        .flat_map(|group| group.children())
        .filter(|child| child.has_tag_name(value_tag))
        .filter_map(|child| child.attribute(attribute))
        .map(str::to_owned)
        .collect()
}

fn parse_cached_node_kinds(
    node: roxmltree::Node<'_, '_>,
    group_tag: &str,
) -> Option<Vec<OutlineNodeKind>> {
    node.children()
        .find(|child| child.has_tag_name(group_tag))
        .into_iter()
        .flat_map(|group| group.children())
        .filter(|child| child.has_tag_name("node-kind"))
        .map(|child| child.attribute("value").and_then(parse_node_kind))
        .collect()
}

fn parse_bool(value: Option<&str>) -> Option<bool> {
    match value? {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn node_kind_to_str(kind: OutlineNodeKind) -> &'static str {
    match kind {
        OutlineNodeKind::Module => "module",
        OutlineNodeKind::Namespace => "namespace",
        OutlineNodeKind::Class => "class",
        OutlineNodeKind::Interface => "interface",
        OutlineNodeKind::Trait => "trait",
        OutlineNodeKind::Impl => "impl",
        OutlineNodeKind::Method => "method",
        OutlineNodeKind::Constructor => "constructor",
        OutlineNodeKind::Function => "function",
        OutlineNodeKind::Declaration => "declaration",
        OutlineNodeKind::Tag => "tag",
        OutlineNodeKind::Section => "section",
        OutlineNodeKind::Unknown => "unknown",
    }
}

fn parse_node_kind(value: &str) -> Option<OutlineNodeKind> {
    match value {
        "module" => Some(OutlineNodeKind::Module),
        "namespace" => Some(OutlineNodeKind::Namespace),
        "class" => Some(OutlineNodeKind::Class),
        "interface" => Some(OutlineNodeKind::Interface),
        "trait" => Some(OutlineNodeKind::Trait),
        "impl" => Some(OutlineNodeKind::Impl),
        "method" => Some(OutlineNodeKind::Method),
        "constructor" => Some(OutlineNodeKind::Constructor),
        "function" => Some(OutlineNodeKind::Function),
        "declaration" => Some(OutlineNodeKind::Declaration),
        "tag" => Some(OutlineNodeKind::Tag),
        "section" => Some(OutlineNodeKind::Section),
        "unknown" => Some(OutlineNodeKind::Unknown),
        _ => None,
    }
}

fn body_kind_to_str(kind: OutlineBodyKind) -> &'static str {
    match kind {
        OutlineBodyKind::Brace => "brace",
        OutlineBodyKind::Indent => "indent",
        OutlineBodyKind::EndKeyword => "end-keyword",
        OutlineBodyKind::None => "none",
    }
}

fn parse_body_kind(value: &str) -> Option<OutlineBodyKind> {
    match value {
        "brace" => Some(OutlineBodyKind::Brace),
        "indent" => Some(OutlineBodyKind::Indent),
        "end-keyword" => Some(OutlineBodyKind::EndKeyword),
        "none" => Some(OutlineBodyKind::None),
        _ => None,
    }
}

fn name_capture_to_str(name: OutlineNameCapture) -> &'static str {
    match name {
        OutlineNameCapture::AfterKeyword => "after-keyword",
        OutlineNameCapture::BeforeParameters => "before-parameters",
    }
}

fn parse_name_capture(value: &str) -> Option<OutlineNameCapture> {
    match value {
        "after-keyword" => Some(OutlineNameCapture::AfterKeyword),
        "before-parameters" => Some(OutlineNameCapture::BeforeParameters),
        _ => None,
    }
}

fn scan_mode_to_str(scan: OutlineScanMode) -> &'static str {
    match scan {
        OutlineScanMode::Keyword => "keyword",
        OutlineScanMode::Callable => "callable",
    }
}

fn parse_scan_mode(value: &str) -> Option<OutlineScanMode> {
    match value {
        "keyword" => Some(OutlineScanMode::Keyword),
        "callable" => Some(OutlineScanMode::Callable),
        _ => None,
    }
}

fn diagnostic_severity_to_str(severity: OutlineDiagnosticSeverity) -> &'static str {
    match severity {
        OutlineDiagnosticSeverity::Info => "info",
        OutlineDiagnosticSeverity::Warning => "warning",
        OutlineDiagnosticSeverity::Error => "error",
    }
}

fn parse_diagnostic_severity(value: &str) -> Option<OutlineDiagnosticSeverity> {
    match value {
        "info" => Some(OutlineDiagnosticSeverity::Info),
        "warning" => Some(OutlineDiagnosticSeverity::Warning),
        "error" => Some(OutlineDiagnosticSeverity::Error),
        _ => None,
    }
}
