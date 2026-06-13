use super::adapters::OutlineLanguageAdapter;
use super::callable_statements::{
    CallableStatement, CallableStatementTerminator, callable_statements,
};
use super::fsm::{ByteRange, DeclarationEvent, StructuralEvent, StructuralEventKind};
use super::lexical::OutlineCodeMask;
use super::scan::*;
pub(super) use super::structure_support::{
    container_depth, containing_container, declaration_depth, editor_range, indent_depth_before,
};
use super::{OutlineBodyKind, OutlineNameCapture, OutlinePlan, OutlineRulePlan, OutlineScanMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructurePassOutput {
    pub containers: Vec<StructuralEvent>,
    pub declarations: Vec<DeclarationEvent>,
}

pub(super) fn discover_structure(
    text: &str,
    mask: &OutlineCodeMask,
    plan: &OutlinePlan,
) -> StructurePassOutput {
    let mut containers = Vec::new();
    let mut declarations = Vec::new();

    for rule in &plan.containers {
        containers.extend(container_events_for_rule(text, mask, plan, rule));
    }

    for rule in &plan.declarations {
        declarations.extend(declaration_events_for_rule(
            text,
            mask,
            plan,
            rule,
            &containers,
        ));
    }

    containers.sort_by_key(|event| (event.signature_range.start, event.signature_range.end));
    declarations.sort_by_key(|event| (event.signature_range.start, event.signature_range.end));

    StructurePassOutput {
        containers,
        declarations,
    }
}

fn container_events_for_rule(
    text: &str,
    mask: &OutlineCodeMask,
    plan: &OutlinePlan,
    rule: &OutlineRulePlan,
) -> Vec<StructuralEvent> {
    let mut events = Vec::new();
    let mut cursor = 0;

    while let Some(keyword_offset) = find_keyword_sequence(text, mask, cursor, &rule.keyword) {
        if let Some(event) = container_at(text, mask, plan, rule, keyword_offset) {
            events.push(event);
        }
        cursor = keyword_offset + rule.keyword[0].len();
    }

    events
}

fn declaration_events_for_rule(
    text: &str,
    mask: &OutlineCodeMask,
    plan: &OutlinePlan,
    rule: &OutlineRulePlan,
    containers: &[StructuralEvent],
) -> Vec<DeclarationEvent> {
    if rule.scan == OutlineScanMode::Callable {
        return callable_declaration_events_for_rule(text, mask, rule, containers);
    }

    let mut events = Vec::new();
    let mut cursor = 0;

    while let Some(keyword_offset) = find_keyword_sequence(text, mask, cursor, &rule.keyword) {
        if let Some(event) = declaration_at(text, mask, plan, rule, keyword_offset) {
            events.push(event);
        }
        cursor = keyword_offset + rule.keyword[0].len();
    }

    events
}

fn callable_declaration_events_for_rule(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    containers: &[StructuralEvent],
) -> Vec<DeclarationEvent> {
    let mut events = Vec::new();
    let statements = callable_statements(
        text,
        mask,
        &rule.callable.operator_tokens,
        &rule.callable.control_headers,
    );
    let mut statement_index = 0;
    let mut cursor = 0;

    while let Some(open_paren) = find_next_code_char(text, mask, cursor, '(') {
        while statement_index < statements.len()
            && statements[statement_index].range.end <= open_paren
        {
            statement_index += 1;
        }
        let Some(statement) = statements.get(statement_index).filter(|statement| {
            statement.range.start <= open_paren && open_paren < statement.range.end
        }) else {
            cursor = open_paren + 1;
            continue;
        };

        if statement.is_expression_context {
            cursor = open_paren + 1;
            continue;
        }

        if let Some(event) =
            callable_declaration_at(text, mask, rule, containers, statement, open_paren)
        {
            events.push(event);
        }
        cursor = open_paren + 1;
    }

    events
}

fn container_at(
    text: &str,
    mask: &OutlineCodeMask,
    plan: &OutlinePlan,
    rule: &OutlineRulePlan,
    keyword_offset: usize,
) -> Option<StructuralEvent> {
    let keyword_end = keyword_sequence_end(text, mask, keyword_offset, &rule.keyword)?;
    let name_range = name_range_after(text, mask, rule, keyword_end)
        .unwrap_or(ByteRange::new(keyword_end, keyword_end));
    let adapter = OutlineLanguageAdapter::for_plan(plan);
    let terminator = signature_terminator(text, mask, adapter, rule, name_range.end)?;
    let (body_range, signature_end) = match terminator {
        RuleTerminator::Body { open, close } => (Some(ByteRange::new(open, close + 1)), close + 1),
        RuleTerminator::Line { end } if rule.body == OutlineBodyKind::Indent => (
            Some(ByteRange::new(
                end,
                indent_body_end(text, keyword_offset, end),
            )),
            end,
        ),
        RuleTerminator::Line { end } => (None, end),
        RuleTerminator::Declaration { end } => (None, end),
    };

    Some(StructuralEvent {
        kind: StructuralEventKind::Body {
            owner_kind: rule.node_kind,
            body_kind: rule.body,
        },
        keyword_range: ByteRange::new(keyword_offset, keyword_end),
        name_range,
        signature_range: ByteRange::new(keyword_offset, signature_end),
        body_range,
    })
}

fn declaration_at(
    text: &str,
    mask: &OutlineCodeMask,
    plan: &OutlinePlan,
    rule: &OutlineRulePlan,
    keyword_offset: usize,
) -> Option<DeclarationEvent> {
    let keyword_end = keyword_sequence_end(text, mask, keyword_offset, &rule.keyword)?;
    let name_range = name_range_after(text, mask, rule, keyword_end)?;
    if previous_code_token(text, mask, keyword_offset)
        .is_some_and(|token| matches!(token.text(text), "(" | ","))
    {
        return None;
    }
    let adapter = OutlineLanguageAdapter::for_plan(plan);
    let signature_start = signature_start(text, mask, adapter, rule, keyword_offset);
    let terminator = signature_terminator(text, mask, adapter, rule, name_range.end)?;
    let (body_range, signature_end, terminated) = match terminator {
        RuleTerminator::Body { open, close } => {
            (Some(ByteRange::new(open, close + 1)), close + 1, false)
        }
        RuleTerminator::Line { end } if rule.body == OutlineBodyKind::Indent => {
            let body_end = indent_body_end(text, keyword_offset, end);
            (Some(ByteRange::new(end, body_end)), body_end, false)
        }
        RuleTerminator::Line { end } => (None, end, false),
        RuleTerminator::Declaration { end } => (None, end, true),
    };

    Some(DeclarationEvent {
        rule: rule.clone(),
        name: text[name_range.start..name_range.end].to_owned(),
        name_range,
        signature_range: ByteRange::new(signature_start, signature_end),
        body_range,
        terminated,
    })
}

fn callable_declaration_at(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    containers: &[StructuralEvent],
    statement: &CallableStatement,
    open_paren: usize,
) -> Option<DeclarationEvent> {
    let name_range = callable_name_before_parameters(text, mask, rule, open_paren)?;
    let name = text.get(name_range.start..name_range.end)?;
    if rule
        .callable
        .reject_names
        .iter()
        .any(|reject| reject == name)
    {
        return None;
    }
    if callable_is_rejected_by_previous_token(text, mask, rule, name_range.start) {
        return None;
    }
    if callable_is_rejected_by_prefix(text, mask, rule, name_range.start) {
        return None;
    }
    if !callable_has_required_previous_token(text, mask, rule, name_range.start) {
        return None;
    }
    if !callable_has_required_non_container_previous_token(text, mask, rule, containers, name_range)
    {
        return None;
    }
    let close_paren = matching_code_paren_after(text, mask, open_paren)?;
    let signature_start = callable_signature_start(text, mask, rule, statement, name_range.start);
    let terminator = callable_signature_terminator(text, mask, rule, statement, close_paren + 1)?;
    let (body_range, signature_end, terminated) = match terminator {
        RuleTerminator::Body { open, close } => {
            (Some(ByteRange::new(open, close + 1)), close + 1, false)
        }
        RuleTerminator::Declaration { end } => (None, end, true),
        RuleTerminator::Line { end } => (None, end, false),
    };

    Some(DeclarationEvent {
        rule: rule.clone(),
        name: name.to_owned(),
        name_range,
        signature_range: ByteRange::new(signature_start, signature_end),
        body_range,
        terminated,
    })
}

fn name_range_after(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    after_keyword: usize,
) -> Option<ByteRange> {
    match rule.name {
        OutlineNameCapture::AfterKeyword => {
            let start = skip_non_code_whitespace(text, mask, after_keyword);
            parse_identifier_range(text, start)
        }
        OutlineNameCapture::BeforeParameters => None,
    }
}

fn callable_name_before_parameters(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    open_paren: usize,
) -> Option<ByteRange> {
    if let Some(name) = callable_compound_name_before_parameters(text, mask, rule, open_paren) {
        return Some(name);
    }

    let token = previous_contiguous_code_token(text, mask, open_paren)?;
    let token_text = token.text(text);
    if rule
        .callable
        .compound_prefixes
        .iter()
        .any(|prefix| token_text == prefix)
    {
        return None;
    }
    if let Some(prefix) = rule
        .callable
        .compound_prefixes
        .iter()
        .find(|prefix| token_text.ends_with(prefix.as_str()))
    {
        let prefix = ByteRange::new(token.end - prefix.len(), token.end);
        return callable_operator_name(text, mask, rule, prefix, open_paren);
    }

    if rule
        .callable
        .container_name_previous
        .iter()
        .any(|previous| previous == token_text)
    {
        return Some(ByteRange::new(token.start, token.end));
    }

    let name = callable_suffix_name_range(text, mask, rule, token)?;
    if name.start == name.end {
        return None;
    }
    Some(name)
}

fn callable_compound_name_before_parameters(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    open_paren: usize,
) -> Option<ByteRange> {
    if rule.callable.compound_prefixes.is_empty() || rule.callable.operator_tokens.is_empty() {
        return None;
    }

    let mut cursor = open_paren;
    loop {
        cursor = skip_code_whitespace_before(text, mask, cursor);
        if cursor == 0 {
            return None;
        }
        if let Some(prefix) = compound_prefix_before(text, mask, rule, cursor) {
            return (cursor <= open_paren)
                .then(|| callable_operator_name(text, mask, rule, prefix, open_paren))
                .flatten();
        }
        let Some(start) = operator_token_before(text, mask, rule, cursor) else {
            return None;
        };
        cursor = start;
    }
}

fn callable_suffix_name_range(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    token: CodeToken,
) -> Option<ByteRange> {
    let token_text = token.text(text);
    let end = token.end;
    let mut cursor = end;
    let mut prefix_start = None;

    if let Some(separator) = rule
        .callable
        .qualified_separators
        .iter()
        .filter_map(|separator| {
            token_text
                .rfind(separator)
                .map(|index| (separator.as_str(), index))
        })
        .max_by_key(|(_, index)| *index)
    {
        if !qualified_separator_has_prefix(token_text, separator.1) {
            return None;
        }
        cursor = token.start + separator.1 + separator.0.len();
    } else {
        for (relative, ch) in token_text.char_indices().rev() {
            let offset = token.start + relative;
            if !is_identifier_char(ch) {
                break;
            }
            cursor = offset;
        }
    }
    for prefix in &rule.callable.name_prefixes {
        if cursor >= token.start + prefix.len()
            && text.get(cursor - prefix.len()..cursor) == Some(prefix.as_str())
        {
            prefix_start = Some(cursor - prefix.len());
            break;
        }
        if code_before_ends_with(text, mask, token.start, prefix) {
            prefix_start = Some(previous_code_sequence_start(
                text,
                mask,
                token.start,
                prefix,
            ));
            break;
        }
    }

    (cursor < end).then_some(ByteRange::new(prefix_start.unwrap_or(cursor), end))
}

fn compound_prefix_before(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    before: usize,
) -> Option<ByteRange> {
    let token = previous_code_token(text, mask, before)?;
    let token_text = token.text(text);
    rule.callable
        .compound_prefixes
        .iter()
        .find(|prefix| token_text.ends_with(prefix.as_str()))
        .map(|prefix| ByteRange::new(token.end - prefix.len(), token.end))
}

fn operator_token_before(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    before: usize,
) -> Option<usize> {
    rule.callable.operator_tokens.iter().find_map(|token| {
        code_before_ends_with(text, mask, before, token)
            .then(|| previous_code_sequence_start(text, mask, before, token))
    })
}

fn callable_operator_name(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    operator: ByteRange,
    open_paren: usize,
) -> Option<ByteRange> {
    let mut cursor = operator.end;
    while cursor < open_paren {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        if !mask.is_code(cursor) {
            cursor += ch.len_utf8();
            continue;
        }
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            continue;
        }
        if ch == '(' && matching_code_paren_after(text, mask, cursor) == Some(open_paren) {
            return Some(ByteRange::new(operator.start, open_paren));
        }
        if let Some(token) = rule
            .callable
            .operator_tokens
            .iter()
            .find(|token| text[cursor..].starts_with(token.as_str()))
        {
            cursor += token.len();
            if cursor > open_paren {
                return None;
            }
            continue;
        }
        return None;
    }

    Some(ByteRange::new(operator.start, open_paren))
}

