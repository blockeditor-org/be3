use beui_core::context::Context;
use beui_core::geometry::{Pos2, pos2, vec2};
use beui_core::input::{Event, Modifiers, TouchId, TouchPhase};
use block_editor_plugin::{InputEvent, PointerButton, Region, WheelUnit};

const WHEEL_LINE: f32 = 40.0;
const WHEEL_PAGE: f32 = 400.0;

pub(crate) struct Input {
    modifiers: Modifiers,
    pointer: Pos2,
    emulated_touch: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            modifiers: Modifiers::NONE,
            pointer: Pos2::ZERO,
            emulated_touch: false,
        }
    }
}

impl Input {
    pub(crate) fn translate(
        &mut self,
        context: &Context,
        region: &Region,
        event: &InputEvent,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let origin = region.rect.min.to_vec2();
        let ratio = context
            .simulated_pixels_per_point()
            .map_or(1.0, |simulated| region.scale_factor / simulated);
        let at = |x: f32, y: f32| pos2((x + origin.x) * ratio, (y + origin.y) * ratio);
        let emulating = context.touch_emulation();
        if !emulating && self.emulated_touch {
            self.emulated_touch = false;
            events.push(self.touch(TouchPhase::Cancel));
        }
        match event {
            InputEvent::PointerMoved { x, y } => {
                self.pointer = at(*x, *y);
                if !emulating {
                    events.push(Event::PointerMoved(self.pointer));
                } else if self.emulated_touch {
                    events.push(self.touch(TouchPhase::Move));
                }
            }
            InputEvent::PointerLeft if !emulating => events.push(Event::PointerGone),
            InputEvent::PointerLeft => {}
            InputEvent::PointerButton {
                button: PointerButton::Primary,
                pressed,
                x,
                y,
            } if emulating => {
                self.pointer = at(*x, *y);
                if *pressed != self.emulated_touch {
                    self.emulated_touch = *pressed;
                    events.push(self.touch(match *pressed {
                        true => TouchPhase::Start,
                        false => TouchPhase::End,
                    }));
                }
            }
            InputEvent::PointerButton { .. } if emulating => {}
            InputEvent::PointerButton {
                button,
                pressed,
                x,
                y,
            } => {
                if let Some(button) = beui_plugin_input::beui_button(*button) {
                    self.pointer = at(*x, *y);
                    events.push(Event::PointerButton {
                        pos: self.pointer,
                        button,
                        pressed: *pressed,
                        modifiers: self.modifiers,
                    });
                }
            }
            InputEvent::Wheel { x, y, unit } => {
                let scale = match unit {
                    WheelUnit::Pixels => 1.0,
                    WheelUnit::Lines => WHEEL_LINE,
                    WheelUnit::Pages => WHEEL_PAGE,
                };
                events.push(Event::Scroll(vec2(x * scale, y * scale)));
            }
            InputEvent::Touch {
                device,
                finger,
                phase,
                x,
                y,
                force,
            } => events.push(Event::Touch {
                id: TouchId {
                    device: *device,
                    finger: *finger,
                },
                phase: beui_plugin_input::beui_touch_phase(*phase),
                pos: at(*x, *y),
                force: *force,
            }),
            InputEvent::Key {
                key,
                pressed,
                repeat,
            } => {
                if let Some(key) = beui_plugin_input::beui_key(*key) {
                    events.push(Event::Key {
                        key,
                        pressed: *pressed,
                        repeat: *repeat,
                        modifiers: self.modifiers,
                    });
                }
            }
            InputEvent::Text(text) | InputEvent::Paste(text) => {
                events.push(Event::Text(text.clone()));
            }
            InputEvent::Modifiers(modifiers) => {
                self.modifiers = Modifiers {
                    alt: modifiers.alt,
                    ctrl: modifiers.control,
                    shift: modifiers.shift,
                    logo: modifiers.logo,
                };
                events.push(Event::Modifiers(self.modifiers));
            }
            InputEvent::WheelEnded => events.push(Event::ScrollEnded),
            InputEvent::Zoom { factor } => events.push(Event::Zoom(*factor)),
            InputEvent::PointerMotion { x, y } => {
                events.push(Event::PointerMotion(vec2(*x, *y) * ratio));
            }
            InputEvent::Focus(false) => {
                if std::mem::take(&mut self.emulated_touch) {
                    events.push(self.touch(TouchPhase::Cancel));
                }
                events.push(Event::Focus(false));
            }
            InputEvent::Ime(ime) => events.push(Event::Ime(beui_plugin_input::beui_ime(ime))),
            InputEvent::Focus(_) => {}
            InputEvent::Back(phase) => {
                events.push(Event::Back(beui_plugin_input::beui_back(*phase)));
            }
        }
        events
    }

    fn touch(&self, phase: TouchPhase) -> Event {
        Event::Touch {
            id: TouchId {
                device: 0,
                finger: 0,
            },
            phase,
            pos: self.pointer,
            force: None,
        }
    }
}
