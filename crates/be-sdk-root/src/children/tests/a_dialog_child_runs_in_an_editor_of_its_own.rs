use super::*;

#[test]
fn a_dialog_child_runs_in_an_editor_of_its_own() {
    // A rename form an editor placed is a dialog the root opens in the editor that renames
    // blocks: no block of its own, which nothing above it shows.
    let mut form = placed(rect((0.0, 0.0), (200.0, 120.0)), ChildMode::Live);
    form.content = ChildContent::Rename([5; 16]);
    let child = wanted(form.clone());
    assert!(
        matches!(&child.shown, Shown::Dialog(dialog) if dialog.content == be_wasm_ops::encode(&form.content) && !dialog.sheet)
    );
    assert_eq!(child.block, None);

    // Two forms renaming the same block, in two places, are two editors, each opened: a dialog is
    // its placer's own, never shared, and the block it renames being open above refuses neither.
    let mut other = form.clone();
    other.child = ChildId(8);
    let opened = reconcile(Vec::new(), vec![wanted(form), wanted(other)], &[[5; 16]]);
    assert_eq!(opened.opened, vec![0, 1]);
    assert!(!Rc::ptr_eq(&opened.kept[0].editor, &opened.kept[1].editor));

    // Kept placed, it keeps its editor.
    let kept = Rc::clone(&opened.kept[0].editor);
    let mut again = placed(rect((10.0, 0.0), (210.0, 120.0)), ChildMode::Live);
    again.content = ChildContent::Rename([5; 16]);
    let second = reconcile(opened.kept, vec![wanted(again)], &[]);
    assert!(second.opened.is_empty());
    assert!(Rc::ptr_eq(&second.kept[0].editor, &kept));

    // Kinds the root shows nothing of yet are not placed as editors.
    let mut panel = placed(rect((0.0, 0.0), (200.0, 120.0)), ChildMode::Live);
    panel.content = ChildContent::Host(be_sdk_api::HostPanel::Apps);
    assert!(
        Child::placed(
            EditorInstanceId(1),
            ViewId::MAIN,
            1.0,
            panel,
            Vec::new(),
            false
        )
        .is_none()
    );
}
