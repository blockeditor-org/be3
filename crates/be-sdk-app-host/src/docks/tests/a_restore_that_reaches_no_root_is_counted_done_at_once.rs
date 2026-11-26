use super::*;

#[test]
fn a_restore_that_reaches_no_root_is_counted_done_at_once() {
    const LONE: EditorInstanceId = EditorInstanceId(5);
    let mut docks = nested();
    docks.register(LONE, "lone", None);
    docks.heard(
        "lone",
        vec![DockMessage::Request(DockViewRequest {
            view: ViewRef {
                instance: LONE,
                view: ViewId::MAIN,
            },
            request: DockRequest::Restore(hosted_by_root().layout),
        })],
    );
    assert!(docks.restoring.is_empty());
    assert_eq!(
        docks.restored.get(&LONE).copied(),
        Some(1),
        "a restore no root will take is done as soon as it is asked"
    );
}
