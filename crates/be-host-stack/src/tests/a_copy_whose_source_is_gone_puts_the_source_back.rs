use super::*;
use uuid::Uuid;

use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

fn text_of(shared: &Shared, block: Uuid) -> Option<String> {
    let held = shared.blocks.get(&block)?;
    Some(TextBlock::text(&TextContent::decode(held.bytes()).ok()?))
}

/// When the block to copy is gone by the time the container said yes, no copy is made: the
/// container is told to name the source again, so it never names a copy that does not exist.
#[test]
fn a_copy_whose_source_is_gone_puts_the_source_back() {
    let harness = Harness::start();
    harness.connect();
    let (notes, copy, missing) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    wait_until("knew text", |_| is_known(TextContent::CONTENT_TYPE));
    // The text already names the copy, as after the container's yes.
    let link = be_block_content::block_url::block_url("", Uuid::nil(), copy);
    create(
        notes,
        TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Notes"),
        Some(TextBlock::of(&link).encode()),
        false,
    );
    // A text not yet on the server takes no replaced child (plans/replace_reports_its_outcome.md).
    wait_until("the text reached the server", |shared| {
        shared.busy(true).is_empty() && shared.unsynced().is_empty()
    });

    assert!(
        !crate::copied_in_place(notes, TextContent::CONTENT_TYPE, missing, copy),
        "a source that is not in the graph gives no copy"
    );
    assert!(node(copy).is_none(), "and no copy is made");
    let source = missing.to_string();
    wait_until("the text names the source again", |shared| {
        text_of(shared, notes).is_some_and(|text| text.contains(&source))
    });
}
