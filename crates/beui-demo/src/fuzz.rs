use std::time::Duration;

use beui::datetime::Date;
use beui::{
    BackEdge, BackGesture, Context, Event, FontLibrary, FreetypeFonts, Key, KeyPress, Modifiers,
    PointerButton, Pos2, RawInput, Rect, TouchId, TouchPhase, Vec2, pos2,
};

use crate::DemoApp;

const FRAME: Duration = Duration::from_micros(16_667);
const SIZES: [Vec2; 4] = [
    Vec2::new(1100.0, 800.0),
    Vec2::new(390.0, 800.0),
    Vec2::new(640.0, 360.0),
    Vec2::new(120.0, 90.0),
];
const KEYS: [Key; 32] = [
    Key::Tab,
    Key::Enter,
    Key::Escape,
    Key::Space,
    Key::Backspace,
    Key::Delete,
    Key::ArrowDown,
    Key::ArrowUp,
    Key::ArrowLeft,
    Key::ArrowRight,
    Key::Home,
    Key::End,
    Key::PageDown,
    Key::PageUp,
    Key::A,
    Key::C,
    Key::V,
    Key::X,
    Key::Z,
    Key::Y,
    Key::Q,
    Key::K,
    Key::P,
    Key::F,
    Key::Zero,
    Key::One,
    Key::Plus,
    Key::Minus,
    Key::BrowserBack,
    Key::F11,
    Key::Alt,
    Key::Shift,
];
const TEXT: [&str; 10] = [
    "a", "Z", "7", " ", "\n", "é", "日本", "👍🏽", "\u{301}", "-.,",
];
const BUTTONS: [PointerButton; 3] = [
    PointerButton::Primary,
    PointerButton::Secondary,
    PointerButton::Middle,
];

pub fn input(data: &[u8]) {
    let mut input = Input(data);
    let mut demo = DemoApp::on(Date::new(2026, 1, 15)).document;
    let context = Context::new(FreetypeFonts::measuring(FontLibrary::bundled()));
    context.stop_clock();
    let mut size = SIZES[0];
    let mut pointer = Pos2::ZERO;
    let mut modifiers = Modifiers::NONE;
    let mut targets: Vec<Rect> = Vec::new();
    let mut events = Vec::new();
    let mut frame = |events: &mut Vec<Event>, size: Vec2, targets: &mut Vec<Rect>| {
        context.advance_clock(FRAME);
        let raw = RawInput {
            events: std::mem::take(events),
        };
        let output = context.run(raw, |context| {
            demo.show(context, Rect::from_min_size(Pos2::ZERO, size));
        });
        let mut named: Vec<_> = output.test_ids().iter().collect();
        named.sort_by(|a, b| a.0.cmp(b.0));
        *targets = named.into_iter().map(|(_, rect)| *rect).collect();
    };
    frame(&mut events, size, &mut targets);
    while !input.is_empty() {
        match input.byte() % 16 {
            0 | 1 => {
                pointer = input.pos(size);
                events.push(Event::PointerMoved(pointer));
            }
            2 => {
                let button = BUTTONS[input.below(BUTTONS.len())];
                let pressed = input.byte() % 2 == 0;
                events.push(Event::PointerButton {
                    pos: pointer,
                    button,
                    pressed,
                    modifiers,
                });
            }
            3..=5 => {
                if targets.is_empty() {
                    continue;
                }
                let target = targets[input.below(targets.len())];
                pointer = target.center();
                let button = match input.byte() % 8 {
                    0 => PointerButton::Secondary,
                    _ => PointerButton::Primary,
                };
                events.push(Event::PointerMoved(pointer));
                frame(&mut events, size, &mut targets);
                for pressed in [true, false] {
                    events.push(Event::PointerButton {
                        pos: pointer,
                        button,
                        pressed,
                        modifiers,
                    });
                    frame(&mut events, size, &mut targets);
                }
            }
            6 => {
                let to = input.pos(size);
                events.push(Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                });
                frame(&mut events, size, &mut targets);
                for step in 1..=4 {
                    events.push(Event::PointerMoved(
                        pointer + (to - pointer) * (step as f32 / 4.0),
                    ));
                    frame(&mut events, size, &mut targets);
                }
                pointer = to;
                events.push(Event::PointerButton {
                    pos: pointer,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers,
                });
            }
            7 => {
                let delta = Vec2::new(input.signed() * 4.0, input.signed() * 4.0);
                events.push(Event::Scroll(delta));
                if input.byte() % 2 == 0 {
                    events.push(Event::ScrollEnded);
                }
            }
            8 | 9 => {
                let key = KEYS[input.below(KEYS.len())];
                let held = input.modifiers();
                let pressed = input.byte() % 4 != 0;
                events.push(Event::key(KeyPress {
                    key,
                    pressed,
                    repeat: false,
                    modifiers: held,
                }));
                if pressed && input.byte() % 2 == 0 {
                    events.push(Event::key(KeyPress::up(key, held)));
                }
            }
            10 => events.push(Event::Text(TEXT[input.below(TEXT.len())].to_owned())),
            11 => {
                modifiers = input.modifiers();
                events.push(Event::Modifiers(modifiers));
            }
            12 => {
                let phase = match input.byte() % 4 {
                    0 => TouchPhase::Start,
                    1 => TouchPhase::Move,
                    2 => TouchPhase::End,
                    _ => TouchPhase::Cancel,
                };
                events.push(Event::Touch {
                    id: TouchId {
                        device: 0,
                        finger: input.below(2) as u64,
                    },
                    phase,
                    pos: input.pos(size),
                    force: None,
                });
            }
            13 => events.push(match input.byte() % 8 {
                0 => Event::PointerGone,
                1 => Event::Focus(input.byte() % 2 == 0),
                2 => Event::Zoom(1.0 + input.signed() / 256.0),
                3 => Event::Back(BackGesture::Started {
                    edge: BackEdge::default(),
                }),
                4 => Event::Back(BackGesture::Progressed(f32::from(input.byte()) / 255.0)),
                5 => Event::Back(BackGesture::Invoked),
                6 => Event::Back(BackGesture::Cancelled),
                _ => Event::PointerMotion(Vec2::new(input.signed(), input.signed())),
            }),
            14 => {
                let by = match input.byte() % 4 {
                    0 => Duration::from_secs(60),
                    _ => FRAME * u32::from(input.byte()),
                };
                context.advance_clock(by);
                frame(&mut events, size, &mut targets);
            }
            _ => {
                if input.byte() % 4 == 0 {
                    size = SIZES[input.below(SIZES.len())];
                }
                frame(&mut events, size, &mut targets);
            }
        }
    }
    frame(&mut events, size, &mut targets);
}

struct Input<'a>(&'a [u8]);

impl Input<'_> {
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn byte(&mut self) -> u8 {
        let Some((&first, rest)) = self.0.split_first() else {
            return 0;
        };
        self.0 = rest;
        first
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::from(self.byte()) % bound
    }

    fn signed(&mut self) -> f32 {
        f32::from(self.byte() as i8)
    }

    fn pos(&mut self, size: Vec2) -> Pos2 {
        let x = f32::from(self.byte()) / 255.0;
        let y = f32::from(self.byte()) / 255.0;
        pos2(x * size.x, y * size.y)
    }

    fn modifiers(&mut self) -> Modifiers {
        let bits = self.byte();
        Modifiers {
            alt: bits & 1 != 0,
            ctrl: bits & 2 != 0,
            shift: bits & 4 != 0,
            logo: bits & 8 != 0,
        }
    }
}
