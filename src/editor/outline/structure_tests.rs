use super::*;
use crate::editor::outline::lexical::OutlineCodeMask;
use crate::editor::outline::{OutlineNodeKind, OutlineRegistry};

fn discover_for_syntax(text: &str, syntax_token: &str) -> StructurePassOutput {
    let registry = OutlineRegistry::load();
    let plan = registry.plan_for_syntax(syntax_token).unwrap();
    let mask = OutlineCodeMask::new(text, &plan.lexical);

    discover_structure(text, &mask, plan)
}

#[test]
fn discover_structure_tracks_ruby_end_keyword_containers_and_definitions() {
    let text = "def top\n  1\nend\n\nmodule Admin\n  class User\n    def save\n      2\n    end\n  end\nend\n";
    let structure = discover_for_syntax(text, "rb");

    assert_eq!(
        structure
            .containers
            .iter()
            .map(|event| {
                let StructuralEventKind::Body { owner_kind, .. } = event.kind;
                (
                    owner_kind,
                    &text[event.name_range.start..event.name_range.end],
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (OutlineNodeKind::Module, "Admin"),
            (OutlineNodeKind::Class, "User")
        ]
    );
    assert_eq!(
        structure
            .declarations
            .iter()
            .map(|event| (event.name.as_str(), event.body_range.is_some()))
            .collect::<Vec<_>>(),
        vec![("top", true), ("save", true)]
    );
    assert!(
        containing_container(
            &structure.containers,
            structure.declarations[1].signature_range.start,
            &structure.declarations[1].rule.method_containers,
        )
        .is_some()
    );
}

#[test]
fn discover_structure_tracks_javascript_function_keyword_entries() {
    let text = "function top() {\n}\nclass Service {\n  function load() {\n  }\n}\n";
    let structure = discover_for_syntax(text, "js");

    assert_eq!(structure.containers.len(), 1);
    assert_eq!(
        &text[structure.containers[0].name_range.start..structure.containers[0].name_range.end],
        "Service"
    );
    assert_eq!(
        structure
            .declarations
            .iter()
            .map(|event| (event.name.as_str(), event.body_range.is_some()))
            .collect::<Vec<_>>(),
        vec![("top", true), ("load", true)]
    );
    assert!(
        containing_container(
            &structure.containers,
            structure.declarations[1].signature_range.start,
            &structure.declarations[1].rule.method_containers,
        )
        .is_some()
    );
}