fn callable_is_rejected_by_previous_token(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    let Some(previous) = previous_contiguous_code_token(text, mask, name_start) else {
        return false;
    };
    let previous_text = previous.text(text);
    rule.callable
        .reject_previous
        .iter()
        .any(|reject| reject == previous_text)
        || previous_code_token(text, mask, name_start).is_some_and(|previous| {
            let previous_text = previous.text(text);
            rule.callable
                .reject_previous
                .iter()
                .any(|reject| reject == previous_text)
        })
}

fn callable_is_rejected_by_prefix(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    rule.callable
        .reject_prefixes
        .iter()
        .any(|prefix| code_before_ends_with(text, mask, name_start, prefix))
}

fn code_before_ends_with(text: &str, mask: &OutlineCodeMask, before: usize, suffix: &str) -> bool {
    let mut cursor = before;

    for expected in suffix.chars().rev() {
        let Some(offset) = previous_code_char(text, mask, cursor) else {
            return false;
        };
        if text[offset..].chars().next() != Some(expected) {
            return false;
        }
        cursor = offset;
    }

    true
}

fn previous_code_sequence_start(
    text: &str,
    mask: &OutlineCodeMask,
    before: usize,
    sequence: &str,
) -> usize {
    let mut cursor = before;

    for _ in sequence.chars().rev() {
        let Some(offset) = previous_code_char(text, mask, cursor) else {
            break;
        };
        cursor = offset;
    }

    cursor
}

