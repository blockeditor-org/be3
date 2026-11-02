use super::*;

use be_block::BlockContent;
use be_type_example_counter::{Counter, CounterContent};

fn seed(instances: &mut Instances, block: Uuid) {
    let mut seeded = CounterContent::default();
    seeded.apply(&Counter::add(5));
    call(
        instances,
        Call::WriteContent {
            reply: reply(next_op()),
            block: block.into_bytes(),
            content_type: CounterContent::CONTENT_TYPE.into_bytes(),
            bytes: seeded.encode(),
            replace: false,
        },
    );
}

fn opened_at(block: Uuid) -> i64 {
    be_host_stack::open(block, CounterContent::CONTENT_TYPE);
    be_host_stack::wait_until(|shared| {
        let held = shared.blocks.get(&block)?;
        CounterContent::decode(held.bytes())
            .ok()
            .map(|counter| counter.root().value())
    })
}

#[test]
fn an_app_seeds_only_blocks_it_holds() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until(|_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let own = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let mut instances = placed_on(own, CounterContent::CONTENT_TYPE);

    seed(&mut instances, stranger);
    seed(&mut instances, own);
    instances.next_screens(PASS);
    be_host_stack::flush();

    assert_eq!(opened_at(own), 5);
    assert_eq!(opened_at(stranger), 0);
}
