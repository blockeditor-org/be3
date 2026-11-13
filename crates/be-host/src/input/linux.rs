use std::cell::{Cell, RefCell};
use std::time::Duration;

use be_block::InputSettings;
use be_host_desktop::desktop_host::{DesktopHost, Host};
use be_host_wayland::{KeyboardConfig, NamedLayout};
use be_sdk_api::{HostInputDevice, KeyboardLayout, KeyboardLayouts, KeyboardLayoutsWatch};
use beui_platform_drm::{InputConfig, InputControl, PointerDevice};

thread_local! {
    static CONTROL: RefCell<Option<InputControl>> = const { RefCell::new(None) };
    static KEYBOARD_REACHED: Cell<bool> = const { Cell::new(false) };
    static LAYOUTS: RefCell<Layouts> = RefCell::default();
}

#[derive(Default)]
struct Layouts {
    named: Vec<NamedLayout>,
    active: u32,
    keymap: Option<KeyboardConfig>,
    seat: Option<InputConfig>,
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
    KEYBOARD_REACHED.set(reach.keyboard);
    if reach.keyboard || reach.middle_click_paste {
        be_host_base::watches::publish::<be_sdk_api::InputReachWatch>(&reach);
    }
    be_host_desktop::remap::start(control.clone());
    CONTROL.with(|slot| *slot.borrow_mut() = control);
}

pub(crate) fn apply(settings: &InputSettings) {
    let keyboard = keyboard(settings);
    let tried = LAYOUTS.with_borrow(|layouts| {
        layouts
            .keymap
            .as_ref()
            .is_none_or(|tried| same_keymap(tried) != same_keymap(&keyboard))
    });
    let named = tried.then(|| be_host_wayland::named_layouts(&keyboard));
    let active = LAYOUTS.with_borrow_mut(|layouts| {
        match named {
            Some(Some(named)) => {
                if layouts.active as usize >= named.len() {
                    layouts.active = 0;
                }
                layouts.named = named;
            }
            Some(None) => {
                let drm_reports = CONTROL.with_borrow(Option::is_some);
                if KEYBOARD_REACHED.get() && !drm_reports {
                    be_host_base::notices::report(unusable(&keyboard));
                }
            }
            None => {}
        }
        layouts.keymap = Some(keyboard.clone());
        layouts.active
    });
    let seat = seat(settings, active);
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(seat.clone());
        }
    });
    LAYOUTS.with_borrow_mut(|layouts| layouts.seat = Some(seat));
    be_host_desktop::desktop_host::configure(|desktop| {
        desktop.primary_selection = settings.middle_click_paste();
    });
    be_host_desktop::remap::configure(settings);
    be_host_desktop::desktop_host::set_keyboard(&keyboard);
    be_host_desktop::desktop_host::set_layout(active);
    publish_layouts();
}

pub(crate) fn run_layout_requests() {
    for index in be_host_base::watches::take_calls(be_sdk_api::requested::switch_layout) {
        switch_layout(index);
    }
}

fn switch_layout(index: u32) {
    let seat = LAYOUTS.with_borrow_mut(|layouts| {
        if index as usize >= layouts.named.len() || index == layouts.active {
            return None;
        }
        layouts.active = index;
        let seat = layouts.seat.as_mut()?;
        seat.active_layout = index;
        Some(seat.clone())
    });
    let Some(seat) = seat else {
        return;
    };
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(seat);
        }
    });
    be_host_desktop::desktop_host::set_layout(index);
    publish_layouts();
}

fn publish_layouts() {
    if !KEYBOARD_REACHED.get() {
        return;
    }
    let value = LAYOUTS.with_borrow(|layouts| KeyboardLayouts {
        layouts: layouts
            .named
            .iter()
            .map(|named| KeyboardLayout {
                name: named.name.clone(),
                label: named.label.clone(),
            })
            .collect(),
        active: layouts.active,
    });
    be_host_base::watches::publish::<KeyboardLayoutsWatch>(&value);
}

fn same_keymap(keyboard: &KeyboardConfig) -> (&str, &str, &str) {
    (&keyboard.layout, &keyboard.variant, &keyboard.options)
}

fn unusable(keyboard: &KeyboardConfig) -> String {
    format!(
        "The keyboard layout {:?} ({:?}, {:?}) could not be compiled, so the last one stays.",
        keyboard.layout, keyboard.variant, keyboard.options
    )
}

fn host_device(device: &PointerDevice) -> HostInputDevice {
    HostInputDevice {
        device: device.id.clone(),
        defaults: device.defaults,
        pointing: device
            .pointing
            .as_ref()
            .map(|pointing| be_sdk_api::Pointing {
                automatic: pointing.automatic.clone(),
            }),
    }
}

fn seat(settings: &InputSettings, active_layout: u32) -> InputConfig {
    InputConfig {
        layout: settings.keyboard_layout.clone().unwrap_or_default(),
        variant: settings.keyboard_variant.clone().unwrap_or_default(),
        options: settings.keyboard_options.clone().unwrap_or_default(),
        active_layout,
        repeat_delay: Duration::from_millis(settings.repeat_delay().into()),
        repeat_interval: Duration::from_millis((1000 / settings.repeat_rate()).into()),
        every_pointer: settings.every_pointer,
        pointers: settings
            .pointers
            .iter()
            .map(|(device, settings)| (device.clone(), *settings))
            .collect(),
        displays: settings
            .displays
            .iter()
            .map(|(device, display)| (device.clone(), display.clone()))
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
