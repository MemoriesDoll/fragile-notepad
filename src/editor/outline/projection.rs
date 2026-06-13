use std::cmp::Reverse;

use super::FunctionEntry;
use super::cascade::CascadedFunction;

pub(super) fn project_functions(mut functions: Vec<CascadedFunction>) -> Vec<FunctionEntry> {
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

    functions
        .into_iter()
        .map(|function| FunctionEntry {
            name: function.name,
            kind: function.kind,
            range: function.range,
            body_range: function.body_range,
            depth: function.depth,
        })
        .collect()
}
