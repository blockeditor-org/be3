use crate::color::Color32;
use crate::font::FontId;
use crate::geometry::{Pos2, Rect, pos2, vec2};
use crate::icons;
use crate::input::{Event, Key, Modifiers};
use crate::painter::Painter;
use crate::styled::Theme;
use crate::styled::theme::{CHIP_RADIUS, FONT_BODY, ICON_SIZE};

const ROW_HEIGHT: f32 = 36.0;
const PADDING: f32 = 6.0;
const GAP: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct KeyId {
    row: usize,
    column: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModifierKey {
    Shift,
    Ctrl,
    Alt,
}

impl ModifierKey {
    fn read(self, modifiers: Modifiers) -> bool {
        match self {
            Self::Shift => modifiers.shift,
            Self::Ctrl => modifiers.ctrl,
            Self::Alt => modifiers.alt,
        }
    }

    fn toggle(self, modifiers: &mut Modifiers) {
        match self {
            Self::Shift => modifiers.shift = !modifiers.shift,
            Self::Ctrl => modifiers.ctrl = !modifiers.ctrl,
            Self::Alt => modifiers.alt = !modifiers.alt,
        }
    }
}

#[derive(Clone, Copy)]
enum Face {
    Char {
        lower: &'static str,
        upper: &'static str,
        key: Option<Key>,
    },
    Named {
        label: &'static str,
        icon: Option<&'static str>,
        key: Key,
        text: Option<&'static str>,
    },
    Modifier {
        label: &'static str,
        modifier: ModifierKey,
    },
}

#[derive(Clone, Copy)]
struct Cap {
    face: Face,
    weight: f32,
}

const fn letter(lower: &'static str, upper: &'static str, key: Key) -> Cap {
    Cap {
        face: Face::Char {
            lower,
            upper,
            key: Some(key),
        },
        weight: 1.0,
    }
}

const fn symbol(lower: &'static str, upper: &'static str) -> Cap {
    Cap {
        face: Face::Char {
            lower,
            upper,
            key: None,
        },
        weight: 1.0,
    }
}

const fn named(label: &'static str, icon: Option<&'static str>, key: Key, weight: f32) -> Cap {
    Cap {
        face: Face::Named {
            label,
            icon,
            key,
            text: None,
        },
        weight,
    }
}

const fn space(weight: f32) -> Cap {
    Cap {
        face: Face::Named {
            label: "Space",
            icon: Some(icons::ICON_SPACE_BAR),
            key: Key::Space,
            text: Some(" "),
        },
        weight,
    }
}

const fn modifier(label: &'static str, modifier: ModifierKey, weight: f32) -> Cap {
    Cap {
        face: Face::Modifier { label, modifier },
        weight,
    }
}

const ROWS: [&[Cap]; 5] = [
    &[
        symbol("1", "!"),
        symbol("2", "@"),
        symbol("3", "#"),
        symbol("4", "$"),
        symbol("5", "%"),
        symbol("6", "^"),
        symbol("7", "&"),
        symbol("8", "*"),
        symbol("9", "("),
        symbol("0", ")"),
        named(
            "Backspace",
            Some(icons::ICON_BACKSPACE),
            Key::Backspace,
            1.5,
        ),
    ],
    &[
        letter("q", "Q", Key::Q),
        letter("w", "W", Key::W),
        letter("e", "E", Key::E),
        letter("r", "R", Key::R),
        letter("t", "T", Key::T),
        letter("y", "Y", Key::Y),
        letter("u", "U", Key::U),
        letter("i", "I", Key::I),
        letter("o", "O", Key::O),
        letter("p", "P", Key::P),
    ],
    &[
        letter("a", "A", Key::A),
        letter("s", "S", Key::S),
        letter("d", "D", Key::D),
        letter("f", "F", Key::F),
        letter("g", "G", Key::G),
        letter("h", "H", Key::H),
        letter("j", "J", Key::J),
        letter("k", "K", Key::K),
        letter("l", "L", Key::L),
        named("Enter", Some(icons::ICON_KEYBOARD_RETURN), Key::Enter, 1.5),
    ],
    &[
        modifier("Shift", ModifierKey::Shift, 1.5),
        letter("z", "Z", Key::Z),
        letter("x", "X", Key::X),
        letter("c", "C", Key::C),
        letter("v", "V", Key::V),
        letter("b", "B", Key::B),
        letter("n", "N", Key::N),
        letter("m", "M", Key::M),
        symbol(",", "<"),
        symbol(".", ">"),
        symbol("-", "_"),
    ],
    &[
        modifier("Ctrl", ModifierKey::Ctrl, 1.25),
        modifier("Alt", ModifierKey::Alt, 1.25),
        named("Tab", Some(icons::ICON_KEYBOARD_TAB), Key::Tab, 1.25),
        space(3.0),
        named("Esc", None, Key::Escape, 1.25),
        named(
            "Left",
            Some(icons::ICON_KEYBOARD_ARROW_LEFT),
            Key::ArrowLeft,
            1.0,
        ),
        named("Up", Some(icons::ICON_KEYBOARD_ARROW_UP), Key::ArrowUp, 1.0),
        named(
            "Down",
            Some(icons::ICON_KEYBOARD_ARROW_DOWN),
            Key::ArrowDown,
            1.0,
        ),
        named(
            "Right",
            Some(icons::ICON_KEYBOARD_ARROW_RIGHT),
            Key::ArrowRight,
            1.0,
        ),
    ],
];

#[derive(Default)]
pub(crate) struct Keyboard {
    pressed: Option<KeyId>,
    modifiers: Modifiers,
    events: Vec<Event>,
}

impl Keyboard {
    pub(crate) fn height() -> f32 {
        ROWS.len() as f32 * ROW_HEIGHT + (ROWS.len() as f32 - 1.0) * GAP + PADDING * 2.0
    }

    pub(crate) fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub(crate) fn hit(rect: Rect, pos: Pos2) -> Option<KeyId> {
        cells(rect)
            .into_iter()
            .find(|(_, bounds, _)| bounds.contains(pos))
            .map(|(id, _, _)| id)
    }

    pub(crate) fn press(&mut self, id: KeyId) {
        let Some(cap) = cap(id) else {
            return;
        };
        self.pressed = Some(id);
        match cap.face {
            Face::Char { lower, upper, key } => {
                let modifiers = self.modifiers;
                let text = if modifiers.shift { upper } else { lower };
                self.type_key(key, Some(text), modifiers);
                self.modifiers = Modifiers::NONE;
            }
            Face::Named { key, text, .. } => {
                let modifiers = self.modifiers;
                self.type_key(Some(key), text, modifiers);
                self.modifiers = Modifiers::NONE;
            }
            Face::Modifier { modifier, .. } => modifier.toggle(&mut self.modifiers),
        }
    }

    pub(crate) fn release(&mut self, id: KeyId) {
        if self.pressed == Some(id) {
            self.pressed = None;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.pressed = None;
        self.modifiers = Modifiers::NONE;
    }

    pub(crate) fn drain(&mut self, out: &mut Vec<Event>) {
        out.append(&mut self.events);
    }

    fn type_key(&mut self, key: Option<Key>, text: Option<&str>, modifiers: Modifiers) {
        if let Some(key) = key {
            self.events.push(Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
            });
        }
        if let Some(text) = text.filter(|_| !modifiers.ctrl && !modifiers.alt) {
            self.events.push(Event::Text(text.to_owned()));
        }
        if let Some(key) = key {
            self.events.push(Event::Key {
                key,
                pressed: false,
                repeat: false,
                modifiers,
            });
        }
    }

    pub(crate) fn paint(&self, painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, Theme::DARK.background);
        for (id, bounds, cap) in cells(rect) {
            let latched = match cap.face {
                Face::Modifier { modifier, .. } => modifier.read(self.modifiers),
                _ => false,
            };
            let held = self.pressed == Some(id);
            let fill = if held || latched {
                Theme::DARK.accent
            } else {
                Theme::DARK.surface_raised
            };
            let color = if held || latched {
                Theme::DARK.on_accent
            } else {
                Theme::DARK.text
            };
            painter.rect_filled(bounds, f32::from(CHIP_RADIUS), fill);
            match cap.face {
                Face::Named {
                    icon: Some(icon), ..
                } => centered(painter, bounds, icon, FontId::icons(ICON_SIZE), color),
                Face::Named { label, .. } | Face::Modifier { label, .. } => {
                    centered(
                        painter,
                        bounds,
                        label,
                        FontId::proportional(FONT_BODY),
                        color,
                    );
                }
                Face::Char { lower, upper, .. } => {
                    let text = if self.modifiers.shift { upper } else { lower };
                    centered(
                        painter,
                        bounds,
                        text,
                        FontId::proportional(FONT_BODY),
                        color,
                    );
                }
            }
        }
    }
}

