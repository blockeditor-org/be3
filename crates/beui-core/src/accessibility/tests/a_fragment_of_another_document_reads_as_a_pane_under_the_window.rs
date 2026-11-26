use accesskit::Toggled;

use super::*;
use crate::program::accessibility_dump::AccessibilityDump;

fn id(document: u64, local: u64) -> AccessNodeId {
    AccessNodeId((document << DOCUMENT_SHIFT) | local)
}

fn named(role: Role, label: &str, bounds: AccessRect) -> Node {
    let mut node = Node::new(role);
    node.set_label(label);
    node.set_bounds(bounds);
    node
}

/// An app's screen, grafted by the host into a document of its own (be-sdk-app-host's
/// `accessibility::Graft`), joins the window's tree as a fragment: a pane at the screen's place,
/// holding the app's root moved to where the screen is drawn, all scaled with the window.
#[test]
fn a_fragment_of_another_document_reads_as_a_pane_under_the_window() {
    let mut pane = named(
        Role::Pane,
        "Checklist",
        AccessRect::new(100.0, 50.0, 300.0, 150.0),
    );
    pane.set_children(vec![id(7, 1)]);
    let mut root = Node::new(Role::GenericContainer);
    root.set_transform(Affine::translate((100.0, 50.0)));
    root.set_children(vec![id(7, 2), id(7, 4)]);
    let mut list = named(Role::List, "Tasks", AccessRect::new(0.0, 0.0, 200.0, 80.0));
    list.set_children(vec![id(7, 3)]);
    let mut checked = named(
        Role::CheckBox,
        "Buy milk",
        AccessRect::new(10.0, 10.0, 130.0, 30.0),
    );
    checked.set_toggled(Toggled::True);
    let hidden = named(
        Role::Button,
        "Hidden",
        AccessRect::new(10.0, 350.0, 70.0, 370.0),
    );
    let fragment = Fragment::new(
        vec![
            (id(7, 0), pane),
            (id(7, 1), root),
            (id(7, 2), list),
            (id(7, 3), checked),
            (id(7, 4), hidden),
        ],
        id(7, 0),
        Some(id(7, 3)),
    );

    let mut dump = AccessibilityDump::new();
    dump.update(tree_update(
        "Test",
        Vec2::new(400.0, 300.0),
        2.0,
        vec![fragment],
    ));
    assert_eq!(
        dump.rendered().text(),
        "Window \"Test\" at 0,0 size 800x600\n\
         \x20 Pane \"Checklist\" at 200,100 size 400x200\n\
         \x20   List \"Tasks\" at 200,100 size 400x160\n\
         \x20     CheckBox \"Buy milk\" toggled=True focused at 220,120 size 240x40\n\
         \x20   Button \"Hidden\" at 220,800 size 120x40 offscreen\n"
    );
}
