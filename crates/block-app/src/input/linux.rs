use std::cell::RefCell;
use std::time::Duration;

use be_block::InputSettings;
use be_wayland::KeyboardConfig;
use beui_platform_drm::{InputConfig, InputControl, PointerDevice};
use block_plugin_api::HostInputDevice;

thread_local! {
    static CONTROL: RefCell<Option<InputControl>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::Setup) {
    let control = setup.get::<InputControl>().cloned();
    if let Some(control) = &control {
        control.on_devices(|devices| {
            crate::plugin_host::publish::<block_plugin_api::InputDevicesWatch>(
                &devices.iter().map(host_device).collect(),
            );
        });
    }
    crate::remap::start(control.clone());
    CONTROL.with(|slot| *slot.borrow_mut() = control);
}

pub(crate) fn apply(settings: &InputSettings) {
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(seat(settings));
        }
    });
    crate::wayland::set_primary_selection(settings.middle_click_paste());
    crate::remap::configure(settings);
    if !crate::wayland::set_keyboard(&keyboard(settings)) {
        eprintln!(
            "block-app: the keymap {:?} could not be compiled for Wayland clients",
            settings.keyboard_layout
        );
    }
}

fn host_device(device: &PointerDevice) -> HostInputDevice {
    HostInputDevice {
        device: device.id.clone(),
        defaults: device.defaults,
    }
}

fn seat(settings: &InputSettings) -> InputConfig {
    InputConfig {
        layout: settings.keyboard_layout.clone().unwrap_or_default(),
        variant: settings.keyboard_variant.clone().unwrap_or_default(),
        options: settings.keyboard_options.clone().unwrap_or_default(),
        repeat_delay: Duration::from_millis(settings.repeat_delay().into()),
        repeat_interval: Duration::from_millis((1000 / settings.repeat_rate()).into()),
        every_pointer: settings.every_pointer,
        pointers: settings
            .pointers
            .iter()
            .map(|(device, settings)| (device.clone(), *settings))
            .collect(),
    }
}

fn keyboard(settings: &InputSettings) -> KeyboardConfig {
    KeyboardConfig {
        layout: settings.keyboard_layout.clone().unwrap_or_default(),
        variant: settings.keyboard_variant.clone().unwrap_or_default(),
        options: settings.keyboard_options.clone().unwrap_or_default(),
        repeat_delay: settings.repeat_delay().cast_signed(),
        repeat_rate: settings.repeat_rate().cast_signed(),
    }
}
