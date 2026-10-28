use block_editor_beui::ShellDialog;
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn renaming_a_block_the_host_asks_about_saves_its_name() {
    let (mut fixture, block) = editor();
    fixture.test.show_dialog(
        block,
        ShellDialog::Rename {
            anchor: None,
            within: None,
        },
    );
    fixture.settle();
    assert!(fixture.says("Rename"));

    fixture.test.click("rename.name");
    fixture.test.type_text("Notes");
    fixture.test.key(Key::Enter, Modifiers::NONE);

    assert_eq!(
        fixture.test.take_renames(),
        vec![(block, Some("Notes".to_owned()))],
        "the typed name is saved to the block"
    );
    let document = fixture.test.document();
    assert!(
        document
            .find_test_id("rename.name")
            .and_then(|field| document.node_rect(field))
            .is_none(),
        "the card closes once the name is saved"
    );
}
