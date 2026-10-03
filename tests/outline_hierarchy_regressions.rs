use fragile_notepad::core::DocumentId;
use fragile_notepad::editor::outline::{
    OutlineNodeKind, OutlineParseRequest, outline_registry_hash, parse_outline_snapshot,
};
use std::sync::Arc;

#[test]
fn local_containers_keep_their_enclosing_functions_and_descendants() {
    for (syntax, source, container, kind) in [
        (
            "rs",
            "fn outer() { mod local { fn leaf() {} } fn sibling() {} }\nfn after() {}",
            "local",
            OutlineNodeKind::Module,
        ),
        (
            "py",
            "def outer():\n    class Local:\n        def leaf(self):\n            pass\n    def sibling():\n        pass\ndef after():\n    pass\n",
            "Local",
            OutlineNodeKind::Class,
        ),
        (
            "js",
            "function outer() { class Local { leaf() {} } function sibling() {} }\nfunction after() {}",
            "Local",
            OutlineNodeKind::Class,
        ),
    ] {
        let result = parse_outline_snapshot(OutlineParseRequest::new(
            DocumentId::new(1),
            Arc::new(source.to_owned()),
            syntax,
            0,
            outline_registry_hash(),
        ));
        assert_eq!(result.tree.roots.len(), 2, "{syntax}: {:?}", result.tree);
        let outer = &result.tree.roots[0];
        assert_eq!(outer.name, "outer", "{syntax}");
        assert_eq!(outer.depth, 0, "{syntax}");
        assert_eq!(outer.children.len(), 2, "{syntax}: {outer:?}");
        let local = &outer.children[0];
        assert_eq!(local.name, container, "{syntax}");
        assert_eq!(local.kind, kind, "{syntax}");
        assert_eq!(local.depth, 1, "{syntax}");
        assert_eq!(local.children.len(), 1, "{syntax}");
        assert_eq!(local.children[0].name, "leaf", "{syntax}");
        assert_eq!(local.children[0].depth, 2, "{syntax}");
        assert_eq!(outer.children[1].name, "sibling", "{syntax}");
        assert_eq!(outer.children[1].depth, 1, "{syntax}");
        assert_eq!(result.tree.roots[1].name, "after", "{syntax}");
        assert_eq!(result.tree.roots[1].depth, 0, "{syntax}");
        for function in &result.functions {
            assert_eq!(
                function.depth,
                match function.name.as_str() {
                    "leaf" => 2,
                    "sibling" => 1,
                    _ => 0,
                },
                "{syntax}: {}",
                function.name
            );
        }
    }
}

#[test]
fn many_local_containers_preserve_ownership_and_sibling_depths() {
    let count = 900;
    let source = (0..count)
        .map(|index| format!("fn outer_{index}() {{ mod local {{ fn leaf() {{}} }} }}\n"))
        .collect::<String>();
    let result = parse_outline_snapshot(OutlineParseRequest::new(
        DocumentId::new(1),
        Arc::new(source),
        "rs",
        0,
        outline_registry_hash(),
    ));
    assert_eq!(result.functions.len(), count * 2);
    assert_eq!(result.tree.roots.len(), count);
    for (index, outer) in result.tree.roots.iter().enumerate() {
        assert_eq!(outer.name, format!("outer_{index}"));
        assert_eq!(outer.depth, 0);
        assert_eq!(outer.children.len(), 1);
        assert_eq!(outer.children[0].name, "local");
        assert_eq!(outer.children[0].depth, 1);
        assert_eq!(outer.children[0].children.len(), 1);
        assert_eq!(outer.children[0].children[0].name, "leaf");
        assert_eq!(outer.children[0].children[0].depth, 2);
    }
}
