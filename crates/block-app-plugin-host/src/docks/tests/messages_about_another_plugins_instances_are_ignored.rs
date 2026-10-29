use super::*;

#[test]
fn messages_about_another_plugins_instances_are_ignored() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    docks.heard("outer", vec![declare(INNER, None)]);
    docks.heard(
        "outer",
        vec![DockMessage::Close(DockChildClose {
            child: DockChild {
                instance: ROOT,
                child: OUTER_CHILD,
            },
            tab: path(&[], 1),
        })],
    );
    assert!(updates(&mut docks, "shell").is_empty());
    assert!(updates(&mut docks, "outer").is_empty());
}
