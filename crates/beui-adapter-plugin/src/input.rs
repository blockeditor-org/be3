use beui_core::geometry::{pos2, vec2};
use beui_core::input::{Event, KeyPress, Modifiers, Pen, TouchId};
use beui_core::runner::{Input, Keystroke, ScrollDelta};
use block_editor_plugin::{InputEvent, Region, WheelUnit};

pub(crate) fn translate(region: &Region, event: &InputEvent) -> Input {
    let origin = region.rect.min.to_vec2();
    let scale = region.scale_factor;
    let at = |x: f32, y: f32| pos2((x + origin.x) * scale, (y + origin.y) * scale);
    match event {
        InputEvent::PointerMoved { x, y } => Event::PointerMoved(at(*x, *y)).into(),
        InputEvent::Predicted { x, y } => Event::Predicted(at(*x, *y)).into(),
        InputEvent::PointerLeft => Event::PointerGone.into(),
        InputEvent::PointerButton {
            button,
            pressed,
            x,
            y,
        } => Event::PointerButton {
            pos: at(*x, *y),
            button: *button,
            pressed: *pressed,
            modifiers: Modifiers::NONE,
        }
        .into(),
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
            phase: *phase,
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
            key: Some(*key),
            pressed: *pressed,
            repeat: *repeat,
            code: None,
            text: text.clone(),
        }
        .into(),
        InputEvent::Text(text) => Event::Text(text.clone()).into(),
        InputEvent::Paste(pasted) => Event::Paste(pasted.clone()).into(),
        InputEvent::Modifiers(modifiers) => Event::Modifiers(*modifiers).into(),
        InputEvent::WheelEnded => Event::ScrollEnded.into(),
        InputEvent::Zoom { factor } => Event::Zoom(*factor).into(),
        InputEvent::PointerMotion { x, y } => Event::PointerMotion(vec2(*x, *y) * scale).into(),
        InputEvent::Focus(focused) => Event::Focus(*focused).into(),
        InputEvent::Ime(ime) => Event::Ime(ime.clone()).into(),
        InputEvent::InterceptedKey {
            key,
            pressed,
            repeat,
        } => Event::InterceptedKey(KeyPress {
            key: *key,
            pressed: *pressed,
            repeat: *repeat,
            modifiers: Modifiers::NONE,
        })
        .into(),
        InputEvent::InterceptedTap { key } => Event::InterceptedTap(*key).into(),
        InputEvent::Back(gesture) => Event::Back(*gesture).into(),
        InputEvent::Pen {
            phase,
            x,
            y,
            pressure,
            tilt_x,
            tilt_y,
            eraser,
        } => Event::Pen {
            phase: *phase,
            pos: at(*x, *y),
            pressure: *pressure,
            pen: Pen {
                tilt: vec2(*tilt_x, *tilt_y),
                eraser: *eraser,
            },
        }
        .into(),
    }
}
