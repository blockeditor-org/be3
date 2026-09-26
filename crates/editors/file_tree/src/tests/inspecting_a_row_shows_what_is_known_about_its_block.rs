use block_editor_plugin::beui::{Document, Event, Modifiers, NodeId, PointerButton, Pos2};
use block_editor_plugin::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn inspecting_a_row_shows_what_is_known_about_its_block() {
    let mut fixture = editor();
    let block_type = Uuid::from_u128(1);
    let block = Uuid::from_u128(2);
    let mut listed = BlockInfo::new(block, block_type, BlockParent::Root);
    listed.name = Some("Notes".to_owned());
    listed.named_by_hand = true;
    listed.references = vec![Uuid::from_u128(3), Uuid::from_u128(4)];
    fixture.host.set_blocks(BlockQuery::Roots, vec![listed]);
    fixture.settle();
    assert!(
        !fixture.test.shown("file-tree.inspect.id"),
        "nothing is inspected until asked"
    );

    let pos = fixture
        .test
        .rect_of(&format!("file-tree.{block}.row"))
        .center();
    fixture.test.step(vec![Event::PointerMoved(pos)]);
    fixture.test.step(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    fixture.test.step(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    fixture.settle();
    let document = fixture.test.document();
    let item = document
        .root()
        .and_then(|root| open_item(document, root, "Inspect", false))
        .expect("right clicking a row opens a menu that offers to inspect it");
    fixture.test.click_at(item);
    fixture.settle();

    fixture
        .test
        .snapshot("inspecting_a_row_shows_what_is_known_about_its_block");
    assert_eq!(
        fixture.test.label("file-tree.inspect.id"),
        block.to_string()
    );
    assert_eq!(fixture.test.label("file-tree.inspect.name"), "Notes");
    assert_eq!(fixture.test.label("file-tree.inspect.parent"), "Root");
    assert_eq!(fixture.test.label("file-tree.inspect.references"), "2");
    assert!(
        fixture.opened().is_empty(),
        "inspecting a block must not open it"
    );

    fixture.test.click("file-tree.inspect.close");
    fixture.settle();
    assert!(
        !fixture.test.shown("file-tree.inspect.id"),
        "closing the inspector hides it"
    );
}

fn open_item(document: &Document, node: NodeId, label: &str, open: bool) -> Option<Pos2> {
    let open = match document.node_kind(node) {
        "overlay" => document.node_detail(node).as_deref() == Some("open"),
        _ => open,
    };
    let text = document.node_detail(node).unwrap_or_default();
    if open && document.node_kind(node) == "text" && text.trim_matches('"') == label {
        return document.node_rect(node).map(|rect| rect.center());
    }
    document
        .children(node)
        .into_iter()
        .find_map(|child| open_item(document, child, label, open))
}
