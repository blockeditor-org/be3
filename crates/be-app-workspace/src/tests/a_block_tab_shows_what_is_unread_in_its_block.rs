use be_sdk_beui::be_block_content::BlockContent;

use super::*;

/// A block tab carries what the account has not read in its block (`BlockInfo::unread`) as a
/// badge beside its title, as the block's row in the files does.
#[test]
fn a_block_tab_shows_what_is_unread_in_its_block() {
    let (mut fixture, _) = editor();
    let chat = unread_chat(&mut fixture, BlockParent::Root, "Lunch", 3);
    fixture.host.show_block(be_sdk_beui::ShowRequest {
        block_id: chat,
        block_type: be_type_chat::ChatContent::CONTENT_TYPE,
        via: None,
        editor: None,
    });
    fixture.settle();

    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    assert!(
        find_text(&document, root, "3", Match::Exact)
            .is_some_and(|node| document.node_rect(node).is_some()),
        "the tab counts the block's three unread messages"
    );
    drop(document);
    fixture
        .test
        .snapshot("a_block_tab_shows_what_is_unread_in_its_block");
}
