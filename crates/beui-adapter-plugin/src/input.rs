use beui_core::geometry::{pos2, vec2};
use beui_core::input::{Event, KeyPress, Modifiers, TouchId};
use beui_core::runner::{Input, Keystroke, ScrollDelta};
use block_editor_plugin::{InputEvent, Region, WheelUnit};

pub(crate) fn translate(region: &Region, event: &InputEvent) -> Option<Input> {
    let origin = region.rect.min.to_vec2();
    let scale = region.scale_factor;
    let at = |x: f32, y: f32| pos2((x + origin.x) * scale, (y + origin.y) * scale);
    let input: Input = match event {
        InputEvent::PointerMoved { x, y } => Event::PointerMoved(at(*x, *y)).into(),
        InputEvent::PointerLeft => Event::PointerGone.into(),
        InputEvent::PointerButton {
            button,
            pressed,
            x,
            y,
        } => {
            let button = beui_plugin_input::beui_button(*button)?;
            Event::PointerButton {
                pos: at(*x, *y),
                button,
                pressed: *pressed,
                modifiers: Modifiers::NONE,
            }
            .into()
        }
        InputEvent::Wheel { x, y, unit } => {
            let delta = vec2(*x, *y);
            match unit {
                WheelUnit::Pixels => ScrollDelta::Pixels(delta * scale),
                WheelUnit::Lines => ScrollDelta::Lines(delta),
                WheelUnit::Pages => ScrollDelta::Pages(delta),
            }
            .into()
        }
        InputEvent::Touch {
            device,
            finger,
            phase,
            x,
            y,
            force,
        } => Event::Touch {
            id: TouchId {
                device: *device,
                finger: *finger,
            },
            phase: beui_plugin_input::beui_touch_phase(*phase),
            pos: at(*x, *y),
            force: *force,
        }
        .into(),
        InputEvent::Key {
            key,
            pressed,
            repeat,
            text,
        } => Keystroke {
            key: beui_plugin_input::beui_key(*key),
            pressed: *pressed,
            repeat: *repeat,
            code: None,
            text: text.clone(),
        }
        .into(),
        InputEvent::Text(text) => Event::Text(text.clone()).into(),
        InputEvent::Paste(pasted) => {
            Event::Paste(beui_plugin_input::beui_pasted(pasted.clone())).into()
        }
        InputEvent::Modifiers(modifiers) => {
            Event::Modifiers(beui_plugin_input::beui_modifiers(*modifiers)).into()
        }
        InputEvent::WheelEnded => Event::ScrollEnded.into(),
        InputEvent::Zoom { factor } => Event::Zoom(*factor).into(),
        InputEvent::PointerMotion { x, y } => Event::PointerMotion(vec2(*x, *y) * scale).into(),
        InputEvent::Focus(focused) => Event::Focus(*focused).into(),
        InputEvent::Ime(ime) => Event::Ime(beui_plugin_input::beui_ime(ime)).into(),
        InputEvent::InterceptedKey {
            key,
            pressed,
            repeat,
        } => {
            let key = beui_plugin_input::beui_key(*key)?;
            Event::InterceptedKey(KeyPress {
                key,
                pressed: *pressed,
                repeat: *repeat,
                modifiers: Modifiers::NONE,
            })
            .into()
        }
        InputEvent::InterceptedTap { key } => {
            let key = beui_plugin_input::beui_key(*key)?;
            Event::InterceptedTap(key).into()
        }
        InputEvent::Back(phase) => Event::Back(beui_plugin_input::beui_back(*phase)).into(),
    };
    Some(input)
}
