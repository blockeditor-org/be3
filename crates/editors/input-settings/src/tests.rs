use block_editor_beui::be_block::InputSettingsContent;
use block_editor_beui::be_block::input_settings::InputDevice;
use block_editor_beui::beui::{Key, Vec2};
use block_editor_beui::{
    EditorHost, HostInputDevice, InputDevices, OwnedEditor, RemapRequest, RemapState, Remaps,
};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::InputSettingsApp;

mod a_device_follows_its_default_until_it_is_overridden;
mod a_device_waiting_on_remaps_asks_the_host_to_show_them;
mod every_pointer_reads_as_mixed_until_it_is_set;
mod middle_click_paste_is_off_until_it_is_switched_on;
mod remaps_that_do_not_read_say_why_and_store_nothing;
mod remaps_written_as_text_are_stored_once_applied;
mod resetting_a_layout_returns_it_to_the_default;
mod typing_a_layout_stores_it;

fn editor(devices: Vec<HostInputDevice>) -> BeuiTest<InputSettingsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_host_value::<InputDevices>(&devices);
    let mut editor = BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.hold(None, InputSettingsContent::default());
    editor.run();
    editor
}

const TALL: Vec2 = Vec2::new(800.0, 1600.0);

fn remaps_editor(state: RemapState) -> BeuiTest<InputSettingsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_host_value::<InputDevices>(&vec![mouse()]);
    host.set_host_value::<Remaps>(&state);
    let mut editor = BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4())).with_size(TALL);
    editor.hold(None, InputSettingsContent::default());
    editor.run();
    editor
}

fn type_remaps(editor: &mut BeuiTest<InputSettingsApp>, text: &str) {
    editor.click("input-settings.remaps");
    editor.run();
    editor.text(text);
    editor.run();
    editor.click("input-settings.remaps.apply");
    editor.run();
}

fn content(editor: &BeuiTest<InputSettingsApp>) -> InputSettingsContent {
    editor.content(None)
}

fn choose_pointer(editor: &mut BeuiTest<InputSettingsApp>, index: usize) {
    editor.click("input-settings.pointer");
    editor.run();
    editor.key_press(Key::Home);
    editor.run();
    for _ in 0..index {
        editor.key_press(Key::ArrowDown);
        editor.run();
    }
    editor.key_press(Key::Enter);
    editor.run();
}

fn mouse() -> HostInputDevice {
    HostInputDevice {
        name: "Mouse".to_owned(),
        vendor: 0x046d,
        product: 0xc52b,
        speed: Some(0.0),
        tap_to_click: None,
        natural_scroll: Some(false),
    }
}

fn touchpad() -> HostInputDevice {
    HostInputDevice {
        name: "Touchpad".to_owned(),
        vendor: 0x06cb,
        product: 0x0001,
        speed: Some(0.0),
        tap_to_click: Some(true),
        natural_scroll: Some(false),
    }
}

fn touchpad_identity() -> InputDevice {
    InputDevice {
        name: "Touchpad".to_owned(),
        vendor: 0x06cb,
        product: 0x0001,
    }
}
