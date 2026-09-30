use super::*;

use std::time::Duration;

use be_block::{BlockContent, Counter, CounterContent};

fn seed(instances: &mut Instances, block: Uuid) {
    let mut seeded = CounterContent::default();
    seeded.apply(&Counter::add(5));
    instances.editor_message(EditorMessage::SeedContent {
        instance: INSTANCE,
        block_id: block.into_bytes(),
        content_type: CounterContent::CONTENT_TYPE.into_bytes(),
        bytes: seeded.encode(),
    });
}

fn opened_at(block: Uuid) -> i64 {
    crate::be::open(block, CounterContent::CONTENT_TYPE);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        CounterContent::decode(&held.bytes)
            .ok()
            .map(|counter| counter.root().value())
    })
    .expect("the new stack never opened the block")
}

#[test]
fn a_plugin_seeds_only_blocks_it_holds() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let own = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);

    seed(&mut instances, stranger);
    seed(&mut instances, own);
    instances.next_screens(PASS);
    crate::be::flush();

    assert_eq!(opened_at(own), 5);
    assert_eq!(opened_at(stranger), 0);
}
