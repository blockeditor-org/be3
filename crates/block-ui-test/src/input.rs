use beui::Event;
use beui_plugin_input::{
    back_phase, pointer_button, protocol_key, protocol_modifiers, touch_phase,
};
use block_plugin_api::{InputEvent, Modifiers, WheelUnit};

#[derive(Default)]
pub(crate) struct Input {
    modifiers: Modifiers,
    held: beui::Modifiers,
}

impl Input {
    pub(crate) fn held(&self) -> beui::Modifiers {
        self.held
    }

    pub(crate) fn normalize(&mut self, events: Vec<Event>) -> Vec<InputEvent> {
        let mut output = Vec::new();
        for event in events {
            self.event(event, &mut output);
        }
        output
    }

    fn modifiers(&mut self, modifiers: beui::Modifiers, output: &mut Vec<InputEvent>) {
        self.held = modifiers;
        let modifiers = protocol_modifiers(modifiers);
        if self.modifiers != modifiers {
            self.modifiers = modifiers;
            output.push(InputEvent::Modifiers(modifiers));
        }
    }

    fn event(&mut self, event: Event, output: &mut Vec<InputEvent>) {
        match event {
            Event::PointerMoved(position) => output.push(InputEvent::PointerMoved {
                x: position.x,
                y: position.y,
            }),
            Event::PointerGone => output.push(InputEvent::PointerLeft),
            Event::PointerMotion(delta) => output.push(InputEvent::PointerMotion {
                x: delta.x,
                y: delta.y,
            }),
            Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => {
                self.modifiers(modifiers, output);
                output.push(InputEvent::PointerButton {
                    button: pointer_button(button),
                    pressed,
                    x: pos.x,
                    y: pos.y,
                });
            }
            Event::Scroll(delta) => {
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: WheelUnit::Pixels,
                });
            }
            Event::ScrollEnded => output.push(InputEvent::WheelEnded),
            Event::Zoom(factor) => output.push(InputEvent::Zoom { factor }),
            Event::Touch {
                id,
                phase,
                pos,
                force,
            } => output.push(InputEvent::Touch {
                device: id.device,
                finger: id.finger,
                phase: touch_phase(phase),
                x: pos.x,
                y: pos.y,
                force,
            }),
            Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } => {
                self.modifiers(modifiers, output);
                output.push(InputEvent::Key {
                    key: protocol_key(key),
                    pressed,
                    repeat,
                });
            }
            Event::Modifiers(modifiers) => self.modifiers(modifiers, output),
            Event::Text(text) => output.push(InputEvent::Text(text)),
            Event::Ime(ime) => output.push(InputEvent::Ime(beui_plugin_input::protocol_ime(&ime))),
            Event::Focus(focused) => output.push(InputEvent::Focus(focused)),
            Event::Back(gesture) => output.push(InputEvent::Back(back_phase(gesture))),
            Event::InterceptedKey(press) => {
                self.modifiers(press.modifiers, output);
                output.push(InputEvent::InterceptedKey {
                    key: protocol_key(press.key),
                    pressed: press.pressed,
                    repeat: press.repeat,
                });
            }
            _ => {}
        }
    }
}
