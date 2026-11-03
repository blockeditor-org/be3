use be_sdk_beui::be_block::InputSettingsContent;
use be_sdk_beui::be_block::input_settings::{InputDevice, PointerSettings, PointerSpeed};
use be_sdk_beui::beui::{Key, Modifiers};
use be_sdk_beui::{
    DisplayMode, DisplaysWatch, EditorHost, HostDisplay, HostInputDevice, InputDevicesWatch,
    InputReach, InputReachWatch, OwnedEditor, Pointing, Rotation,
};
use be_sdk_test::BeuiTest;
use uuid::Uuid;

use crate::input::InputSettingsRoot;

mod a_device_follows_its_default_until_it_is_overridden;
mod a_touchscreen_is_put_on_the_display_chosen_for_it;
mod a_narrow_page_stacks_each_control_under_its_label;
mod a_pointer_kept_here_stays_on_this_device;
mod every_pointer_reads_as_mixed_until_it_is_set;
mod in_a_windowed_desktop_the_keyboard_and_pasting_are_set_but_not_the_pointers;
mod middle_click_paste_is_off_until_it_is_switched_on;
mod opening_the_remaps_makes_them_with_the_remapper_and_opens_them;
mod outside_a_session_input_is_not_offered;
mod resetting_a_layout_returns_it_to_the_default;
mod typing_a_layout_stores_it;

fn editor(devices: Vec<HostInputDevice>) -> BeuiTest<InputSettingsRoot> {
    let host = EditorHost::default();
    host.set_watched::<InputDevicesWatch>(&devices);
    host.set_watched::<InputReachWatch>(&InputReach {
        keyboard: true,
        middle_click_paste: true,
    });
    open(host)
}

fn open(host: EditorHost) -> BeuiTest<InputSettingsRoot> {
    host.set_editable(true);
    let mut editor = BeuiTest::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.hold(None, InputSettingsContent::default());
    editor.run();
    editor
}

fn content(editor: &BeuiTest<InputSettingsRoot>) -> InputSettingsContent {
    editor.content(None)
}

fn reset(editor: &mut BeuiTest<InputSettingsRoot>, row: &str) {
    editor.click(format!("{row}.menu"));
    editor.run();
    editor.click(format!("{row}.reset"));
    editor.run();
}

fn choose_pointer(editor: &mut BeuiTest<InputSettingsRoot>, index: usize) {
    editor.click("input.pointer");
    editor.run();
    editor.key(Key::Home, Modifiers::NONE);
    editor.run();
    for _ in 0..index {
        editor.key(Key::ArrowDown, Modifiers::NONE);
        editor.run();
    }
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
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
        pointing: None,
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
        pointing: None,
    }
}

fn touchpad_identity() -> InputDevice {
    InputDevice {
        name: "Touchpad".to_owned(),
        vendor: 0x06cb,
        product: 0x0001,
    }
}

fn touchscreen() -> HostInputDevice {
    HostInputDevice {
        device: touchscreen_identity(),
        defaults: PointerSettings::default(),
        pointing: Some(Pointing {
            automatic: Some("laptop".to_owned()),
        }),
    }
}

fn touchscreen_identity() -> InputDevice {
    InputDevice {
        name: "ELAN Touchscreen".to_owned(),
        vendor: 0x04f3,
        product: 0x2a1c,
    }
}

fn display(id: &str, name: &str, connector: &str) -> HostDisplay {
    let mode = DisplayMode {
        width: 1920,
        height: 1080,
        refresh_millihertz: 60_000,
    };
    HostDisplay {
        id: id.to_owned(),
        name: name.to_owned(),
        connector: connector.to_owned(),
        modes: vec![mode],
        default: mode,
        current: mode,
        rotation: Rotation::Normal,
        rect: be_sdk_beui::Rect::ZERO,
        scale: 1.0,
    }
}