fn callable_has_required_previous_token(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    if rule.callable.require_previous.is_empty() {
        return true;
    }
    let Some(previous) = previous_contiguous_code_token(text, mask, name_start) else {
        return false;
    };
    let previous_text = previous.text(text);
    rule.callable
        .require_previous
        .iter()
        .any(|required| required == previous_text)
}

fn callable_has_required_non_container_previous_token(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    containers: &[StructuralEvent],
    name_range: ByteRange,
) -> bool {
    if rule.callable.require_non_container_previous.is_empty()
        && rule.callable.require_non_container_previous_kind.is_empty()
        || callable_has_container_name_previous_token(text, mask, rule, name_range.start)
        || callable_name_matches_containing_container(text, containers, rule, name_range)
        || callable_name_has_qualified_separator(text, mask, rule, name_range.start)
    {
        return true;
    }
    let Some(previous) = previous_contiguous_code_token(text, mask, name_range.start) else {
        return false;
    };
    let previous_text = previous.text(text);
    rule.callable
        .require_non_container_previous
        .iter()
        .any(|required| required == previous_text)
        || rule
            .callable
            .require_non_container_previous_kind
            .iter()
            .any(|required| token_matches_required_kind(text, mask, previous, required))
}

fn token_matches_required_kind(
    text: &str,
    mask: &OutlineCodeMask,
    token: CodeToken,
    required: &str,
) -> bool {
    let token_text = token.text(text);
    match required {
        "identifier" => token_text.chars().all(is_identifier_char),
        "qualified-identifier" => is_qualified_identifier(token_text),
        "template-type-tail" => {
            token_text.ends_with('>')
                && template_type_prefix_before_tail(text, mask, token).is_some_and(|previous| {
                    let previous_text = previous.text(text);
                    is_qualified_identifier(previous_text)
                        || token_matches_required_kind(text, mask, previous, "template-type-tail")
                })
        }
        _ => false,
    }
}

