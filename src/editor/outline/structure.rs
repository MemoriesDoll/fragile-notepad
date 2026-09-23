use super::callable_statements::{
    CallableStatement, CallableStatementTerminator, callable_statements,
};
use super::fsm::{ByteRange, DeclarationEvent, StructuralEvent, StructuralEventKind};
use super::scan::*;
use super::source::{OutlineSource, SyntaxSymbol};
#[cfg(test)]
pub(super) use super::structure_support::containing_container;
use super::{OutlineBodyKind, OutlineNameCapture, OutlinePlan, OutlineRulePlan, OutlineScanMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructurePassOutput {
    pub containers: Vec<StructuralEvent>,
    pub declarations: Vec<DeclarationEvent>,
}

pub(super) fn discover_structure(
    text: &str,
    source: &OutlineSource,
    plan: &OutlinePlan,
) -> StructurePassOutput {
    let mut containers = Vec::new();
    let mut declarations = Vec::new();

    for rule in &plan.containers {
        containers.extend(container_events_for_rule(text, source, rule));
    }
    // Overlapping XML keywords can describe the same body. Prefer the earliest
    // header and, at the same start, the longest prefix before the name.
    containers.sort_by_key(|event| {
        (
            event.signature_range.start,
            std::cmp::Reverse(event.name_range.start),
        )
    });
    let mut container_bodies = std::collections::HashSet::new();
    containers.retain(|event| {
        let extent = event.body_range.unwrap_or(ByteRange::new(
            event.signature_range.end,
            event.signature_range.end,
        ));
        container_bodies.insert((extent.start, extent.end))
    });
    let container_names = ContainerNames::new(text, &containers);

    for rule in &plan.declarations {
        declarations.extend(declaration_events_for_rule(
            text,
            source,
            rule,
            &container_names,
        ));
    }

    containers.sort_by_key(|event| (event.signature_range.start, event.signature_range.end));
    declarations.sort_by_key(|event| (event.signature_range.start, event.signature_range.end));
    // Overlapping keyword rules (e.g. a modifier plus declaration keyword) describe
    // one declaration. Remove duplicates before they contribute nesting intervals.
    let mut seen = std::collections::HashSet::new();
    declarations.retain(|event| seen.insert((event.name_range.start, event.signature_range.end)));

    StructurePassOutput {
        containers,
        declarations,
    }
}

fn container_events_for_rule(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
) -> Vec<StructuralEvent> {
    let mut events = Vec::new();
    let mut cursor = 0;

    while let Some(keyword_offset) = find_keyword_sequence(text, source, cursor, &rule.keyword) {
        if let Some(event) = container_at(text, source, rule, keyword_offset) {
            events.push(event);
        }
        cursor = keyword_offset + rule.keyword[0].len();
    }

    events
}

fn declaration_events_for_rule(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    containers: &ContainerNames,
) -> Vec<DeclarationEvent> {
    if rule.scan == OutlineScanMode::Callable {
        let statements = callable_statements(
            text,
            source,
            &rule.callable.operator_tokens,
            &rule.callable.control_headers,
        );
        let mut events =
            callable_declaration_events_for_rule(text, source, rule, containers, &statements);
        if rule.callable.assignment_arrow.is_some() {
            events.extend(arrow_function_declaration_events_for_rule(
                text,
                source,
                rule,
                containers,
                &statements,
            ));
            events.sort_by_key(|event| (event.signature_range.start, event.signature_range.end));
        }
        return events;
    }

    let mut events = Vec::new();
    let mut cursor = 0;

    while let Some(keyword_offset) = find_keyword_sequence(text, source, cursor, &rule.keyword) {
        if let Some(event) = declaration_at(text, source, rule, keyword_offset) {
            events.push(event);
        }
        cursor = keyword_offset + rule.keyword[0].len();
    }

    events
}

fn callable_declaration_events_for_rule(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    containers: &ContainerNames,
    statements: &[CallableStatement],
) -> Vec<DeclarationEvent> {
    let mut events = Vec::new();
    let mut statement_index = 0;
    let mut cursor = 0;

    let Some(parameters_open) = source.symbol_char(SyntaxSymbol::ParametersOpen) else {
        return events;
    };
    while let Some(open_paren) = find_next_code_char(text, source, cursor, parameters_open) {
        while statement_index < statements.len()
            && statements[statement_index].range.end <= open_paren
        {
            statement_index += 1;
        }
        let Some(statement) = statements.get(statement_index).filter(|statement| {
            statement.range.start <= open_paren && open_paren < statement.range.end
        }) else {
            cursor = open_paren + parameters_open.len_utf8();
            continue;
        };

        if statement.is_expression_context {
            cursor = open_paren + parameters_open.len_utf8();
            continue;
        }

        if let Some(event) =
            callable_declaration_at(text, source, rule, containers, statement, open_paren)
        {
            events.push(event);
        }
        cursor = open_paren + parameters_open.len_utf8();
    }

    events
}

fn arrow_function_declaration_events_for_rule(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    containers: &ContainerNames,
    statements: &[CallableStatement],
) -> Vec<DeclarationEvent> {
    let mut events = Vec::new();

    for statement in statements
        .iter()
        .filter(|statement| statement.is_expression_context)
    {
        for arrow in top_level_arrow_offsets(
            text,
            source,
            statement.range,
            rule.callable.assignment_arrow.as_deref().unwrap_or(""),
        ) {
            if let Some(event) =
                arrow_function_declaration_at(text, source, rule, containers, statement, arrow)
            {
                events.push(event);
            }
        }
    }

    events
}

fn container_at(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    keyword_offset: usize,
) -> Option<StructuralEvent> {
    let keyword_end = keyword_sequence_end(text, source, keyword_offset, &rule.keyword)?;
    let name_range = name_range_after(text, source, rule, keyword_end)
        .unwrap_or(ByteRange::new(keyword_end, keyword_end));
    let terminator = signature_terminator(text, source, rule, name_range.end)?;
    let (body_range, signature_end) = match terminator {
        RuleTerminator::Body { open, end } => (Some(ByteRange::new(open, end)), end),
        RuleTerminator::Line { end } if rule.body == OutlineBodyKind::Indent => {
            let body_end = indent_body_end(text, source, keyword_offset, end);
            (Some(ByteRange::new(end, body_end)), body_end)
        }
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    keyword_offset: usize,
) -> Option<DeclarationEvent> {
    let keyword_end = keyword_sequence_end(text, source, keyword_offset, &rule.keyword)?;
    let name_range = name_range_after(text, source, rule, keyword_end)?;
    if previous_code_token(text, source, keyword_offset).is_some_and(|token| {
        matches!(
            source.symbol_text(token.text(text)),
            SyntaxSymbol::ParametersOpen | SyntaxSymbol::Separator
        )
    }) {
        return None;
    }
    let signature_start = signature_start(text, source, keyword_offset);
    let terminator = signature_terminator(text, source, rule, name_range.end)?;
    let (body_range, signature_end, terminated) = match terminator {
        RuleTerminator::Body { open, end } => (Some(ByteRange::new(open, end)), end, false),
        RuleTerminator::Line { end } if rule.body == OutlineBodyKind::Indent => {
            let body_end = indent_body_end(text, source, keyword_offset, end);
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    containers: &ContainerNames,
    statement: &CallableStatement,
    open_paren: usize,
) -> Option<DeclarationEvent> {
    let name_range = callable_name_before_parameters(text, source, rule, open_paren)?;
    let name = text.get(name_range.start..name_range.end)?;
    if rule
        .callable
        .reject_names
        .iter()
        .any(|reject| reject == name)
    {
        return None;
    }
    if callable_is_rejected_by_previous_token(text, source, rule, name_range.start) {
        return None;
    }
    if callable_is_rejected_by_prefix(text, source, rule, name_range.start) {
        return None;
    }
    if !callable_has_required_previous_token(text, source, rule, name_range.start) {
        return None;
    }
    if !callable_has_required_non_container_previous_token(
        text, source, rule, containers, name_range,
    ) {
        return None;
    }
    let close_paren = matching_code_paren_after(text, source, open_paren)?;
    let signature_start = callable_signature_start(text, source, rule, statement, name_range.start);
    let terminator = callable_signature_terminator(
        text,
        source,
        rule,
        statement,
        source.next_token(close_paren)?.end,
    )?;
    let (body_range, signature_end, terminated) = match terminator {
        RuleTerminator::Body { open, end } => (Some(ByteRange::new(open, end)), end, false),
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

fn arrow_function_declaration_at(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    containers: &ContainerNames,
    statement: &CallableStatement,
    arrow: usize,
) -> Option<DeclarationEvent> {
    let assignment = arrow_assignment_before(text, source, statement.range.start, arrow)?;
    let token = previous_contiguous_code_token(text, source, assignment)?;
    let name_range = callable_suffix_name_range(text, source, rule, token)?;
    let name = text.get(name_range.start..name_range.end)?;
    if name.is_empty()
        || rule
            .callable
            .reject_names
            .iter()
            .any(|reject| reject == name)
    {
        return None;
    }
    if callable_is_rejected_by_prefix(text, source, rule, name_range.start) {
        return None;
    }
    if !callable_has_required_non_container_previous_token(
        text, source, rule, containers, name_range,
    ) {
        return None;
    }

    let body_open = next_code_token(
        text,
        source,
        arrow + rule.callable.assignment_arrow.as_ref()?.len(),
    )
    .filter(|token| source.is_body_open(text, token.start))
    .map(|token| token.start)?;
    if body_open >= statement.range.end {
        return None;
    }
    let body_close = matching_code_brace(text, source, body_open)?;

    Some(DeclarationEvent {
        rule: rule.clone(),
        name: name.to_owned(),
        name_range,
        signature_range: ByteRange::new(statement.range.start, source.next_token(body_close)?.end),
        body_range: Some(ByteRange::new(
            body_open,
            source.next_token(body_close)?.end,
        )),
        terminated: false,
    })
}

fn name_range_after(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    after_keyword: usize,
) -> Option<ByteRange> {
    match rule.name {
        OutlineNameCapture::AfterKeyword => {
            let start = skip_non_code_whitespace(text, source, after_keyword);
            parse_identifier_range(text, source, start)
        }
        OutlineNameCapture::BeforeParameters => None,
    }
}

fn callable_name_before_parameters(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    open_paren: usize,
) -> Option<ByteRange> {
    if let Some(name) = callable_compound_name_before_parameters(text, source, rule, open_paren) {
        return Some(name);
    }

    let token = previous_contiguous_code_token(text, source, open_paren)?;
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
        return callable_operator_name(text, source, rule, prefix, open_paren);
    }

    if rule
        .callable
        .container_name_previous
        .iter()
        .any(|previous| previous == token_text)
    {
        return Some(ByteRange::new(token.start, token.end));
    }

    let name = callable_suffix_name_range(text, source, rule, token)?;
    if name.start == name.end {
        return None;
    }
    Some(name)
}

fn callable_compound_name_before_parameters(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    open_paren: usize,
) -> Option<ByteRange> {
    if rule.callable.compound_prefixes.is_empty() || rule.callable.operator_tokens.is_empty() {
        return None;
    }

    let mut cursor = open_paren;
    loop {
        cursor = skip_code_whitespace_before(text, source, cursor);
        if cursor == 0 {
            return None;
        }
        if let Some(prefix) = compound_prefix_before(text, source, rule, cursor) {
            return (cursor <= open_paren)
                .then(|| callable_operator_name(text, source, rule, prefix, open_paren))
                .flatten();
        }
        let Some(start) = operator_token_before(text, source, rule, cursor) else {
            return None;
        };
        cursor = start;
    }
}

fn callable_suffix_name_range(
    text: &str,
    source: &OutlineSource,
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
        if !qualified_separator_has_prefix(token_text, separator.1, source) {
            return None;
        }
        cursor = token.start + separator.1 + separator.0.len();
    } else {
        for (relative, ch) in token_text.char_indices().rev() {
            let offset = token.start + relative;
            if !source.is_word_char(ch) {
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
        if code_before_ends_with(text, source, token.start, prefix) {
            prefix_start = Some(previous_code_sequence_start(
                text,
                source,
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    before: usize,
) -> Option<ByteRange> {
    let token = previous_code_token(text, source, before)?;
    let token_text = token.text(text);
    rule.callable
        .compound_prefixes
        .iter()
        .find(|prefix| token_text.ends_with(prefix.as_str()))
        .map(|prefix| ByteRange::new(token.end - prefix.len(), token.end))
}

fn operator_token_before(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    before: usize,
) -> Option<usize> {
    rule.callable.operator_tokens.iter().find_map(|token| {
        code_before_ends_with(text, source, before, token)
            .then(|| previous_code_sequence_start(text, source, before, token))
    })
}

fn callable_operator_name(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    operator: ByteRange,
    open_paren: usize,
) -> Option<ByteRange> {
    let mut cursor = operator.end;
    while cursor < open_paren {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        if !source.is_code(cursor) {
            cursor += ch.len_utf8();
            continue;
        }
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            continue;
        }
        if source.symbol(ch) == SyntaxSymbol::ParametersOpen
            && matching_code_paren_after(text, source, cursor) == Some(open_paren)
        {
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    let Some(previous) = previous_contiguous_code_token(text, source, name_start) else {
        return false;
    };
    let previous_text = previous.text(text);
    rule.callable
        .reject_previous
        .iter()
        .any(|reject| reject == previous_text)
        || previous_code_token(text, source, name_start).is_some_and(|previous| {
            let previous_text = previous.text(text);
            rule.callable
                .reject_previous
                .iter()
                .any(|reject| reject == previous_text)
        })
}

fn callable_is_rejected_by_prefix(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    rule.callable
        .reject_prefixes
        .iter()
        .any(|prefix| code_before_ends_with(text, source, name_start, prefix))
}

fn arrow_assignment_before(
    text: &str,
    source: &OutlineSource,
    start: usize,
    arrow: usize,
) -> Option<usize> {
    let mut assignment = None;
    let mut cursor = start;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    while cursor < arrow {
        let ch = text[cursor..].chars().next()?;
        let len = ch.len_utf8();
        if !source.is_code(cursor) {
            cursor += len;
            continue;
        }

        match source.symbol(ch) {
            SyntaxSymbol::ParametersOpen => paren_depth += 1,
            SyntaxSymbol::ParametersClose => paren_depth = paren_depth.saturating_sub(1),
            SyntaxSymbol::BracketsOpen => bracket_depth += 1,
            SyntaxSymbol::BracketsClose => bracket_depth = bracket_depth.saturating_sub(1),
            SyntaxSymbol::BodyOpen => brace_depth += 1,
            SyntaxSymbol::BodyClose => brace_depth = brace_depth.saturating_sub(1),
            SyntaxSymbol::Assignment
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
            {
                if standalone_assignment_at(text, source, cursor) {
                    assignment = Some(cursor);
                }
            }
            _ => {}
        }

        cursor += len;
    }

    assignment
}

fn standalone_assignment_at(text: &str, source: &OutlineSource, offset: usize) -> bool {
    let Some(assignment) = source.symbol_char(SyntaxSymbol::Assignment) else {
        return false;
    };
    if !text[offset..].starts_with(assignment) {
        return false;
    }
    if text[offset + assignment.len_utf8()..]
        .chars()
        .next()
        .is_some_and(|ch| source.has_token_role(ch, "assignment-reject-after"))
    {
        return false;
    }
    previous_code_char(text, source, offset)
        .and_then(|previous| text[previous..].chars().next())
        .is_none_or(|ch| !source.has_token_role(ch, "assignment-reject-before"))
}

fn top_level_arrow_offsets(
    text: &str,
    source: &OutlineSource,
    range: ByteRange,
    arrow_token: &str,
) -> Vec<usize> {
    let mut arrows = Vec::new();
    if arrow_token.is_empty() {
        return arrows;
    }
    let mut cursor = range.start;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    while cursor < range.end {
        let Some(ch) = text[cursor..].chars().next() else {
            break;
        };
        let len = ch.len_utf8();
        if !source.is_code(cursor) {
            cursor += len;
            continue;
        }

        match source.symbol(ch) {
            SyntaxSymbol::ParametersOpen => paren_depth += 1,
            SyntaxSymbol::ParametersClose => paren_depth = paren_depth.saturating_sub(1),
            SyntaxSymbol::BracketsOpen => bracket_depth += 1,
            SyntaxSymbol::BracketsClose => bracket_depth = bracket_depth.saturating_sub(1),
            SyntaxSymbol::BodyOpen => brace_depth += 1,
            SyntaxSymbol::BodyClose => brace_depth = brace_depth.saturating_sub(1),
            _ if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                let arrow_end = cursor + arrow_token.len();
                if arrow_end <= range.end
                    && text.get(cursor..arrow_end) == Some(arrow_token)
                    && source.is_code_range(cursor, arrow_end)
                {
                    arrows.push(cursor);
                    cursor = arrow_end;
                    continue;
                }
            }
            _ => {}
        }

        cursor += len;
    }

    arrows
}

fn code_before_ends_with(text: &str, source: &OutlineSource, before: usize, suffix: &str) -> bool {
    let mut cursor = before;

    for expected in suffix.chars().rev() {
        let Some(offset) = previous_code_char(text, source, cursor) else {
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
    source: &OutlineSource,
    before: usize,
    sequence: &str,
) -> usize {
    let mut cursor = before;

    for _ in sequence.chars().rev() {
        let Some(offset) = previous_code_char(text, source, cursor) else {
            break;
        };
        cursor = offset;
    }

    cursor
}

fn callable_has_required_previous_token(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    if rule.callable.require_previous.is_empty() {
        return true;
    }
    let Some(previous) = previous_contiguous_code_token(text, source, name_start) else {
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    containers: &ContainerNames,
    name_range: ByteRange,
) -> bool {
    if rule.callable.require_non_container_previous.is_empty()
        && rule.callable.require_non_container_previous_kind.is_empty()
        || callable_has_container_name_previous_token(text, source, rule, name_range.start)
        || callable_name_matches_containing_container(text, containers, rule, name_range)
        || callable_name_has_qualified_separator(text, source, rule, name_range.start)
    {
        return true;
    }
    let Some(previous) = previous_contiguous_code_token(text, source, name_range.start) else {
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
            .any(|required| token_matches_required_kind(text, source, previous, required))
}

fn token_matches_required_kind(
    text: &str,
    source: &OutlineSource,
    token: CodeToken,
    required: &str,
) -> bool {
    let token_text = token.text(text);
    match required {
        "identifier" => token_text.chars().all(|ch| source.is_word_char(ch)),
        "qualified-identifier" => is_qualified_identifier(token_text, source),
        "template-type-tail" => {
            source
                .symbol_char(SyntaxSymbol::GenericsClose)
                .is_some_and(|close| token_text.ends_with(close))
                && template_type_prefix_before_tail(text, source, token).is_some_and(|previous| {
                    let previous_text = previous.text(text);
                    is_qualified_identifier(previous_text, source)
                        || token_matches_required_kind(text, source, previous, "template-type-tail")
                })
        }
        "array-type-tail" => {
            let Some(close) = previous_code_token(text, source, token.end).filter(|close| {
                source.symbol_text(close.text(text)) == SyntaxSymbol::BracketsClose
            }) else {
                return false;
            };
            let Some(open) = source.matching_delimiter(close.start) else {
                return false;
            };
            // Array type suffixes have empty bracket pairs, possibly with spacing.
            if source
                .next_token(open)
                .and_then(|open| source.next_token(open.end))
                != Some(close)
            {
                return false;
            }
            previous_contiguous_code_token(text, source, open).is_some_and(|prefix| {
                is_qualified_identifier(prefix.text(text), source)
                    || token_matches_required_kind(text, source, prefix, "template-type-tail")
                    || token_matches_required_kind(text, source, prefix, "array-type-tail")
            })
        }
        _ => false,
    }
}

fn template_type_prefix_before_tail(
    text: &str,
    source: &OutlineSource,
    token: CodeToken,
) -> Option<CodeToken> {
    let token_text = token.text(text);
    if source.symbol_text(token_text) == SyntaxSymbol::GenericsClose {
        return matching_code_angle_before(text, source, token.start)
            .and_then(|open| previous_contiguous_code_token(text, source, open));
    }

    let open = matching_angle_in_token(token_text, source)?;
    let prefix_end = token.start + open;
    (prefix_end > token.start).then_some(CodeToken {
        start: token.start,
        end: prefix_end,
    })
}

fn matching_angle_in_token(token: &str, source: &OutlineSource) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in token.char_indices().rev() {
        match source.symbol(ch) {
            SyntaxSymbol::GenericsClose => depth += 1,
            SyntaxSymbol::GenericsOpen => {
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

fn is_qualified_identifier(value: &str, source: &OutlineSource) -> bool {
    let is_identifier = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .next()
                .is_some_and(|ch| source.is_identifier_start(ch))
            && part.chars().all(|ch| source.is_word_char(ch))
    };
    is_identifier(value)
        || source
            .plan
            .declarations
            .iter()
            .flat_map(|rule| &rule.callable.qualified_separators)
            .any(|separator| !separator.is_empty() && value.split(separator).all(is_identifier))
}

fn callable_name_matches_containing_container(
    text: &str,
    containers: &ContainerNames,
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

    containers.contains(unprefixed_name, name_range.start)
}

struct ContainerNames(std::collections::HashMap<String, (Vec<usize>, Vec<usize>)>);

impl ContainerNames {
    fn new(text: &str, containers: &[StructuralEvent]) -> Self {
        let mut names: std::collections::HashMap<String, (Vec<usize>, Vec<usize>)> =
            std::collections::HashMap::new();
        for container in containers {
            let Some(body) = container.body_range.filter(|range| range.start < range.end) else {
                continue;
            };
            let Some(name) = text.get(container.name_range.start..container.name_range.end) else {
                continue;
            };
            let (starts, ends) = names.entry(name.to_owned()).or_default();
            starts.push(body.start);
            ends.push(body.end);
        }
        for (starts, ends) in names.values_mut() {
            starts.sort_unstable();
            ends.sort_unstable();
        }
        Self(names)
    }

    fn contains(&self, name: &str, offset: usize) -> bool {
        self.0.get(name).is_some_and(|(starts, ends)| {
            starts.partition_point(|start| *start <= offset)
                > ends.partition_point(|end| *end <= offset)
        })
    }
}

fn callable_name_has_qualified_separator(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    rule.callable.qualified_separators.iter().any(|separator| {
        code_before_ends_with(text, source, name_start, separator)
            && qualified_prefix_before_separator(text, source, name_start, separator)
    })
}

fn qualified_prefix_before_separator(
    text: &str,
    source: &OutlineSource,
    name_start: usize,
    separator: &str,
) -> bool {
    let separator_start = previous_code_sequence_start(text, source, name_start, separator);
    let Some(prefix) = previous_contiguous_code_token(text, source, separator_start) else {
        return false;
    };

    let prefix_text = prefix.text(text);
    prefix_text
        .chars()
        .next_back()
        .is_some_and(|ch| source.is_word_char(ch))
}

fn qualified_separator_has_prefix(
    token: &str,
    separator_index: usize,
    source: &OutlineSource,
) -> bool {
    separator_index > 0
        && token[..separator_index]
            .chars()
            .next_back()
            .is_some_and(|ch| source.is_word_char(ch))
}

fn callable_has_container_name_previous_token(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    name_start: usize,
) -> bool {
    let Some(previous) = previous_contiguous_code_token(text, source, name_start) else {
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    statement: &CallableStatement,
    name_start: usize,
) -> usize {
    let mut start = name_start;
    let mut cursor = name_start;

    while let Some(token) = previous_code_token(text, source, cursor) {
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    statement: &CallableStatement,
    after_parameters: usize,
) -> Option<RuleTerminator> {
    let mut cursor = after_parameters;
    let mut angle_depth = 0usize;
    if next_code_token(text, source, cursor)
        .is_some_and(|token| source.symbol_text(token.text(text)) == SyntaxSymbol::ParametersOpen)
    {
        return None;
    }

    while cursor < statement.range.end {
        if !source.is_code(cursor) {
            cursor += next_char_len(text, cursor);
            continue;
        }
        let ch = text[cursor..].chars().next()?;
        match source.symbol(ch) {
            SyntaxSymbol::GenericsOpen => angle_depth += 1,
            SyntaxSymbol::GenericsClose => angle_depth = angle_depth.saturating_sub(1),
            SyntaxSymbol::StatementEnd
                if angle_depth == 0
                    && rule
                        .declaration_terminator
                        .as_deref()
                        .is_some_and(|end| end != "line" && text[cursor..].starts_with(end)) =>
            {
                return (matches!(statement.terminator, CallableStatementTerminator::Semicolon)
                    && cursor < statement.range.end)
                    .then_some(RuleTerminator::Declaration {
                        end: cursor + ch.len_utf8(),
                    });
            }
            _ if source.is_body_open(text, cursor) && angle_depth == 0 => {
                if !matches!(statement.terminator, CallableStatementTerminator::Body) {
                    return None;
                }
                let close = matching_code_brace(text, source, cursor)?;
                return Some(RuleTerminator::Body {
                    open: cursor,
                    end: source.next_token(close)?.end,
                });
            }
            _ if matches!(ch, '\r' | '\n') && angle_depth == 0 => {
                if rule.declaration_terminator.as_deref() == Some("line") {
                    return Some(RuleTerminator::Declaration { end: cursor });
                }
                let line_end = line_end_offset(text, cursor);
                let tail = text.get(cursor..line_end).unwrap_or("");
                if !tail.trim().is_empty() {
                    return None;
                }
            }
            SyntaxSymbol::Assignment if angle_depth == 0 => {
                if rule
                    .callable
                    .assignment_arrow
                    .as_deref()
                    .is_some_and(|arrow| text[cursor..].starts_with(arrow))
                {
                    return None;
                }
                if let Some(next) = next_code_token(text, source, cursor + ch.len_utf8()) {
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
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    after_name: usize,
) -> Option<RuleTerminator> {
    match rule.body {
        OutlineBodyKind::Brace => delimited_signature_terminator(text, source, rule, after_name),
        OutlineBodyKind::Indent => indent_signature_terminator(text, source, after_name),
        OutlineBodyKind::EndKeyword => end_keyword_signature_terminator(text, source, after_name),
        OutlineBodyKind::None => Some(RuleTerminator::Line {
            end: line_end_offset(text, after_name),
        }),
    }
}

fn delimited_signature_terminator(
    text: &str,
    source: &OutlineSource,
    rule: &OutlineRulePlan,
    after_name: usize,
) -> Option<RuleTerminator> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut cursor = after_name;

    while cursor < text.len() {
        if !source.is_code(cursor) {
            cursor += next_char_len(text, cursor);
            continue;
        }

        let ch = text[cursor..].chars().next()?;
        match source.symbol(ch) {
            SyntaxSymbol::ParametersOpen => paren_depth += 1,
            SyntaxSymbol::ParametersClose => paren_depth = paren_depth.saturating_sub(1),
            SyntaxSymbol::BracketsOpen => bracket_depth += 1,
            SyntaxSymbol::BracketsClose => bracket_depth = bracket_depth.saturating_sub(1),
            SyntaxSymbol::GenericsOpen => angle_depth += 1,
            SyntaxSymbol::GenericsClose => angle_depth = angle_depth.saturating_sub(1),
            SyntaxSymbol::StatementEnd
                if paren_depth == 0
                    && bracket_depth == 0
                    && angle_depth == 0
                    && rule
                        .declaration_terminator
                        .as_deref()
                        .is_some_and(|end| end != "line" && text[cursor..].starts_with(end)) =>
            {
                return Some(RuleTerminator::Declaration {
                    end: cursor + ch.len_utf8(),
                });
            }
            _ if matches!(ch, '\r' | '\n')
                && paren_depth == 0
                && bracket_depth == 0
                && angle_depth == 0
                && rule.declaration_terminator.as_deref() == Some("line") =>
            {
                return Some(RuleTerminator::Declaration { end: cursor });
            }
            _ if source.is_body_open(text, cursor)
                && paren_depth == 0
                && bracket_depth == 0
                && angle_depth == 0 =>
            {
                let close = matching_code_brace(text, source, cursor)?;
                return Some(RuleTerminator::Body {
                    open: cursor,
                    end: source.next_token(close)?.end,
                });
            }
            _ => {}
        }

        cursor += ch.len_utf8();
    }

    None
}

fn indent_signature_terminator(
    text: &str,
    source: &OutlineSource,
    after_name: usize,
) -> Option<RuleTerminator> {
    let body = source.body(OutlineBodyKind::Indent)?;
    let mut cursor = after_name;
    while cursor < text.len() {
        let ch = text[cursor..].chars().next()?;
        if source.is_code(cursor) {
            match source.symbol(ch) {
                _ if source.is_delimiter_open(text, cursor) => {
                    let close = source.matching_delimiter(cursor)?;
                    cursor = source.next_token(close)?.end;
                    continue;
                }
                _ if body
                    .header_end
                    .as_deref()
                    .is_some_and(|end| text[cursor..].starts_with(end)) =>
                {
                    return Some(RuleTerminator::Line {
                        end: line_end_offset(text, cursor),
                    });
                }
                _ if matches!(ch, '\r' | '\n') => return None,
                _ if body
                    .line_continuation
                    .as_deref()
                    .is_some_and(|token| text[cursor..].starts_with(token)) =>
                {
                    cursor = next_line_start_offset(text, cursor);
                    continue;
                }
                _ => {}
            }
        }
        cursor += ch.len_utf8();
    }
    None
}

fn indent_body_end(
    text: &str,
    source: &OutlineSource,
    header_offset: usize,
    after_header: usize,
) -> usize {
    let header_indent = indentation_before(text, header_offset);
    let mut cursor = next_line_start_offset(text, after_header);
    let mut continuation_end = cursor;
    while cursor < text.len() {
        let line_end = line_end_offset(text, cursor);
        // Comments and the interior of multiline literals cannot dedent a body.
        let first = text[cursor..line_end]
            .char_indices()
            .find(|(relative, ch)| {
                !ch.is_whitespace()
                    && (source.is_code(cursor + relative)
                        || source.is_literal_start(cursor + relative))
            })
            .map(|(relative, _)| cursor + relative);
        if cursor >= continuation_end {
            if let Some(first) = first {
                if indentation_before(text, first) <= header_indent {
                    return cursor.saturating_sub(line_ending_len_before(text, cursor));
                }
            }
        }
        for token in source
            .tokens_from(cursor)
            .iter()
            .take_while(|token| token.start < line_end)
        {
            if source.is_delimiter_open(text, token.start) {
                if let Some(close) = source.matching_delimiter(token.start) {
                    continuation_end = continuation_end
                        .max(source.next_token(close).map_or(close, |token| token.end));
                }
            }
        }
        cursor = next_line_start_offset(text, line_end);
    }
    text.len()
}

fn end_keyword_signature_terminator(
    text: &str,
    source: &OutlineSource,
    after_name: usize,
) -> Option<RuleTerminator> {
    let end = matching_end_keyword(text, source, after_name).unwrap_or(text.len());

    Some(RuleTerminator::Body {
        open: after_name,
        end,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleTerminator {
    Body { open: usize, end: usize },
    Line { end: usize },
    Declaration { end: usize },
}

#[cfg(test)]
#[path = "structure_tests.rs"]
mod tests;
