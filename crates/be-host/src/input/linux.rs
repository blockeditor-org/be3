use std::cell::RefCell;
use std::time::Duration;

use be_block::InputSettings;
use be_host_desktop::desktop_host::{DesktopHost, Host};
use be_host_wayland::KeyboardConfig;
use be_sdk_api::HostInputDevice;
use beui_platform_drm::{InputConfig, InputControl, PointerDevice};

thread_local! {
    static CONTROL: RefCell<Option<InputControl>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::native::Setup) {
    let control = setup.get::<InputControl>().cloned();
    if let Some(control) = &control {
        control.on_devices(|devices| {
            be_host_base::watches::publish::<be_sdk_api::InputDevicesWatch>(
                &devices.iter().map(host_device).collect(),
            );
        });
    }
    let reach = be_sdk_api::InputReach {
        keyboard: control.is_some() || Host::running(),
        middle_click_paste: Host::running(),
    };
    if reach.keyboard || reach.middle_click_paste {
        be_host_base::watches::publish::<be_sdk_api::InputReachWatch>(&reach);
    }
    be_host_desktop::remap::start(control.clone());
    CONTROL.with(|slot| *slot.borrow_mut() = control);
}

pub(crate) fn apply(settings: &InputSettings) {
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(seat(settings));
        }
    });
    Host::set_primary_selection(settings.middle_click_paste());
    be_host_desktop::remap::configure(settings);
    if !be_host_desktop::desktop_host::set_keyboard(&keyboard(settings)) {
        eprintln!(
            "be-host: the keymap {:?} could not be compiled for Wayland clients",
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
