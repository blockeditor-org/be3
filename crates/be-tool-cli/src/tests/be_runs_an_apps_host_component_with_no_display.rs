use super::*;

use be_type_example_counter::CounterContent;

/// `be` runs the counter's host component headless, on a block of a stack it started, the app
/// read from the bundle BE_BUNDLE names rather than downloaded: no window, no presenter, no
/// gpu; the component adds one once the content arrives, and the block shows it
/// (plans/headless_apps.md, step 1).
#[test]
fn be_runs_an_apps_host_component_with_no_display() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    be_host_stack::wait_until("knew the counter type", |_| {
        be_host_stack::is_known(CounterContent::CONTENT_TYPE).then_some(())
    });
    let block = Uuid::now_v7();
    be_host_stack::create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    be_host_stack::flush_and_send();
    be_host_stack::wait_until("listed the counter", |shared| {
        shared.graph.get(block).map(|_| ())
    });

    let mut hosting = crate::hosting::Hosting::start(
        block,
        CounterContent::CONTENT_TYPE,
        Uuid::new_v4(),
        &std::env::temp_dir(),
    )
    .expect("the counter's host component starts");
    let count = hosting.run_until(std::time::Duration::from_secs(120), || {
        let content = be_host_stack::content(block)?;
        let count = CounterContent::decode(content.bytes()).ok()?.root().value();
        (count == 1).then_some(count)
    });
    assert_eq!(count, Some(1), "the host component added one to its block");
}
