use super::*;
use uuid::Uuid;

use be_type_text::{TextBlock, TextContent};

/// Unlink's copy is made where the container's yes is heard: by the time the answer reaches the
/// host the copy exists, so a host that stops before placing it leaves a detached copy the text
/// names, never a name with no block behind it.
#[test]
fn a_copy_put_in_a_childs_place_is_made_before_the_answer() {
    let harness = Harness::start();
    harness.connect();
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
    ask_for(vec![Query::Roots]);
    wait_until("listed the roots", |shared| {
        shared.graph.get(source).is_some()
    });

    // The copy is asked for before the text is asked to take it, so the mirror keeps it once
    // made: a detached block nothing asks for leaves the mirror as its creation settles, which
    // the worker may reach before a request sent after the change.
    let copy = Uuid::now_v7();
    ask_for(vec![Query::Block(copy)]);
    let answer = replace_with_copy(notes, TextContent::CONTENT_TYPE, source, copy)
        .expect("the text and the source are known");
    assert!(made(Some(answer)), "the text takes the copy");
    assert!(
        node(copy).is_some(),
        "the copy is made by the time the answer is heard"
    );
}
