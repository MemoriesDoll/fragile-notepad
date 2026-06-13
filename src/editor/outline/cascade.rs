use super::fsm::{DeclarationEvent, StructuralEvent, StructuralEventKind};
use super::structure::{
    container_depth, containing_container, declaration_depth, editor_range, indent_depth_before,
};
use super::{
    FunctionKind, OutlineBodyKind, OutlineNode, OutlineNodeKind, OutlineScanMode, OutlineTree,
};
use std::cmp::Reverse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CascadeOutput {
    pub tree: OutlineTree,
    pub functions: Vec<CascadedFunction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CascadedFunction {
    pub name: String,
    pub kind: FunctionKind,
    pub range: super::EditorRange,
    pub body_range: Option<super::EditorRange>,
    pub depth: usize,
    pub start_offset: usize,
    pub end_offset: usize,
}

pub(super) fn cascade(
    text: &str,
    containers: &[StructuralEvent],
    declarations: Vec<DeclarationEvent>,
) -> CascadeOutput {
    let mut roots = container_nodes(text, containers);
    let mut functions = Vec::new();
    let mut function_nodes = Vec::new();

    for declaration in &declarations {
        if declaration.rule.scan == OutlineScanMode::Callable
            && declaration.terminated
            && declaration_depth(&declarations, declaration.signature_range.start) > 0
        {
            continue;
        }

        let depth = match declaration.rule.body {
            OutlineBodyKind::Indent => indent_depth_before(text, declaration.signature_range.start),
            _ => {
                container_depth(containers, declaration.signature_range.start)
                    + declaration_depth(&declarations, declaration.signature_range.start)
            }
        };
        let mut kind = if declaration.rule.node_kind == OutlineNodeKind::Method
            || containing_container(
                containers,
                declaration.signature_range.start,
                &declaration.rule.method_containers,
            )
            .is_some()
        {
            FunctionKind::Method
        } else {
            FunctionKind::Function
        };
        if declaration.terminated {
            kind = FunctionKind::Declaration;
        }

        let Some(range) = editor_range(text, declaration.signature_range) else {
            continue;
        };
        let body_range = declaration
            .body_range
            .and_then(|range| editor_range(text, range));

        let node_kind = match kind {
            FunctionKind::Function => OutlineNodeKind::Function,
            FunctionKind::Method => OutlineNodeKind::Method,
            FunctionKind::Declaration => OutlineNodeKind::Declaration,
        };
        let function = CascadedFunction {
            name: declaration.name.clone(),
            kind,
            range,
            body_range,
            depth,
            start_offset: declaration.signature_range.start,
            end_offset: declaration.signature_range.end,
        };
        let node = OutlineNode::new(
            function.name.clone(),
            node_kind,
            function.range,
            function.body_range,
            depth,
        );

        function_nodes.push((function.clone(), node));
        functions.push(function);
    }
    deduplicate_functions(&mut functions);
    deduplicate_function_nodes(&mut function_nodes);

    for (_, node) in function_nodes {
        attach_to_nearest_container(&mut roots, node);
    }

    CascadeOutput {
        tree: OutlineTree::new(roots),
        functions,
    }
}

fn deduplicate_functions(functions: &mut Vec<CascadedFunction>) {
    functions.sort_by_key(|function| {
        (
            function.end_offset,
            Reverse(function.end_offset.saturating_sub(function.start_offset)),
            function.start_offset,
        )
    });
    functions.dedup_by(|left, right| {
        left.end_offset == right.end_offset && left.name == right.name && left.depth == right.depth
    });
    functions.sort_by_key(|function| function.range.start);
}

fn deduplicate_function_nodes(function_nodes: &mut Vec<(CascadedFunction, OutlineNode)>) {
    function_nodes.sort_by_key(|(function, _)| {
        (
            function.end_offset,
            Reverse(function.end_offset.saturating_sub(function.start_offset)),
            function.start_offset,
        )
    });
    function_nodes.dedup_by(|(left, _), (right, _)| {
        left.end_offset == right.end_offset && left.name == right.name && left.depth == right.depth
    });
    function_nodes.sort_by_key(|(function, _)| function.range.start);
}

fn container_nodes(text: &str, containers: &[StructuralEvent]) -> Vec<OutlineNode> {
    let mut roots = Vec::new();
    let mut sorted = containers.to_vec();
    sorted.sort_by_key(|event| (event.signature_range.start, event.signature_range.end));

    for event in sorted {
        let Some(range) = editor_range(text, event.signature_range) else {
            continue;
        };
        let body_range = event.body_range.and_then(|range| editor_range(text, range));
        let StructuralEventKind::Body { owner_kind, .. } = event.kind;
        let depth = containers
            .iter()
            .filter(|container| {
                container.body_range.is_some_and(|range| {
                    range.start <= event.signature_range.start
                        && event.signature_range.start < range.end
                }) && container.signature_range.start != event.signature_range.start
            })
            .count();
        let name = text
            .get(event.name_range.start..event.name_range.end)
            .unwrap_or("")
            .to_owned();
        let node = OutlineNode::new(name, owner_kind, range, body_range, depth);

        attach_to_nearest_container(&mut roots, node);
    }

    roots
}

fn attach_to_nearest_container(nodes: &mut Vec<OutlineNode>, node: OutlineNode) {
    if let Some(path) = nearest_container_path(nodes, node.range.start, node.depth) {
        node_at_path_mut(nodes, &path).children.push(node);
    } else {
        nodes.push(node);
    }
}

fn nearest_container_path(
    nodes: &[OutlineNode],
    position: super::EditorPosition,
    depth: usize,
) -> Option<Vec<usize>> {
    let mut best = None;

    for (index, node) in nodes.iter().enumerate() {
        let contains = node
            .body_range
            .or(Some(node.range))
            .is_some_and(|range| range.start <= position && position < range.end);
        if !contains || node.depth >= depth {
            continue;
        }

        let mut path = vec![index];
        if let Some(mut child_path) = nearest_container_path(&node.children, position, depth) {
            path.append(&mut child_path);
        }
        best = Some(path);
    }

    best
}

fn node_at_path_mut<'a>(nodes: &'a mut [OutlineNode], path: &[usize]) -> &'a mut OutlineNode {
    let (first, rest) = path.split_first().expect("non-empty outline path");
    let node = &mut nodes[*first];
    if rest.is_empty() {
        node
    } else {
        node_at_path_mut(&mut node.children, rest)
    }
}
