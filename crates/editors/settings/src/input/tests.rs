use block_editor_beui::be_block::InputSettingsContent;
use block_editor_beui::be_block::input_settings::{InputDevice, PointerSettings, PointerSpeed};
use block_editor_beui::beui::{Key, Modifiers};
use block_editor_beui::{EditorHost, HostInputDevice, InputDevicesWatch, OwnedEditor};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use uuid::Uuid;

use crate::input::InputSettingsApp;

mod a_device_follows_its_default_until_it_is_overridden;
mod a_narrow_page_stacks_each_control_under_its_label;
mod a_pointer_kept_here_stays_on_this_device;
mod every_pointer_reads_as_mixed_until_it_is_set;
mod middle_click_paste_is_off_until_it_is_switched_on;
mod opening_the_remaps_makes_them_with_the_remapper_and_opens_them;
mod outside_a_session_input_is_not_offered;
mod resetting_a_layout_returns_it_to_the_default;
mod typing_a_layout_stores_it;

fn editor(devices: Vec<HostInputDevice>) -> BeuiTest<InputSettingsApp> {
    let host = EditorHost::default();
    host.set_watched::<InputDevicesWatch>(&devices);
    open(host)
}

fn open(host: EditorHost) -> BeuiTest<InputSettingsApp> {
    host.set_editable(true);
    let mut editor = BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.hold(None, InputSettingsContent::default());
    editor.run();
    editor
}

fn content(editor: &BeuiTest<InputSettingsApp>) -> InputSettingsContent {
    editor.content(None)
}

fn reset(editor: &mut BeuiTest<InputSettingsApp>, row: &str) {
    editor.click(format!("{row}.menu"));
    editor.click(format!("{row}.reset"));
}

fn choose_pointer(editor: &mut BeuiTest<InputSettingsApp>, index: usize) {
    editor.click("input.pointer");
    editor.key(Key::Home, Modifiers::NONE);
    for _ in 0..index {
        editor.key(Key::ArrowDown, Modifiers::NONE);
    }
    editor.key(Key::Enter, Modifiers::NONE);
}

fn mouse() -> HostInputDevice {
    HostInputDevice {
        device: InputDevice {
            name: "Mouse".to_owned(),
            vendor: 0x046d,
            product: 0xc52b,
        },
        defaults: PointerSettings {
            speed: Some(PointerSpeed::new(0.0)),
            tap_to_click: None,
            natural_scroll: Some(false),
        },
    }
}

fn touchpad() -> HostInputDevice {
    HostInputDevice {
        device: InputDevice {
            name: "Touchpad".to_owned(),
            vendor: 0x06cb,
            product: 0x0001,
        },
        defaults: PointerSettings {
            speed: Some(PointerSpeed::new(0.0)),
            tap_to_click: Some(true),
            natural_scroll: Some(false),
        },
    }
}

fn touchpad_identity() -> InputDevice {
    InputDevice {
        name: "Touchpad".to_owned(),
        vendor: 0x06cb,
        product: 0x0001,
    }
}
