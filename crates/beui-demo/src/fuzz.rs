use std::time::Duration;

use beui::datetime::Date;
use beui::reactive::{live_values, pin_wall_clock};
use beui::{
    BackEdge, BackGesture, Context, Event, FontLibrary, FreetypeFonts, Key, KeyPress, Modifiers,
    PointerButton, Pos2, RawInput, Rect, Shape, TouchId, TouchPhase, Vec2, pos2,
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

const WALL_CLOCK: Duration = Duration::from_secs(1_768_478_400);

pub fn input(data: &[u8]) {
    beui::verify_paint(true);
    beui::verify_layout(true);
    let first = run(data);
    let live = live_values();
    let second = run(data);
    assert_eq!(
        live_values(),
        live,
        "a run left reactive values alive after its document was dropped"
    );
    compare(&first, &second);
}

struct Traced {
    shapes: Vec<Shape>,
    damage: Vec<Rect>,
    repaint: bool,
    repaint_after: Duration,
    test_ids: Vec<(String, Rect, bool)>,
    copied_text: Option<String>,
}

fn compare(first: &[Traced], second: &[Traced]) {
    assert_eq!(
        first.len(),
        second.len(),
        "the same input drew a different number of frames"
    );
    for (index, (first, second)) in first.iter().zip(second).enumerate() {
        let differs = |what: &str| panic!("frame {index} of the same input differs in {what}");
        if first.shapes.len() != second.shapes.len() {
            differs("how many shapes it paints");
        }
        if let Some(shape) = first
            .shapes
            .iter()
            .zip(&second.shapes)
            .position(|(first, second)| !same_shape(first, second))
        {
            differs(&format!("shape {shape}"));
        }
        if first.damage != second.damage {
            differs("its damage");
        }
        if (first.repaint, first.repaint_after) != (second.repaint, second.repaint_after) {
            differs("when it asks to be painted again");
        }
        if first.test_ids != second.test_ids {
            differs("the test ids it shows");
        }
        if first.copied_text != second.copied_text {
            differs("the text it copies");
        }
    }
}

fn same_shape(first: &Shape, second: &Shape) -> bool {
    match (first, second) {
        (
            Shape::Drawing { rect, clip, .. },
            Shape::Drawing {
                rect: other_rect,
                clip: other_clip,
                ..
            },
        ) => (rect, clip) == (other_rect, other_clip),
        (
            Shape::Image {
                rect,
                source,
                tint,
                corner_radius,
                smooth,
                rotation,
                clip,
                ..
            },
            Shape::Image {
                rect: other_rect,
                source: other_source,
                tint: other_tint,
                corner_radius: other_corner_radius,
                smooth: other_smooth,
                rotation: other_rotation,
                clip: other_clip,
                ..
            },
        ) => {
            (rect, source, tint, corner_radius, smooth, rotation, clip)
                == (
                    other_rect,
                    other_source,
                    other_tint,
                    other_corner_radius,
                    other_smooth,
                    other_rotation,
                    other_clip,
                )
        }
        _ => first.same_look(second),
    }
}

fn run(data: &[u8]) -> Vec<Traced> {
    let mut input = Input(data);
    let mut traced = Vec::new();
    let mut demo = DemoApp::on(Date::new(2026, 1, 15)).document;
    let context = Context::new(FreetypeFonts::new(FontLibrary::bundled()));
    context.stop_clock();
    pin_wall_clock(Some((context.now(), WALL_CLOCK)));
    context.keep_accessibility(true);
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
        let mut test_ids: Vec<_> = output
            .test_ids()
            .iter()
            .flat_map(|(name, shown)| {
                shown
                    .iter()
                    .map(move |at| (name.to_owned(), at.rect, at.visible))
            })
            .collect();
        test_ids.sort_by(|a, b| a.0.cmp(&b.0));
        *targets = test_ids
            .iter()
            .filter(|(_, _, visible)| *visible)
            .map(|(_, rect, _)| *rect)
            .collect();
        traced.push(Traced {
            shapes: output.shapes(),
            damage: output.damage.rects().to_vec(),
            repaint: output.repaint,
            repaint_after: output.repaint_after,
            test_ids,
            copied_text: output.copied_text.clone(),
        });
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
    pin_wall_clock(None);
    traced
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
