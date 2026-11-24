use be_sdk_beui::beui::Key;
use be_sdk_beui::{BlockInfo, BlockParent};

use super::*;

/// A name past what a block's name holds (`metadata::MAX_NAME_BYTES`) is refused, not cut: the
/// block keeps its name, and the editor says why.
#[test]
fn a_name_longer_than_a_name_holds_is_refused_and_said_why() {
    let mut fixture = editor();
    let notes = Uuid::from_u128(5);
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    my_files(&mut fixture, vec![listed]);
    fixture.settle();

    fixture.choose(notes, "Rename");
    fixture.test.type_text(&"x".repeat(200));
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert!(
        fixture.test.take_renames().is_empty(),
        "the block keeps its name"
    );
    let toast = format!("toast.{}", crate::root::rename::RENAME_REFUSED);
    assert!(fixture.test.shown(&toast), "and the editor says why");
}
