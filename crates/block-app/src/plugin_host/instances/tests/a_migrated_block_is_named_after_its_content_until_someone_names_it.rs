use super::*;

use std::time::Duration;

use be_block::{BlockContent, BrowserTabContent, BrowserTabOp, HistoryItem, LiveEdit};
use block::Block;
use block_client::blocks::web_browser_tab::WebBrowserTab;

fn titled(block: Uuid, title: &str) {
    crate::be::operate_from(
        block,
        0,
        BrowserTabContent::encode_operation(&BrowserTabOp::Replace(HistoryItem {
            url: "https://example.com/".into(),
            title: title.into(),
        })),
    );
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        let held = shared.blocks.get(&block)?;
        let tab = BrowserTabContent::decode(&held.bytes).ok()?;
        (tab.current().title == title).then_some(())
    })
    .expect("the new stack never took the title");
}

#[test]
fn a_migrated_block_is_named_after_its_content_until_someone_names_it() {
    let harness = crate::be::Harness::start();
    harness.connect();
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    let old = client.create_block(WebBrowserTab::new());
    let mut instances = placed_with(&client, old.id(), WebBrowserTab::TYPE_ID);
    instances.next_screens(PASS);
    crate::be::wait_for(Duration::from_secs(20), |shared| {
        shared.blocks.contains_key(&old.id()).then_some(())
    })
    .expect("the new stack never held the tab");

    titled(old.id(), "Example Domain");
    instances.next_screens(PASS);
    assert_eq!(old.name(), Some("Example Domain".to_owned()));

    old.set_name("Research");
    titled(old.id(), "Another Page");
    instances.next_screens(PASS);
    assert_eq!(old.name(), Some("Research".to_owned()));
}
