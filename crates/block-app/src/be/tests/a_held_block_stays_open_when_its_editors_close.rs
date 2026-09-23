use super::*;

use be_block::{UiSettings as Settings, UiSettingsContent};
use block::Block;
use block_client::blocks::ui_settings::UiSettings;

fn zoom_of(shared: &Shared, block: Uuid) -> Option<f32> {
    let held = shared.blocks.get(&block)?;
    UiSettingsContent::decode(&held.bytes)
        .ok()
        .map(|settings| settings.root().zoom())
}

#[test]
fn a_held_block_stays_open_when_its_editors_close() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    hold(block, UiSettings::TYPE_ID);
    hold(block, UiSettings::TYPE_ID);
    wait_until("opened the held block", |shared| {
        zoom_of(shared, block) == Some(1.0)
    });
    close(block);

    operate(
        block,
        UiSettingsContent::encode_operation(&Settings::set_zoom(1.5)),
    );
    wait_until("kept the held block live", |shared| {
        zoom_of(shared, block) == Some(1.5)
    });
}
