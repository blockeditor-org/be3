use super::*;

use be_block::ImageContent;

fn holds(shared: &Shared, block: Uuid, name: &str) -> bool {
    shared.blocks.get(&block).is_some_and(|held| {
        ImageContent::decode(&held.bytes).is_ok_and(|image| {
            image.header().source_name == name && image.data() == name.as_bytes()
        })
    })
}

fn image(name: &str) -> Vec<u8> {
    ImageContent::from_file(name, name.as_bytes().to_vec()).encode()
}

#[test]
fn replacing_content_reaches_a_block_open_or_not() {
    let harness = Harness::start();
    harness.connect();
    let (open, closed) = (Uuid::new_v4(), Uuid::new_v4());

    hold(open, ImageContent::CONTENT_TYPE);
    wait_until("opened the image", |shared| {
        shared.blocks.contains_key(&open)
    });
    replace(open, ImageContent::CONTENT_TYPE, image("first.png"));
    wait_until("replaced the open image", |shared| {
        holds(shared, open, "first.png")
    });
    replace(open, ImageContent::CONTENT_TYPE, image("second.png"));
    wait_until("replaced it again", |shared| {
        holds(shared, open, "second.png")
    });

    replace(closed, ImageContent::CONTENT_TYPE, image("unopened.png"));
    flush();
    hold(closed, ImageContent::CONTENT_TYPE);
    wait_until("opened what was written while it was closed", |shared| {
        holds(shared, closed, "unopened.png")
    });
}
