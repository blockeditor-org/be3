use super::*;

use be_block::{Theme, UiSettings as Settings, UiSettingsContent};

fn theme_of(shared: &Shared, block: Uuid) -> Option<Theme> {
    let held = shared.blocks.get(&block)?;
    UiSettingsContent::decode(&held.bytes)
        .ok()
        .map(|settings| settings.root().theme())
}

#[test]
fn a_held_block_stays_open_when_its_editors_close() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    hold(block, UiSettingsContent::CONTENT_TYPE);
    hold(block, UiSettingsContent::CONTENT_TYPE);
    wait_until("opened the held block", |shared| {
        theme_of(shared, block) == Some(Theme::Dark)
    });
    close(block);

    operate(
        block,
        UiSettingsContent::encode_operation(&Settings::set_theme(Some(Theme::Light))),
    );
    wait_until("kept the held block live", |shared| {
        theme_of(shared, block) == Some(Theme::Light)
    });
}
