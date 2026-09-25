use super::*;

use std::time::Duration;

use be_block::{BlockContent, BrowserTabContent, HistoryItem, LiveEdit};

fn titled(block: Uuid, title: &str) {
    let shown = crate::be::content(block)
        .and_then(|content| BrowserTabContent::decode(&content.bytes).ok())
        .expect("the new stack holds the tab");
    let edit = shown
        .root()
        .replace(&HistoryItem::new("https://example.com/", title));
    crate::be::operate_from(block, 0, BrowserTabContent::encode_operation(&edit));
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let tab = BrowserTabContent::decode(&held.bytes).ok()?;
        (tab.root().current().title == title).then_some(())
    })
    .expect("the new stack never took the title");
}

fn named(block: Uuid, name: &str) {
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let node = shared.graph.get(block)?;
        (node.metadata.name.as_deref() == Some(name)).then_some(())
    })
    .unwrap_or_else(|| panic!("the block was never named {name}"));
}

#[test]
fn a_migrated_block_is_named_after_its_content_until_someone_names_it() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed_on(block, BrowserTabContent::CONTENT_TYPE);
    instances.next_screens(PASS);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        (shared.blocks.contains_key(&block) && shared.graph.get(block).is_some()).then_some(())
    })
    .expect("the new stack never held the tab");

    titled(block, "Example Domain");
    instances.next_screens(PASS);
    named(block, "Example Domain");

    crate::be::set_name(block, Some("Research".to_owned()));
    titled(block, "Another Page");
    instances.next_screens(PASS);
    named(block, "Research");
}