fn template_type_prefix_before_tail(
    text: &str,
    mask: &OutlineCodeMask,
    token: CodeToken,
) -> Option<CodeToken> {
    let token_text = token.text(text);
    if token_text == ">" {
        return matching_code_angle_before(text, mask, token.start)
            .and_then(|open| previous_contiguous_code_token(text, mask, open));
    }

    let open = matching_angle_in_token(token_text)?;
    let prefix_end = token.start + open;
    (prefix_end > token.start).then_some(CodeToken {
        start: token.start,
        end: prefix_end,
    })
}

fn matching_angle_in_token(token: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in token.char_indices().rev() {
        match ch {
            '>' => depth += 1,
            '<' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn is_qualified_identifier(value: &str) -> bool {
    value.split("::").all(|part| {
        !part.is_empty()
            && part.chars().next().is_some_and(is_identifier_start)
            && part.chars().all(is_identifier_char)
    })
}

fn callable_name_matches_containing_container(
    text: &str,
    containers: &[StructuralEvent],
    rule: &OutlineRulePlan,
    name_range: ByteRange,
) -> bool {
    if rule.callable.container_name_previous.is_empty() {
        return false;
    }
    let Some(name) = text.get(name_range.start..name_range.end) else {
        return false;
    };
    let unprefixed_name = rule
        .callable
        .name_prefixes
        .iter()
        .find_map(|prefix| name.strip_prefix(prefix))
        .unwrap_or(name);

    containers
        .iter()
        .filter(|container| {
            container
                .body_range
                .is_some_and(|body| body.start <= name_range.start && name_range.start < body.end)
        })
        .any(|container| {
            text.get(container.name_range.start..container.name_range.end) == Some(unprefixed_name)
        })
}

fn callable_name_has_qualified_separator(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    rule.callable.qualified_separators.iter().any(|separator| {
        code_before_ends_with(text, mask, name_start, separator)
            && qualified_prefix_before_separator(text, mask, name_start, separator)
    })
}

fn qualified_prefix_before_separator(
    text: &str,
    mask: &OutlineCodeMask,
    name_start: usize,
    separator: &str,
) -> bool {
    let separator_start = previous_code_sequence_start(text, mask, name_start, separator);
    let Some(prefix) = previous_contiguous_code_token(text, mask, separator_start) else {
        return false;
    };

    let prefix_text = prefix.text(text);
    prefix_text
        .chars()
        .next_back()
        .is_some_and(is_identifier_char)
}

fn qualified_separator_has_prefix(token: &str, separator_index: usize) -> bool {
    separator_index > 0
        && token[..separator_index]
            .chars()
            .next_back()
            .is_some_and(is_identifier_char)
}

fn callable_has_container_name_previous_token(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    let Some(previous) = previous_contiguous_code_token(text, mask, name_start) else {
        return false;
    };
    let previous_text = previous.text(text);
    rule.callable
        .container_name_previous
        .iter()
        .any(|container| container == previous_text)
}

fn callable_signature_start(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    statement: &CallableStatement,
    name_start: usize,
) -> usize {
    let mut start = name_start;
    let mut cursor = name_start;

    while let Some(token) = previous_code_token(text, mask, cursor) {
        let token_text = token.text(text);
        if rule
            .callable
            .start_boundaries
            .iter()
            .any(|boundary| boundary == token_text)
            || token.start < statement.range.start
        {
            break;
        }
        start = token.start;
        cursor = token.start;
    }

    line_start_offset(text, start).max(start)
}

fn callable_signature_terminator(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    statement: &CallableStatement,
    after_parameters: usize,
) -> Option<RuleTerminator> {
    let mut cursor = after_parameters;
    let mut angle_depth = 0usize;
    if next_code_token(text, mask, cursor).is_some_and(|token| token.text(text) == "(") {
        return None;
    }

    while cursor < statement.range.end {
        if !mask.is_code(cursor) {
            cursor += next_char_len(text, cursor);
            continue;
        }
        let ch = text[cursor..].chars().next()?;
        match ch {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            ';' if angle_depth == 0 && rule.declaration_terminator.as_deref() == Some(";") => {
                return (matches!(statement.terminator, CallableStatementTerminator::Semicolon)
                    && cursor + 1 <= statement.range.end)
                    .then_some(RuleTerminator::Declaration { end: cursor + 1 });
            }
            '{' if angle_depth == 0 => {
                if !matches!(statement.terminator, CallableStatementTerminator::Body) {
                    return None;
                }
                let close = matching_code_brace(text, mask, cursor)?;
                return Some(RuleTerminator::Body {
                    open: cursor,
                    close,
                });
            }
            '\r' | '\n' if angle_depth == 0 => {
                if rule.declaration_terminator.as_deref() == Some("line") {
                    return Some(RuleTerminator::Declaration { end: cursor });
                }
                let line_end = line_end_offset(text, cursor);
                let tail = text.get(cursor..line_end).unwrap_or("");
                if !tail.trim().is_empty() {
                    return None;
                }
            }
            '=' if angle_depth == 0 => {
                if text[cursor..].starts_with("=>") {
                    return None;
                }
                if let Some(next) = next_code_token(text, mask, cursor + ch.len_utf8()) {
                    if rule
                        .callable
                        .assignment_continuations
                        .iter()
                        .any(|continuation| continuation == next.text(text))
                    {
                        cursor = next.end;
                        continue;
                    }
                }
                return None;
            }
            _ => {}
        }
        cursor += ch.len_utf8();
    }

    None
}

fn signature_terminator(
    text: &str,
    mask: &OutlineCodeMask,
    adapter: OutlineLanguageAdapter,
    rule: &OutlineRulePlan,
    after_name: usize,
) -> Option<RuleTerminator> {
    match rule.body {
        OutlineBodyKind::Brace => delimited_signature_terminator(text, mask, rule, after_name),
        OutlineBodyKind::Indent => indent_signature_terminator(text, after_name),
        OutlineBodyKind::EndKeyword => {
            end_keyword_signature_terminator(text, mask, adapter, after_name)
        }
        OutlineBodyKind::None => Some(RuleTerminator::Line {
            end: line_end_offset(text, after_name),
        }),
    }
}

fn delimited_signature_terminator(
    text: &str,
    mask: &OutlineCodeMask,
    rule: &OutlineRulePlan,
    after_name: usize,
) -> Option<RuleTerminator> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut cursor = after_name;

    while cursor < text.len() {
        if !mask.is_code(cursor) {
            cursor += next_char_len(text, cursor);
            continue;
        }

        let ch = text[cursor..].chars().next()?;
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            ';' if paren_depth == 0
                && bracket_depth == 0
                && angle_depth == 0
                && rule.declaration_terminator.as_deref() == Some(";") =>
            {
                return Some(RuleTerminator::Declaration { end: cursor + 1 });
            }
            '\r' | '\n'
                if paren_depth == 0
                    && bracket_depth == 0
                    && angle_depth == 0
                    && rule.declaration_terminator.as_deref() == Some("line") =>
            {
                return Some(RuleTerminator::Declaration { end: cursor });
            }
            '{' if paren_depth == 0 && bracket_depth == 0 && angle_depth == 0 => {
                let close = matching_code_brace(text, mask, cursor)?;
                return Some(RuleTerminator::Body {
                    open: cursor,
                    close,
                });
            }
            _ => {}
        }

        cursor += ch.len_utf8();
    }

    None
}

fn indent_signature_terminator(text: &str, after_name: usize) -> Option<RuleTerminator> {
    Some(RuleTerminator::Line {
        end: line_end_offset(text, after_name),
    })
}

fn indent_body_end(text: &str, header_offset: usize, after_header: usize) -> usize {
    let header_indent = indentation_before(text, header_offset);
    let mut cursor = next_line_start_offset(text, after_header);

    while cursor < text.len() {
        let line_end = line_end_offset(text, cursor);
        let line = text.get(cursor..line_end).unwrap_or("");
        if !line.trim().is_empty() {
            let indent = indentation_before(text, cursor + leading_whitespace_len(line));
            if indent <= header_indent {
                return cursor.saturating_sub(line_ending_len_before(text, cursor));
            }
        }
        let next = next_line_start_offset(text, line_end);
        if next <= cursor {
            break;
        }
        cursor = next;
    }

    text.len()
}

fn end_keyword_signature_terminator(
    text: &str,
    mask: &OutlineCodeMask,
    adapter: OutlineLanguageAdapter,
    after_name: usize,
) -> Option<RuleTerminator> {
    let line_end = line_end_offset(text, after_name);
    let close = matching_end_keyword(text, mask, adapter, after_name).unwrap_or(line_end);

    Some(RuleTerminator::Body {
        open: line_end,
        close,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleTerminator {
    Body { open: usize, close: usize },
    Line { end: usize },
    Declaration { end: usize },
}

#[cfg(test)]
#[path = "structure_tests.rs"]
mod tests;
