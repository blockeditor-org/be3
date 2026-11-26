use be_sdk_beui::be_block_content::{ChildChange, LiveEdit};

use super::*;

/// Unlinking a reference rewrites its URL in the block's type, as with the text closed; the
/// open editor takes it as it takes any other edit and keeps typing where it was.
#[test]
fn a_reference_unlinked_while_the_text_is_open_is_rewritten_under_the_caret() {
    let old = Uuid::from_u128(0x01d0_0000_0000_4000_8000_0000_0000_0001);
    let new = Uuid::from_u128(0x0e00_0000_0000_4000_8000_0000_0000_0002);
    let link = |block| block_url("https://example.com/", ORGANIZATION_ID, block);
    let mut editor = editor(&format!("see {}", link(old)));
    caret_at(&mut editor, 3);

    let operations = editor
        .content::<TextContent>(None)
        .child_operations(ChildChange::Replace { old, new }, PEER)
        .expect("the text takes the replacement");
    for operation in &operations {
        editor.edit::<TextContent>(None, operation);
    }
    editor.run();
    editor.type_text(",");

    assert_eq!(text(&editor), format!("see, {}", link(new)));
}
