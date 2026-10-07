use block_editor_beui::be_block::InputSettingsContent;
use block_editor_beui::be_block::input_settings::InputDevice;
use block_editor_beui::{Editor, EditorHost, HostInputDevice};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::InputSettingsApp;

mod a_pointer_follows_its_device_default_until_it_is_set;
mod resetting_a_layout_returns_it_to_the_default;
mod typing_a_layout_stores_it;

fn editor(devices: Vec<HostInputDevice>) -> BeuiTest<InputSettingsApp> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_input_devices(devices);
    let mut editor = BeuiTest::new(Editor::new(host, Uuid::new_v4()));
    editor.hold(None, InputSettingsContent::default());
    editor.run();
    editor
}

fn content(editor: &BeuiTest<InputSettingsApp>) -> InputSettingsContent {
    editor.content(None)
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
