use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A text that names the block unlinked puts the copy in its place: only once its type answers
/// is the copy placed in the text.
#[test]
fn an_unlink_the_containers_type_makes_places_the_copy_in_it() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (source, notes) = (Uuid::new_v4(), Uuid::new_v4());
    let text = TextContent::CONTENT_TYPE;
    let link = be_block_content::block_url::block_url("https://be3.example/", Uuid::nil(), source);
    make(
        source,
        text,
        BlockParent::Root,
        "Source",
        TextBlock::of("source").encode(),
    );
    make(
        notes,
        text,
        BlockParent::Root,
        "Notes",
        TextBlock::of(&link).encode(),
    );
    synced();
    let _apps = known(&[text]);
    let mut core = core();
    core.block_types.insert(source, text);
    core.block_types.insert(notes, text);

    core.queue_copy(source, notes);
    core.process_pending_copies();
    let copy = match &core.pending_copies[0].stage {
        CopyStage::Replacing { copy_id, .. } => *copy_id,
        _ => panic!("the copy is made and sent to the text before its type answers"),
    };
    settle(&mut core);

    be_host_stack::wait_until("the text names the copy", |shared| {
        let held = shared.blocks.get(&notes)?;
        let text = TextBlock::text(&TextContent::decode(held.bytes()).ok()?);
        text.contains(&copy.to_string()).then_some(())
    });
}
