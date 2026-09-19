use fragile_notepad::core::DocumentId;
use fragile_notepad::editor::outline::{
    OutlineParseRequest, outline_registry_hash, parse_outline_request, parse_outline_snapshot,
};
use std::sync::Arc;
use std::time::Instant;

#[test]
fn large_outline_preserves_all_declarations_and_sibling_depths() {
    let hash = outline_registry_hash();
    for count in [900, 1800, 3600, 7200] {
        let text = (0..count)
            .map(|index| format!("fn function_{index}() {{ let value = {index}; }}\n"))
            .collect::<String>();
        let started = Instant::now();
        let result = parse_outline_snapshot(OutlineParseRequest::new(
            DocumentId::new(1),
            Arc::new(text),
            "rs",
            0,
            hash,
        ));
        eprintln!(
            "outline functions={count} elapsed_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.0
        );
        assert_eq!(result.functions.len(), count);
        assert_eq!(result.tree.roots.len(), count);
        for (index, function) in result.functions.iter().enumerate() {
            assert_eq!(function.name, format!("function_{index}"));
            assert_eq!(function.depth, 0);
            assert_eq!(function.range.start.line, index);
        }
    }
}

#[test]
fn async_outline_preserves_non_tokio_executor_compatibility() {
    let request = OutlineParseRequest::new(
        DocumentId::new(1),
        Arc::new("fn example() {}".to_owned()),
        "rs",
        3,
        outline_registry_hash(),
    );
    assert_eq!(
        futures::executor::block_on(parse_outline_request(request.clone())),
        parse_outline_snapshot(request)
    );
}

#[test]
fn repeated_class_names_preserve_constructor_and_method_ownership() {
    let count = 900;
    let request = OutlineParseRequest::new(
        DocumentId::new(1),
        Arc::new("class Item { Item() {} void work() {} };\n".repeat(count)),
        "cpp",
        0,
        outline_registry_hash(),
    );
    let result = parse_outline_snapshot(request);
    assert_eq!(result.functions.len(), count * 2);
    assert_eq!(result.tree.roots.len(), count);
    for node in &result.tree.roots {
        assert_eq!(node.name, "Item");
        assert_eq!(node.children.len(), 2);
        assert_eq!(node.children[0].name, "Item");
        assert_eq!(node.children[1].name, "work");
        assert!(node.children.iter().all(|method| method.depth == 1));
    }
}

#[test]
fn bounded_async_workers_preserve_results_on_tokio() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let requests = (0..8)
        .map(|id| {
            OutlineParseRequest::new(
                DocumentId::new(id),
                Arc::new(format!("fn function_{id}() {{}}")),
                "rs",
                id,
                outline_registry_hash(),
            )
        })
        .collect::<Vec<_>>();
    let expected = requests
        .iter()
        .cloned()
        .map(parse_outline_snapshot)
        .collect::<Vec<_>>();
    let actual = runtime.block_on(futures::future::join_all(
        requests.into_iter().map(parse_outline_request),
    ));
    assert_eq!(actual, expected);
}
