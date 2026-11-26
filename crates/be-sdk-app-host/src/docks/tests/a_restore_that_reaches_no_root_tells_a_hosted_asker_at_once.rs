use super::*;

/// A restore a hosted instance asks for that reaches no root, here because its layout cannot be
/// read to splice it into its placer's, is counted done, and the asker hears so at once with
/// what the root hosts, rather than waiting for the root's next hold.
#[test]
fn a_restore_that_reaches_no_root_tells_a_hosted_asker_at_once() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    updates(&mut docks, "outer");
    let inner = hosted(&mut docks, "inner", INNER);
    assert_eq!(inner.restored, 0);

    let mut unreadable = inner.layout.clone();
    unreadable.tree.items = vec![tab(3, 5)];
    docks.heard(
        "inner",
        vec![DockMessage::Request(DockViewRequest {
            view: ViewRef {
                instance: INNER,
                view: ViewId::MAIN,
            },
            request: DockRequest::Restore(unreadable),
        })],
    );
    assert!(docks.restoring.is_empty(), "the restore is sent nowhere");
    assert!(
        updates(&mut docks, "shell").is_empty(),
        "the root hears nothing of it"
    );
    let told = hosted(&mut docks, "inner", INNER);
    assert_eq!(told.restored, 1, "the asker hears its restore is done");
    assert_eq!(told.layout, inner.layout, "with what the root still hosts");
}
