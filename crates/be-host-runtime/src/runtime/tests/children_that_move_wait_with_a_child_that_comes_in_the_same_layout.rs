use super::*;

/// Children that only move go with the painting that moves them. When a child comes or goes in
/// the same layout, that painting waits for the poll, so the moving ones wait with it.
#[test]
fn children_that_move_wait_with_a_child_that_comes_in_the_same_layout() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.instances.mount(
        EditorInstanceId(1),
        View::MAIN,
        Uuid::nil(),
        InstanceRole::Editor(
            Uuid::nil(),
            crate::EditorBlock {
                id: Uuid::nil(),
                block_type: Uuid::nil(),
                view_block: None,
            },
        ),
        &Arc::new(crate::AppCatalog::default()),
    );

    runtime.hear(vec![placing(1, &[1])]);
    assert!(
        runtime.held.is_empty(),
        "a child that only moves is applied at once"
    );

    runtime.hear(vec![placing(2, &[1, 2])]);
    let held: Vec<u64> = runtime
        .held
        .laid()
        .flat_map(|laid| laid.children.iter().map(|placed| placed.instance.0))
        .collect();
    assert_eq!(
        held,
        vec![1, 2],
        "beside a child that comes, it waits with it"
    );
}
