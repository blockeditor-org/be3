use be_sdk_beui::{EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use be_type_text::TextBlock;
use uuid::Uuid;

use crate::root::TextRoot;

/// A view block deleted elsewhere, or no longer shared, never loads: the text shows all the
/// same rather than waiting on it for ever.
#[test]
fn a_text_whose_view_block_is_gone_still_shows() {
    let (block, view) = (Uuid::new_v4(), Uuid::new_v4());
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_view_block(Some(view));
    let mut editor = BeuiTest::<TextRoot>::new(OwnedEditor::new(host, block));
    editor.hold(None, TextBlock::of("Line 1\nLine 2\n"));
    editor.presence_visible(true);
    editor.run();
    assert!(
        editor.shown("text.surface"),
        "the text shows though its view block is not there"
    );
}
