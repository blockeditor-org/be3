use super::*;

use be_block::{BlockContent, LiveEdit};
use browser_tab_types::{BrowserTabContent, HistoryItem};

fn titled(block: Uuid, title: &str) {
    let shown = block_app_stack::content(block)
        .and_then(|content| BrowserTabContent::decode(content.bytes()).ok())
        .expect("the new stack holds the tab");
    let edit = shown
        .root()
        .replace(&HistoryItem::new("https://example.com/", title));
    block_app_stack::operate(block, BrowserTabContent::encode_operation(&edit));
    block_app_stack::wait_until(|shared| {
        let held = shared.blocks.get(&block)?;
        let tab = BrowserTabContent::decode(held.bytes()).ok()?;
        (tab.root().current().title == title).then_some(())
    });
}

fn named(block: Uuid, name: &str) {
    block_app_stack::wait_until(|shared| {
        let node = shared.graph.get(block)?;
        (node.metadata.name().as_deref() == Some(name)).then_some(())
    });
}

#[test]
fn a_block_is_named_after_its_content_until_someone_names_it() {
    let harness = block_app_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed_on(block, BrowserTabContent::CONTENT_TYPE);
    instances.next_screens(PASS);
    to_replica(
        &mut instances,
        block,
        &block_type_guest::editor::FromEditor::Resend,
    );
    block_app_stack::wait_until(|shared| {
        (shared.blocks.contains_key(&block) && shared.graph.get(block).is_some()).then_some(())
    });

    titled(block, "Example Domain");
    instances.next_screens(PASS);
    named(block, "Example Domain");

    block_app_stack::set_name(block, Some("Research".to_owned()));
    titled(block, "Another Page");
    instances.next_screens(PASS);
    named(block, "Research");
}
