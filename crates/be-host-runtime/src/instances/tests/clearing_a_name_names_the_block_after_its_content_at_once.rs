use super::*;

use be_block::{BlockContent, LiveEdit};
use be_type_browser_tab::{BrowserTabContent, HistoryItem};

fn name_of(block: Uuid) -> Option<String> {
    be_host_stack::node(block).and_then(|node| node.metadata.name())
}

#[test]
fn clearing_a_name_names_the_block_after_its_content_at_once() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let mut instances = placed_on(block, BrowserTabContent::CONTENT_TYPE);
    instances.next_screens(PASS);
    to_replica(
        &mut instances,
        block,
        &be_sdk_type_guest::editor::FromEditor::Resend,
    );
    be_host_stack::wait_until(|shared| {
        (shared.blocks.contains_key(&block) && shared.graph.get(block).is_some()).then_some(())
    });

    be_host_stack::set_name(block, Some("Research".to_owned()));
    let shown = be_host_stack::content(block)
        .and_then(|content| BrowserTabContent::decode(content.bytes()).ok())
        .expect("the new stack holds the tab");
    let edit = shown
        .root()
        .replace(&HistoryItem::new("https://example.com/", "Example Domain"));
    be_host_stack::operate(block, BrowserTabContent::encode_operation(&edit));
    be_host_stack::wait_until(|shared| {
        let held = shared.blocks.get(&block)?;
        let tab = BrowserTabContent::decode(held.bytes()).ok()?;
        (tab.root().current().title == "Example Domain").then_some(())
    });
    assert_eq!(name_of(block).as_deref(), Some("Research"));

    be_host_stack::set_name(block, None);
    assert_eq!(name_of(block).as_deref(), Some("Example Domain"));
}
