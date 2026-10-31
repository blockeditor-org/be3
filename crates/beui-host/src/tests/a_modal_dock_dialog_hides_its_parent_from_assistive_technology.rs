use super::*;

fn hidden_ids(harness: &Harness) -> (Vec<String>, Vec<String>) {
    ::reactive::flush();
    let document = harness.document();
    let view = document
        .accessibility_view()
        .expect("the dock is described");
    let mut shown = Vec::new();
    let mut hidden = Vec::new();
    let mut pending = vec![(view.root(), false)];
    while let Some((id, within_hidden)) = pending.pop() {
        let Some(node) = view.get(&id) else {
            continue;
        };
        let inert = within_hidden || node.is_hidden();
        if let Some(named) = node.author_id() {
            for named in named.split(beui_core::accessibility::TEST_ID_SEPARATOR) {
                match inert {
                    true => hidden.push(named.to_owned()),
                    false => shown.push(named.to_owned()),
                }
            }
        }
        pending.extend(node.children().iter().map(|child| (*child, inert)));
    }
    (shown, hidden)
}

#[test]
fn a_modal_dock_dialog_hides_its_parent_from_assistive_technology() {
    let _root = crate::reactive::RootScope::new();
    let dock = dialog_dock(vec![1, 2], vec![(10, 1, false)]);
    let dialogs = dock.dialogs;
    let mut harness = Harness::sized(dock.document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let (shown, hidden) = hidden_ids(&harness);
    assert!(
        shown.iter().any(|id| id == "content.1") && hidden.is_empty(),
        "a dialog that is not modal leaves its parent to assistive technology: {hidden:?}"
    );

    dialogs.set(vec![(10, 1, true)]);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let (shown, hidden) = hidden_ids(&harness);
    assert!(
        hidden.iter().any(|id| id == "content.1"),
        "a modal dialog makes the panel it blocks inert: {hidden:?}"
    );
    assert!(
        shown.iter().any(|id| id == "dialog.10.field"),
        "while the dialog itself stays: {shown:?}"
    );
    assert!(
        shown.iter().any(|id| id == "dock.tab.2"),
        "and so does the rest of the dock: {shown:?}"
    );

    dialogs.set(Vec::new());
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let (shown, hidden) = hidden_ids(&harness);
    assert!(
        shown.iter().any(|id| id == "content.1") && hidden.is_empty(),
        "closing the dialog gives the panel back: {hidden:?}"
    );
}
