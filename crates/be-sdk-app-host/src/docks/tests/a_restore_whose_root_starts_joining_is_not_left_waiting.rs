use super::*;

#[test]
fn a_restore_whose_root_starts_joining_is_not_left_waiting() {
    const TOP: EditorInstanceId = EditorInstanceId(4);
    let mut docks = nested();
    updates(&mut docks, "shell");
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    updates(&mut docks, "outer");
    let inner = hosted(&mut docks, "inner", INNER);

    docks.heard(
        "inner",
        vec![DockMessage::Request(DockViewRequest {
            view: ViewRef {
                instance: INNER,
                view: ViewId::MAIN,
            },
            request: DockRequest::Restore(inner.layout),
        })],
    );
    assert!(matches!(
        updates(&mut docks, "shell").as_slice(),
        [(ROOT, DockUpdate::ChildRequest(_))]
    ));

    // Before the root answers, it is docked in another app's dock: it joins that dock, so the
    // host stops listening to its holds and its count can never answer the restore.
    docks.register(TOP, "top", None);
    docks.placed(
        TOP,
        &[DockedChild {
            child: ChildId(30),
            tab: 7,
            block: Uuid::from_u128(1),
        }],
        None,
    );
    assert!(docks.joins(ROOT));
    assert!(
        docks.restoring.is_empty(),
        "a restore sent to a root that now joins another dock is not left waiting"
    );
    assert_eq!(docks.restored.get(&INNER).copied(), Some(1));
}
