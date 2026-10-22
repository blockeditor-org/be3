use beui::Event;
use beui::{Key, Modifiers};
use block_plugin_api::{InputEvent, WheelUnit};

#[derive(Default)]
pub(crate) struct Input {
    held: Modifiers,
    time_micros: u64,
}

impl Input {
    pub(crate) fn normalize(&mut self, events: Vec<Event>, time_micros: u64) -> Vec<InputEvent> {
        self.time_micros = time_micros;
        let mut output = Vec::new();
        for event in events {
            self.event(event, &mut output);
        }
        output
    }

    fn modifiers(&mut self, modifiers: Modifiers, output: &mut Vec<InputEvent>) {
        if self.held != modifiers {
            self.held = modifiers;
            output.push(InputEvent::Modifiers(modifiers));
        }
    }

    fn event(&mut self, event: Event, output: &mut Vec<InputEvent>) {
        let time_micros = self.time_micros;
        match event {
            Event::PointerMoved(position) => output.push(InputEvent::PointerMoved {
                x: position.x,
                y: position.y,
                time_micros,
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
                    button,
                    pressed,
                    x: pos.x,
                    y: pos.y,
                    time_micros,
                });
            }
            Event::Scroll(delta) => {
                output.push(InputEvent::Wheel {
                    x: delta.x,
                    y: delta.y,
                    unit: WheelUnit::Pixels,
                });
            }
            Event::ScrollStarted => output.push(InputEvent::WheelStarted),
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
                phase,
                x: pos.x,
                y: pos.y,
                force,
                time_micros,
            }),
            Event::Pen {
                phase,
                pos,
                pressure,
                pen,
            } => output.push(InputEvent::Pen {
                phase,
                x: pos.x,
                y: pos.y,
                pressure,
                tilt_x: pen.tilt.x,
                tilt_y: pen.tilt.y,
                eraser: pen.eraser,
                time_micros,
            }),
            Event::Predicted(pos) => output.push(InputEvent::Predicted { x: pos.x, y: pos.y }),
            Event::Key { press, text, .. } => {
                self.modifiers(press.modifiers, output);
                match press.key {
                    Key::Unidentified => output.extend(text.map(InputEvent::Text)),
                    key => output.push(InputEvent::Key {
                        key,
                        pressed: press.pressed,
                        repeat: press.repeat,
                        text,
                    }),
                }
            }
            Event::Modifiers(modifiers) => self.modifiers(modifiers, output),
            Event::Text(text) => output.push(InputEvent::Text(text)),
            Event::Paste(pasted) => output.push(InputEvent::Paste(pasted)),
            Event::Ime(ime) => output.push(InputEvent::Ime(ime)),
            Event::Focus(focused) => output.push(InputEvent::Focus(focused)),
            Event::Back(gesture) => output.push(InputEvent::Back(gesture)),
            Event::InterceptedKey(press) => {
                self.modifiers(press.modifiers, output);
                if press.key != Key::Unidentified {
                    output.push(InputEvent::InterceptedKey {
                        key: press.key,
                        pressed: press.pressed,
                        repeat: press.repeat,
                    });
                }
            }
            Event::InterceptedTap(key) if key != Key::Unidentified => {
                output.push(InputEvent::InterceptedTap { key });
            }
            _ => {}
        }
    }
}
