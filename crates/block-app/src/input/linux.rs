use std::cell::RefCell;
use std::time::Duration;

use be_block::InputSettings;
use be_wayland::KeyboardConfig;
use beui_adapter_drm::{InputConfig, InputControl};

thread_local! {
    static CONTROL: RefCell<Option<InputControl>> = const { RefCell::new(None) };
}

pub(crate) fn start(setup: &beui::Setup) {
    let control = setup.get::<InputControl>().cloned();
    CONTROL.with(|slot| *slot.borrow_mut() = control);
}

pub(crate) fn apply(settings: &InputSettings) {
    CONTROL.with(|control| {
        if let Some(control) = control.borrow().as_ref() {
            control.set(seat(settings));
        }
    });
    if !crate::wayland::set_keyboard(&keyboard(settings)) {
        eprintln!(
            "block-app: the keymap {:?} could not be compiled for Wayland clients",
            settings.keyboard_layout
        );
    }
}

fn seat(settings: &InputSettings) -> InputConfig {
    let rate = settings.repeat_rate.per_second();
    InputConfig {
        layout: settings.keyboard_layout.clone(),
        variant: settings.keyboard_variant.clone(),
        options: settings.keyboard_options.clone(),
        repeat_delay: Duration::from_millis(settings.repeat_delay.milliseconds().into()),
        repeat_interval: Duration::from_millis((1000 / rate).into()),
        pointer_speed: settings.pointer_speed.get().into(),
        tap_to_click: settings.tap_to_click,
        natural_scroll: settings.natural_scroll,
    }
}

fn keyboard(settings: &InputSettings) -> KeyboardConfig {
    KeyboardConfig {
        layout: settings.keyboard_layout.clone(),
        variant: settings.keyboard_variant.clone(),
        options: settings.keyboard_options.clone(),
        repeat_delay: settings.repeat_delay.milliseconds().cast_signed(),
        repeat_rate: settings.repeat_rate.per_second().cast_signed(),
    }
}
