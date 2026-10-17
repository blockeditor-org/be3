use beui_core::geometry::{Rect, pos2, vec2};
use beui_core::input::Key;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Modifier {
    Shift,
    Ctrl,
    Alt,
    Meta,
    Fn,
    Superscript,
    Subscript,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Copy,
    Paste,
    PastePlain,
    SelectAll,
    Undo,
    Redo,
    DeleteWordLeft,
    DeleteWordRight,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Layer {
    #[default]
    Text,
    Greek,
    Numpad,
    Emoji,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slider {
    CursorLeft,
    CursorRight,
    SelectWords,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Value {
    Char(char),
    Key(Key),
    Modifier(Modifier),
    CapsLock,
    Action(Action),
    Layer(Layer),
    Compose,
    Slider(Slider),
}

impl Value {
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Modifier(_))
    }

    pub fn repeats(self) -> bool {
        match self {
            Self::Char(_) => true,
            Self::Key(key) => !matches!(key, Key::Escape | Key::Enter | Key::Tab),
            Self::Action(action) => {
                matches!(action, Action::DeleteWordLeft | Action::DeleteWordRight)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Center,
    Nw,
    Ne,
    Sw,
    Se,
    W,
    E,
    N,
    S,
}

impl Slot {
    pub const ALL: [Slot; 9] = [
        Slot::Center,
        Slot::Nw,
        Slot::Ne,
        Slot::Sw,
        Slot::Se,
        Slot::W,
        Slot::E,
        Slot::N,
        Slot::S,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Normal,
    Action,
    Space,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct KeyDef {
    pub values: [Option<Value>; 9],
    pub width: f32,
    pub shift: f32,
    pub role: Role,
}

impl KeyDef {
    pub const fn new(center: Value) -> Self {
        let mut values = [None; 9];
        values[0] = Some(center);
        Self {
            values,
            width: 1.0,
            shift: 0.0,
            role: Role::Normal,
        }
    }

    pub const fn at(mut self, slot: Slot, value: Value) -> Self {
        self.values[slot as usize] = Some(value);
        self
    }

    pub const fn nw(self, value: Value) -> Self {
        self.at(Slot::Nw, value)
    }

    pub const fn ne(self, value: Value) -> Self {
        self.at(Slot::Ne, value)
    }

    pub const fn sw(self, value: Value) -> Self {
        self.at(Slot::Sw, value)
    }

    pub const fn se(self, value: Value) -> Self {
        self.at(Slot::Se, value)
    }

    pub const fn arrows(self) -> Self {
        self.at(Slot::W, Value::Key(Key::ArrowLeft))
            .at(Slot::E, Value::Key(Key::ArrowRight))
            .at(Slot::N, Value::Key(Key::ArrowUp))
            .at(Slot::S, Value::Key(Key::ArrowDown))
    }

    pub const fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub const fn shift(mut self, shift: f32) -> Self {
        self.shift = shift;
        self
    }

    pub const fn action(mut self) -> Self {
        self.role = Role::Action;
        self
    }

    pub const fn space(mut self) -> Self {
        self.role = Role::Space;
        self
    }

    pub fn value(&self, slot: Slot) -> Option<Value> {
        self.values[slot.index()]
    }

    pub fn span(&self) -> f32 {
        self.shift + self.width
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Row {
    pub keys: &'static [KeyDef],
    pub height: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    pub rows: &'static [Row],
}

impl Layout {
    pub fn width(&self) -> f32 {
        self.rows
            .iter()
            .map(|row| row.keys.iter().map(KeyDef::span).sum::<f32>())
            .fold(0.0, f32::max)
    }

    pub fn height(&self) -> f32 {
        self.rows.iter().map(|row| row.height).sum()
    }

    pub fn key(&self, at: KeyAt) -> Option<&KeyDef> {
        self.rows.get(at.row)?.keys.get(at.column)
    }

    pub fn key_rect(&self, rect: Rect, at: KeyAt) -> Option<Rect> {
        let row = self.rows.get(at.row)?;
        let def = row.keys.get(at.column)?;
        let (width, height) = (self.width(), self.height());
        let top: f32 = self.rows[..at.row].iter().map(|row| row.height).sum();
        let left: f32 = row.keys[..at.column].iter().map(KeyDef::span).sum::<f32>() + def.shift;
        Some(Rect::from_min_size(
            pos2(
                rect.left() + left / width * rect.width(),
                rect.top() + top / height * rect.height(),
            ),
            vec2(
                def.width / width * rect.width(),
                row.height / height * rect.height(),
            ),
        ))
    }

    pub fn find(&self, value: Value) -> Option<KeyAt> {
        self.rows.iter().enumerate().find_map(|(row, line)| {
            line.keys
                .iter()
                .position(|def| def.value(Slot::Center) == Some(value))
                .map(|column| KeyAt { row, column })
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyAt {
    pub row: usize,
    pub column: usize,
}

const fn c(character: char) -> Value {
    Value::Char(character)
}

const fn k(key: Key) -> Value {
    Value::Key(key)
}

const fn key(character: char) -> KeyDef {
    KeyDef::new(c(character))
}

const ESC: Value = k(Key::Escape);
const TAB: Value = k(Key::Tab);
const SHIFT: Value = Value::Modifier(Modifier::Shift);
const CTRL: Value = Value::Modifier(Modifier::Ctrl);
const ALT: Value = Value::Modifier(Modifier::Alt);
const META: Value = Value::Modifier(Modifier::Meta);
const FN: Value = Value::Modifier(Modifier::Fn);
const SUPERSCRIPT: Value = Value::Modifier(Modifier::Superscript);
const SUBSCRIPT: Value = Value::Modifier(Modifier::Subscript);
const COPY: Value = Value::Action(Action::Copy);
const PASTE: Value = Value::Action(Action::Paste);
const PASTE_PLAIN: Value = Value::Action(Action::PastePlain);
const SELECT_ALL: Value = Value::Action(Action::SelectAll);
const UNDO: Value = Value::Action(Action::Undo);
const REDO: Value = Value::Action(Action::Redo);

const BACKSPACE: KeyDef = KeyDef::new(k(Key::Backspace))
    .ne(k(Key::Delete))
    .sw(Value::Action(Action::DeleteWordLeft))
    .se(Value::Action(Action::DeleteWordRight))
    .at(Slot::W, Value::Slider(Slider::SelectWords))
    .width(1.5)
    .action();

const SPACE: KeyDef = KeyDef::new(c(' '))
    .at(Slot::W, Value::Slider(Slider::CursorLeft))
    .at(Slot::E, Value::Slider(Slider::CursorRight))
    .space();

const COMPOSE: KeyDef = KeyDef::new(Value::Compose)
    .nw(k(Key::Home))
    .arrows()
    .action();

const ENTER: KeyDef = KeyDef::new(k(Key::Enter)).action();

pub const QWERTY: Layout = Layout {
    rows: &[
        Row {
            keys: &[
                key('q').ne(c('1')).se(ESC),
                key('w').nw(c('~')).ne(c('2')).sw(c('@')),
                key('e').nw(c('!')).ne(c('3')).sw(c('#')).se(c('€')),
                key('r').ne(c('4')).sw(c('$')),
                key('t').ne(c('5')).sw(c('%')),
                key('y').ne(c('6')).sw(c('^')).se(REDO),
                key('u').ne(c('7')).sw(c('&')),
                key('i').ne(c('8')).sw(c('*')),
                key('o').ne(c('9')).sw(c('(')).se(c(')')),
                key('p').ne(c('0')),
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                key('a')
                    .nw(TAB)
                    .ne(c('`'))
                    .sw(SELECT_ALL)
                    .se(c('£'))
                    .shift(0.5),
                key('s').ne(c('§')).sw(c('ß')).se(SUPERSCRIPT),
                key('d').se(SUBSCRIPT),
                key('f').se(PASTE_PLAIN),
                key('g').ne(c('-')).sw(c('_')),
                key('h').ne(c('=')).sw(c('+')),
                key('j').sw(c('{')).se(c('}')),
                key('k').sw(c('[')).se(c(']')),
                key('l').ne(c('|')).sw(c('\\')),
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(SHIFT).ne(Value::CapsLock).width(1.5).action(),
                key('z').se(UNDO),
                key('x').nw(c('†')).sw(REDO),
                key('c').ne(c('<')).sw(c('.')).se(COPY),
                key('v').ne(c('>')).sw(c(',')).se(PASTE),
                key('b').ne(c('?')).sw(c('/')),
                key('n').ne(c(':')).sw(c(';')),
                key('m').ne(c('"')).sw(c('\'')),
                BACKSPACE,
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(CTRL)
                    .nw(Value::Layer(Layer::Greek))
                    .ne(META)
                    .se(Value::Layer(Layer::Numpad))
                    .width(1.7)
                    .action(),
                KeyDef::new(FN)
                    .nw(ALT)
                    .sw(Value::Layer(Layer::Emoji))
                    .width(1.1)
                    .action(),
                SPACE.width(4.4),
                COMPOSE.width(1.1),
                ENTER.width(1.7),
            ],
            height: 0.95,
        },
    ],
};

pub const GREEK: Layout = Layout {
    rows: &[
        Row {
            keys: &[
                key('θ').ne(c('^')),
                key('ω').nw(c('⌊')).ne(c('∨')).sw(c('↔')).se(c('⊂')),
                key('ε').nw(c('⌋')).ne(c('∧')).sw(c('↑')).se(c('⊃')),
                key('ρ').nw(c('⌈')).ne(c('⊥')).sw(c('↕')).se(c('∀')),
                key('τ').nw(c('⌉')).ne(c('∡')).se(c('⊷')),
                key('ψ').nw(c('∥')).ne(c('⊢')).sw(c('≪')).se(c('7')),
                key('υ').nw(c('⌀')).ne(c('⟨')).sw(c('≡')).se(c('8')),
                key('ι').nw(c('∞')).ne(c('⟩')).sw(c('≫')).se(c('9')),
                key('ο').nw(c('∝')).sw(c('□')),
                key('π').nw(c('∅')).sw(c('∘')),
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                key('α').se(c('ℂ')),
                key('σ').sw(c('←')).se(c('∂')),
                key('δ').sw(c('↓')).se(c('∫')),
                key('φ').sw(c('→')).se(c('∃')),
                key('γ').sw(c('∋')).se(c('∈')),
                key('η').nw(c('⊕')).se(c('4')),
                key('ξ').nw(c('⊖')).sw(c('ℕ')).se(c('5')),
                key('κ').nw(c('⊙')).sw(c('ℝ')).se(c('6')),
                key('λ').nw(c('⊗')),
                key('ϡ').nw(c('⌀')),
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(SHIFT).ne(Value::CapsLock).width(1.5).action(),
                key('ζ').sw(c('⇌')).se(c('√')),
                key('χ').sw(c('\u{20d7}')).se(c('∪')),
                key('ϛ').se(c('∩')),
                key('ϟ').sw(c('∇')).se(c('ℵ')),
                key('β').nw(c('≤')).sw(c('ℤ')).se(c('1')),
                key('ν').nw(c('≠')).sw(c('ℚ')).se(c('2')),
                key('μ').nw(c('≥')).sw(c('×')).se(c('3')),
                BACKSPACE,
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(Value::Layer(Layer::Text))
                    .ne(ESC)
                    .width(1.2)
                    .action(),
                KeyDef::new(Value::Layer(Layer::Numpad))
                    .ne(TAB)
                    .width(1.2)
                    .action(),
                KeyDef::new(FN).width(1.2).action(),
                SPACE.nw(SUPERSCRIPT).sw(SUBSCRIPT).width(2.8),
                KeyDef::new(c('0')).ne(c('.')).width(1.2).action(),
                COMPOSE.width(1.2),
                ENTER.nw(c('=')).width(1.2),
            ],
            height: 0.95,
        },
    ],
};

pub const NUMPAD: Layout = Layout {
    rows: &[
        Row {
            keys: &[
                KeyDef::new(ESC).ne(c('~')).se(c('!')).width(0.75).action(),
                key('(').ne(c('[')).se(c('{')).width(0.75).action(),
                key('7').nw(c('<')).ne(c('>')),
                key('8').ne(c('∞')),
                key('9').ne(c('π')),
                key('*')
                    .nw(c('√'))
                    .ne(c('×'))
                    .sw(c('#'))
                    .width(0.75)
                    .action(),
                key('/').nw(c('%')).sw(c('÷')).width(0.75).action(),
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(TAB).ne(c('|')).se(c('\\')).width(0.75).action(),
                key(')').ne(c(']')).se(c('}')).width(0.75).action(),
                key('4'),
                key('5').arrows(),
                key('6'),
                key('+').nw(c('Σ')).ne(c('$')).width(0.75).action(),
                key('-').nw(c('^')).width(0.75).action(),
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(Value::Layer(Layer::Greek)).width(0.75).action(),
                KeyDef::new(SHIFT).ne(FN).se(ALT).width(0.75).action(),
                key('1').nw(SUPERSCRIPT).sw(SUBSCRIPT),
                key('2'),
                key('3'),
                BACKSPACE,
            ],
            height: 1.0,
        },
        Row {
            keys: &[
                KeyDef::new(Value::Layer(Layer::Text))
                    .ne(CTRL)
                    .width(1.5)
                    .action(),
                key('0').width(1.5),
                key('.')
                    .nw(c(':'))
                    .ne(c(','))
                    .sw(c(';'))
                    .width(0.75)
                    .action(),
                SPACE
                    .nw(c('"'))
                    .ne(c('\''))
                    .sw(Value::Compose)
                    .se(c('_'))
                    .width(0.75),
                ENTER.nw(c('±')).sw(c('=')).width(1.5),
            ],
            height: 0.95,
        },
    ],
};

pub fn layout(layer: Layer) -> &'static Layout {
    match layer {
        Layer::Text | Layer::Emoji => &QWERTY,
        Layer::Greek => &GREEK,
        Layer::Numpad => &NUMPAD,
    }
}
