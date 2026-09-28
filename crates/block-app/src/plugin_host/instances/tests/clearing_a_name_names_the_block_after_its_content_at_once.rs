use super::*;

use std::time::Duration;

use be_block::{BlockContent, BrowserTabContent, HistoryItem, LiveEdit};

fn name_of(block: Uuid) -> Option<String> {
    crate::be::node(block).and_then(|node| node.metadata.name)
}

#[test]
fn clearing_a_name_names_the_block_after_its_content_at_once() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed_on(block, BrowserTabContent::CONTENT_TYPE);
    instances.next_screens(PASS);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        (shared.blocks.contains_key(&block) && shared.graph.get(block).is_some()).then_some(())
    })
    .expect("the new stack never held the tab");

    crate::be::set_name(block, Some("Research".to_owned()));
    let shown = crate::be::content(block)
        .and_then(|content| BrowserTabContent::decode(&content.bytes).ok())
        .expect("the new stack holds the tab");
    let edit = shown
        .root()
        .replace(&HistoryItem::new("https://example.com/", "Example Domain"));
    crate::be::operate_from(block, 0, BrowserTabContent::encode_operation(&edit));
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let tab = BrowserTabContent::decode(&held.bytes).ok()?;
        (tab.root().current().title == "Example Domain").then_some(())
    })
    .expect("the new stack never took the title");
    assert_eq!(name_of(block).as_deref(), Some("Research"));

    crate::be::set_name(block, None);
    assert_eq!(name_of(block).as_deref(), Some("Example Domain"));
}