fn centered(painter: &Painter, rect: Rect, text: &str, font: FontId, color: Color32) {
    let galley = painter.layout(text, font, f32::INFINITY);
    let size = galley.size();
    painter.galley(rect.center() - size * 0.5, galley, color);
}

#[cfg(test)]
impl Keyboard {
    pub(crate) fn locate(rect: Rect, label: &str) -> Option<Pos2> {
        cells(rect)
            .into_iter()
            .find(|(_, _, cap)| face_label(cap) == label)
            .map(|(_, bounds, _)| bounds.center())
    }
}

#[cfg(test)]
fn face_label(cap: &Cap) -> &'static str {
    match cap.face {
        Face::Char { lower, .. } => lower,
        Face::Named { label, .. } | Face::Modifier { label, .. } => label,
    }
}

fn cap(id: KeyId) -> Option<Cap> {
    ROWS.get(id.row).and_then(|row| row.get(id.column)).copied()
}

fn cells(rect: Rect) -> Vec<(KeyId, Rect, Cap)> {
    let inner = rect.shrink(PADDING);
    let widest = ROWS.iter().map(|row| weight(row)).fold(0.0_f32, f32::max);
    if widest <= 0.0 || !inner.is_positive() {
        return Vec::new();
    }
    let slot = inner.width() / widest;
    let mut cells = Vec::new();
    for (row, caps) in ROWS.iter().enumerate() {
        let top = inner.top() + row as f32 * (ROW_HEIGHT + GAP);
        let mut x = inner.left() + (inner.width() - slot * weight(caps)) / 2.0;
        for (column, cap) in caps.iter().enumerate() {
            let width = slot * cap.weight - GAP;
            let bounds = Rect::from_min_size(pos2(x, top), vec2(width.max(0.0), ROW_HEIGHT));
            cells.push((KeyId { row, column }, bounds, *cap));
            x += slot * cap.weight;
        }
    }
    cells
}

fn weight(row: &[Cap]) -> f32 {
    row.iter().map(|cap| cap.weight).sum()
}
