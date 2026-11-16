use super::*;
use uuid::Uuid;

use be_type_text::{TextBlock, TextContent};

/// A text made offline edits as a client of its own, not the sequence library's `LOADED`, whose
/// edits replacing a child are refused: unlinking a reference in it puts the copy in its place.
#[test]
fn a_copy_put_in_a_childs_place_in_a_text_made_offline_is_taken() {
    let _harness = Harness::start();
    let (notes, source) = (Uuid::new_v4(), Uuid::new_v4());
    wait_until("knew text", |_| is_known(TextContent::CONTENT_TYPE));
    let link = be_block_content::block_url::block_url("", Uuid::nil(), source);
    for (block, name, body) in [
        (source, "Source", "source"),
        (notes, "Notes", link.as_str()),
    ] {
        create(
            block,
            TextContent::CONTENT_TYPE,
            be_block_graph::BlockParent::Root,
            be_block_content::BlockMetadata::named(name),
            Some(TextBlock::of(body).encode()),
            false,
        );
    }
    wait_until("made both offline", |shared| {
        shared.graph.get(source).is_some() && shared.graph.get(notes).is_some()
    });

    let (copy, answer) = replace_with_copy(notes, TextContent::CONTENT_TYPE, source)
        .expect("the text and the source are known");
    assert!(made(Some(answer)), "the text takes the copy");
    let named = copy.to_string();
    wait_until("the text names the copy", |shared| {
        text_of(shared, notes).is_some_and(|text| text.contains(&named))
    });
}
