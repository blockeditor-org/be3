use super::*;

#[test]
fn a_modal_dock_dialog_blocks_its_parent_until_it_closes() {
    let _root = crate::reactive::RootScope::new();
    let dock = dialog_dock(vec![1, 2], Vec::new());
    let (pressed, keyed, dialogs) = (dock.pressed, dock.keyed, dock.dialogs);
    let attended = dock.attended;
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let parent = harness.rect(harness.find("content.1"));
    let corner = parent.max - vec2(12.0, 12.0);
    harness.click(corner);
    let field = harness.find("content.1");
    assert_eq!(harness.document().focused_node(), Some(field));
    pressed.update(|pressed| pressed.clear());

    dialogs.set(vec![(10, 1, false)]);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let dialog = harness.find("dialog.10.field");
    assert_eq!(
        harness.document().focused_node(),
        Some(dialog),
        "the focus goes to the dialog"
    );
    harness.click(corner);
    assert_eq!(
        pressed.get_untracked(),
        vec![1],
        "a dialog that is not modal leaves its parent live"
    );
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "and a press on the parent takes the focus there"
    );
    assert!(
        attended.with_untracked(|attended| attended.is_empty()),
        "without calling the dialog's attention"
    );
    assert!(
        harness
            .document()
            .overlay_scrims(crate::WindowId::MAIN)
            .is_empty(),
        "nor is it dimmed"
    );
    pressed.update(|pressed| pressed.clear());

    dialogs.set(vec![(10, 1, true)]);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(dialog),
        "made modal, the dialog takes the focus back"
    );
    let dimmed = harness
        .document()
        .overlay_scrims(crate::WindowId::MAIN)
        .into_iter()
        .map(|scrim| scrim.rect)
        .collect::<Vec<_>>();
    assert_eq!(
        dimmed,
        vec![harness.rect(harness.find("content.1"))],
        "a scrim dims the parent while the dialog blocks it"
    );
    let inside = harness.center(dialog);
    harness.click(inside);
    assert_eq!(
        pressed.get_untracked(),
        vec![10],
        "the dialog stays above what blocks its parent"
    );
    pressed.update(|pressed| pressed.clear());

    harness.click(corner);
    harness.frame(Vec::new());
    assert!(
        pressed.with_untracked(|pressed| pressed.is_empty()),
        "the parent hears no press"
    );
    assert_eq!(
        attended.get_untracked(),
        vec![10],
        "it calls the modal dialog's attention instead"
    );
    assert_eq!(
        harness.document().focused_node(),
        Some(dialog),
        "a press on the parent leaves the focus in the dialog"
    );

    with_installed(harness.document_mut(), |document| {
        document.focus_focusable(field)
    });
    assert_eq!(
        harness.document().focused_node(),
        Some(dialog),
        "the focus cannot move into the parent"
    );
    harness.key(Key::A, Modifiers::NONE);
    assert_eq!(
        keyed.get_untracked(),
        vec![10],
        "keys reach the dialog, not the parent"
    );

    dialogs.set(Vec::new());
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "the focus goes back to the parent when the dialog closes"
    );
    harness.click(corner);
    assert_eq!(
        pressed.get_untracked(),
        vec![1],
        "the parent hears presses again"
    );
}
