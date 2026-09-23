use std::sync::Arc;

use block_client::BlockClient;
use block_client::blocks::hotbar::Hotbar as HotbarBlock;
use block_editor_plugin::be_block::hotbar::{Hotbar, HotbarContent, HotbarSlot};
use block_editor_plugin::{Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::HotbarApp;

mod unpinning_a_component_from_the_hotbar_takes_its_row_away;

fn editor(slots: Vec<HotbarSlot>) -> ContentHarness<HotbarApp> {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(HotbarBlock::new());
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), client, block.id());
    let mut harness = ContentHarness::new(BeuiTest::new(editor), host);
    harness.hold(
        None,
        HotbarContent::new(&Hotbar {
            slots: slots.into_iter().collect(),
        }),
    );
    harness.run();
    harness
}
