use beui::{Event, Vec2};
use block_plugin_api::{ImeInput, InputEvent, Modifiers, WheelUnit};
use block_ui::input::{pointer_button, protocol_key, protocol_modifiers, touch_phase};

#[derive(Default)]
pub(crate) struct Input {
    modifiers: Modifiers,
    held: beui::Modifiers,
}

impl Input {
    pub(crate) fn held(&self) -> beui::Modifiers {
        self.held
    }

    pub(crate) fn normalize(&mut self, events: Vec<Event>, origin: Vec2) -> Vec<InputEvent> {
        let mut output = Vec::new();
        for event in events {
            self.event(event, origin, &mut output);
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

    fn event(&mut self, event: Event, origin: Vec2, output: &mut Vec<InputEvent>) {
        match event {
            Event::PointerMoved(position) => {
                let position = position - origin;
                output.push(InputEvent::PointerMoved {
                    x: position.x,
                    y: position.y,
                });
            }
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
                let position = pos - origin;
                output.push(InputEvent::PointerButton {
                    button: pointer_button(button),
                    pressed,
                    x: position.x,
                    y: position.y,
                });
            }
            Event::Scroll(delta) => {
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: WheelUnit::Pixels,
                });
            }
            Event::Zoom(factor) => output.push(InputEvent::Zoom { factor }),
            Event::Touch {
                id,
                phase,
                pos,
                force,
            } => {
                let position = pos - origin;
                output.push(InputEvent::Touch {
                    device: id.device,
                    finger: id.finger,
                    phase: touch_phase(phase),
                    x: position.x,
                    y: position.y,
                    force,
                });
            }
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
            Event::Ime(ime) => output.push(InputEvent::Ime(match ime {
                beui::ImeEvent::Enabled => ImeInput::Enabled,
                beui::ImeEvent::Preedit(text) => ImeInput::Preedit(text),
                beui::ImeEvent::Commit(text) => ImeInput::Commit(text),
                beui::ImeEvent::Disabled => ImeInput::Disabled,
            })),
            Event::Focus(focused) => output.push(InputEvent::Focus(focused)),
            _ => {}
        }
    }
}
