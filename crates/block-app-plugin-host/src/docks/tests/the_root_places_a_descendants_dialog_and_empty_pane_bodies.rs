use super::*;
use block_plugin_api::{DockDialog, Size};

#[test]
fn the_root_places_a_descendants_dialog_and_empty_pane_bodies() {
    let mut docks = nested();
    let mut inner = inner();
    inner.dialogs.push(DockDialog {
        tab: path(&[], 6),
        parent: path(&[], 5),
        title: "Dialog".to_owned(),
        glyph: String::new(),
        size: Size {
            width: 200.0,
            height: 100.0,
        },
        modal: false,
        dismiss_on_outside_press: false,
        closable: true,
    });
    if let DockItemKind::Pane(pane) = &mut inner.tree.items[0].kind {
        pane.keep = true;
        pane.empty = Some(path(&[], 7));
    }
    docks.heard("inner", vec![declare(INNER, Some(inner))]);
    host(&mut docks, Some(hosted_by_root()));

    for (tab, what) in [(6, "the dialog's body"), (7, "the empty pane's body")] {
        assert_eq!(
            docks.resolve(ROOT, &path(&[OUTER_CHILD, INNER_CHILD], tab)),
            Some((INNER_BLOCK, tab)),
            "{what} is no tab of the layout but is placed"
        );
    }
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD, INNER_CHILD], 8)),
        None
    );
    let delivered = docks
        .delivered
        .get(&(ROOT, OUTER_CHILD))
        .expect("the root heard its child");
    let dialog = delivered
        .dialog(&path(&[INNER_CHILD], 6))
        .expect("the dialog is spliced through the child that declared it");
    assert_eq!(dialog.parent, path(&[INNER_CHILD], 5));
    let nodes = delivered.nest().expect("the splice is valid");
    assert!(
        nodes
            .panes()
            .iter()
            .any(|pane| pane.empty == Some(path(&[INNER_CHILD], 7))),
        "the empty pane's body is named through the child"
    );
}
