use super::*;

fn restore(docks: &mut Docks, app: &str, instance: EditorInstanceId, layout: DockLayout) {
    docks.heard(
        app,
        vec![DockMessage::Request(DockViewRequest {
            view: ViewRef {
                instance,
                view: ViewId::MAIN,
            },
            request: DockRequest::Restore(layout),
        })],
    );
}

#[test]
fn a_restore_is_counted_done_once_the_root_says_it_took_it() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    let outer = hosted(&mut docks, "outer", OUTER);
    let inner = hosted(&mut docks, "inner", INNER);
    assert_eq!(inner.restored, 0);

    restore(&mut docks, "inner", INNER, inner.layout.clone());
    assert!(matches!(
        updates(&mut docks, "shell").as_slice(),
        [(ROOT, DockUpdate::ChildRequest(_))]
    ));
    // What the root hosted before it took the restore says it has not.
    let mut before = hosted_by_root();
    before.focused = Some(path(&[], 1));
    host(&mut docks, DockHold::Hosted(before.clone()));
    updates(&mut docks, "outer");
    assert_eq!(hosted(&mut docks, "inner", INNER).restored, 0);

    let mut took = before;
    took.restored = 1;
    host(&mut docks, DockHold::Hosted(took.clone()));
    assert_eq!(
        hosted(&mut docks, "inner", INNER).restored,
        1,
        "the inner dock hears its restore was taken"
    );
    assert_eq!(
        updates(&mut docks, "outer"),
        [],
        "the outer dock asked for none"
    );

    // A root that lets the child go takes none of what it asked: its restore is done too.
    restore(&mut docks, "outer", OUTER, outer.layout);
    updates(&mut docks, "shell");
    host(&mut docks, DockHold::Released);
    updates(&mut docks, "outer");
    updates(&mut docks, "inner");
    host(&mut docks, DockHold::Hosted(took));
    assert_eq!(
        hosted(&mut docks, "outer", OUTER).restored,
        1,
        "a restore the root let go of waits no longer"
    );
}
