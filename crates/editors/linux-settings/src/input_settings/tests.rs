use block_editor_beui::be_block::InputSettingsContent;
use block_editor_beui::be_block::input_settings::InputDevice;
use block_editor_beui::beui::Key;
use block_editor_beui::{EditorHost, HostInputDevice, InputDevices, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::input_settings::InputSettingsApp;

mod a_device_follows_its_default_until_it_is_overridden;
mod a_narrow_page_stacks_each_control_under_its_label;
mod every_pointer_reads_as_mixed_until_it_is_set;
mod middle_click_paste_is_off_until_it_is_switched_on;
mod opening_the_remaps_makes_them_with_the_remapper_and_opens_them;
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
