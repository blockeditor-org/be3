use super::*;

#[test]
fn a_modal_dock_dialog_blocks_its_parent_until_it_closes() {
    let dock = dialog_dock(vec![1, 2], Vec::new());
    let (pressed, keyed, set_dialogs) = (dock.pressed, dock.keyed, dock.set_dialogs);
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let parent = harness.rect(harness.find("content.1"));
    let corner = parent.min + vec2(12.0, 12.0);
    harness.click(corner);
    let field = harness.find("content.1");
    assert_eq!(harness.document().focused_node(), Some(field));
    pressed.borrow_mut().clear();

    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        set_dialogs.set(vec![(10, 1, false)]);
    });
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
        *pressed.borrow(),
        vec![1],
        "a dialog that is not modal leaves its parent live"
    );
    pressed.borrow_mut().clear();

    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        set_dialogs.set(vec![(10, 1, true)]);
    });
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(dialog),
        "made modal, the dialog takes the focus back"
    );
    let inside = harness.center(dialog);
    harness.click(inside);
    assert_eq!(
        *pressed.borrow(),
        vec![10],
        "the dialog stays above what blocks its parent"
    );
    pressed.borrow_mut().clear();

    harness.click(corner);
    harness.frame(Vec::new());
    assert!(pressed.borrow().is_empty(), "the parent hears no press");
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
        *keyed.borrow(),
        vec![10],
        "keys reach the dialog, not the parent"
    );

    crate::reactive::with_reactive_scope(harness.document_mut(), || set_dialogs.set(Vec::new()));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused_node(),
        Some(field),
        "the focus goes back to the parent when the dialog closes"
    );
    harness.click(corner);
    assert_eq!(*pressed.borrow(), vec![1], "the parent hears presses again");
}
