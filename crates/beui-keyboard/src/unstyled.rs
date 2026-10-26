pub mod compose;
pub mod engine;
pub mod layout;
pub mod tables;

use std::cell::RefCell;
use std::time::Duration;

use beui_core::geometry::vec2;
use beui_core::input::{PointerButton, PointerPhase};
use beui_core::interact::forward::ForwardedInput;
use beui_core::node::NodeId;
use beui_core::timer;
use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::*;
use reactive::{Stored, store};

use beui_view::types::{Direction, ItemSize};
pub use compose::{Preview, preview};
pub use engine::{Engine, Latch, Mods, modify};
pub use layout::{Action, KeyAt, KeyDef, Layer, Layout, Modifier, Role, Slider, Slot, Value};

#[derive(Clone, Copy)]
pub struct KeyboardHandle {
    engine: Stored<RefCell<Engine>>,
    revision: Signal<u64>,
}

impl KeyboardHandle {
    pub fn layer(&self) -> Memo<Layer> {
        let handle = *self;
        create_memo(move || handle.read(Engine::layer))
    }

    pub fn mods(&self) -> Memo<Mods> {
        let handle = *self;
        create_memo(move || handle.read(Engine::mods))
    }

    pub fn composing(&self) -> Memo<bool> {
        let handle = *self;
        create_memo(move || handle.read(Engine::composing))
    }

    pub fn sequence(&self) -> Memo<Option<String>> {
        let handle = *self;
        create_memo(move || handle.read(|engine| engine.sequence().map(str::to_owned)))
    }

    pub fn set_layer(&self, layer: Layer) {
        self.change(|engine| engine.set_layer(layer));
    }

    pub fn type_text(&self, text: &str) {
        self.change(|engine| engine.type_text(text));
    }

    pub fn tap(&self, value: Value) {
        let now = timer::now();
        self.change(|engine| engine.tap(value, now));
    }

    pub fn close(&self) {
        self.change(Engine::cancel_all);
        close_keyboard();
    }

    fn read<R>(&self, read: impl FnOnce(&Engine) -> R) -> R {
        self.revision.get();
        self.engine.with(|engine| read(&engine.borrow()))
    }

    fn change<R>(&self, change: impl FnOnce(&mut Engine) -> R) -> R {
        let (result, events, paste) = self.engine.with(|engine| {
            let mut engine = engine.borrow_mut();
            let result = change(&mut engine);
            (result, engine.take_events(), engine.take_paste())
        });
        if !events.is_empty() {
            type_events(events);
        }
        if paste {
            request_typed_paste();
        }
        self.revision.update(|revision| *revision += 1);
        result
    }
}

#[derive(Clone, Copy)]
pub struct KeyCapHandle {
    pub at: KeyAt,
    pub def: &'static KeyDef,
    pub values: Memo<[Option<Value>; 9]>,
    pub previews: Memo<[Preview; 9]>,
    pub pressed: Memo<Option<Option<Value>>>,
    pub latch: Memo<Latch>,
    pub active: Memo<bool>,
}

#[component]
pub fn SoftwareKeyboard(cap: RenderFn<KeyCapHandle>, emoji: RenderFn<KeyboardHandle>) -> NodeId {
    let engine = store(RefCell::new(Engine::default()));
    let revision = create_signal(0u64);
    let keyboard = KeyboardHandle { engine, revision };
    let layer = keyboard.layer();
    let repeat = create_timer(move || {
        let now = timer::now();
        let next = keyboard.change(|engine| engine.tick(now));
        next.map(|next| {
            next.saturating_duration_since(now)
                .max(Duration::from_millis(1))
        })
    });
    let forward = move |input: ForwardedInput| {
        let now = timer::now();
        let rect = input.rect;
        keyboard.change(|engine| {
            let layout = engine.layout();
            engine.set_key_size(vec2(
                rect.width() / layout.width().max(1.0),
                rect.height() / layout.height().max(1.0),
            ));
            for event in input.pointer_events() {
                match event.phase {
                    PointerPhase::Down(PointerButton::Primary) => {
                        if let Some(at) = Engine::hit(engine.layout(), rect, event.pos) {
                            engine.press(event.pointer, at, event.pos, now);
                        }
                    }
                    PointerPhase::Move => engine.moved(event.pointer, event.pos, now),
                    PointerPhase::Up(PointerButton::Primary) => engine.released(event.pointer, now),
                    PointerPhase::Cancel | PointerPhase::Gone => engine.cancelled(event.pointer),
                    _ => {}
                }
            }
        });
        let next = keyboard.engine.with(|engine| engine.borrow_mut().tick(now));
        match next {
            Some(next) => repeat.restart(next.saturating_duration_since(now)),
            None => repeat.stop(),
        }
    };
    view! {
        <List spacing=0.0>
            <Dynamic value={layer}>|layer: Layer| {
                match layer {
                    Layer::Emoji => view! {
                        {emoji.call(keyboard)} @sizing=ItemSize::Percent(100.0)
                    },
                    _ => view! {
                        <Interactive
                            @sizing=ItemSize::Percent(100.0)
                            press_focus=false
                            forwards_wheel=false
                            on_forward={forward}
                        >
                            <KeyRows layout={layout::layout(layer)} keyboard cap />
                        </Interactive>
                    },
                }
            }</Dynamic>
        </List>
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Cell {
    Gap(usize),
    Key(usize),
    Fill,
}

#[component]
fn KeyRows(
    layout: &'static Layout,
    keyboard: KeyboardHandle,
    cap: RenderFn<KeyCapHandle>,
) -> NodeId {
    let rows: Vec<usize> = (0..layout.rows.len()).collect();
    view! {
        <List spacing=0.0>
            <ForEach keys={rows}>|row: usize|
                <KeyRow
                    @sizing=ItemSize::Percent(layout.rows[row].height)
                    layout
                    row
                    keyboard
                    cap
                />
            </ForEach>
        </List>
    }
}

#[component]
fn KeyRow(
    layout: &'static Layout,
    row: usize,
    keyboard: KeyboardHandle,
    cap: RenderFn<KeyCapHandle>,
) -> NodeId {
    let keys = layout.rows[row].keys;
    let mut cells = Vec::new();
    for (column, def) in keys.iter().enumerate() {
        if def.shift > 0.0 {
            cells.push(Cell::Gap(column));
        }
        cells.push(Cell::Key(column));
    }
    let span: f32 = keys.iter().map(KeyDef::span).sum();
    let fill = layout.width() - span;
    if fill > 0.0 {
        cells.push(Cell::Fill);
    }
    let mods = keyboard.mods();
    let composing = keyboard.composing();
    let sequence = keyboard.sequence();
    let layer = keyboard.layer();
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <ForEach keys={cells}>|cell: Cell| {
                match cell {
                    Cell::Gap(column) => view! {
                        <Frame @sizing=ItemSize::Percent(keys[column].shift) />
                    },
                    Cell::Fill => view! {
                        <Frame @sizing=ItemSize::Percent(fill) />
                    },
                    Cell::Key(column) => {
                        let def = &keys[column];
                        let at = KeyAt { row, column };
                        let values = create_memo(move || {
                            let mods = mods.get();
                            def.values.map(|value| value.map(|value| modify(value, mods)))
                        });
                        let previews = create_memo(move || {
                            let values = values.get();
                            sequence.with(|sequence| {
                                values.map(|value| match (sequence, value) {
                                    (Some(sequence), Some(value)) => preview(sequence, value),
                                    _ => Preview::Same,
                                })
                            })
                        });
                        let pressed = create_memo(move || keyboard.read(|engine| engine.pressed(at)));
                        let latch = create_memo(move || match def.value(Slot::Center) {
                            Some(Value::Modifier(modifier)) => keyboard.read(|engine| {
                                match (engine.latch(modifier), engine.held(modifier)) {
                                    (Latch::Off, true) => Latch::Latched,
                                    (latch, _) => latch,
                                }
                            }),
                            _ => Latch::Off,
                        });
                        let active = create_memo(move || match def.value(Slot::Center) {
                            Some(Value::Compose) => composing.get(),
                            Some(Value::Layer(target)) => layer.get() == target,
                            _ => false,
                        });
                        let handle = KeyCapHandle {
                            at,
                            def,
                            values,
                            previews,
                            pressed,
                            latch,
                            active,
                        };
                        view! {
                            <KeyCell @sizing=ItemSize::Percent(def.width) handle cap />
                        }
                    }
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn KeyCell(handle: KeyCapHandle, cap: RenderFn<KeyCapHandle>) -> NodeId {
    let label = handle
        .def
        .value(Slot::Center)
        .map(description)
        .unwrap_or_default();
    component_accessibility(create_memo(move || {
        let mut node = accesskit::Node::new(accesskit::Role::Button);
        node.set_label(label.clone());
        node
    }));
    view! {
        <Frame>{cap.call(handle)}</Frame>
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Face {
    pub glyph: Option<beui_core::icons::Glyph>,
    pub text: String,
    pub after: String,
}

impl Face {
    fn text(text: impl Into<String>) -> Self {
        Self {
            glyph: None,
            text: text.into(),
            after: String::new(),
        }
    }

    fn glyph(glyph: beui_core::icons::Glyph) -> Self {
        Self {
            glyph: Some(glyph),
            text: String::new(),
            after: String::new(),
        }
    }

    fn around(text: &str, glyph: beui_core::icons::Glyph, after: &str) -> Self {
        Self {
            glyph: Some(glyph),
            text: text.to_owned(),
            after: after.to_owned(),
        }
    }
}

pub fn face(value: Value) -> Face {
    use beui_core::icons::*;
    use beui_core::input::Key;
    match value {
        Value::Char(' ') => Face::glyph(ICON_SPACE),
        Value::Char('\t') => Face::text("\\t"),
        Value::Char('\u{a0}') => Face::text("nbsp"),
        Value::Char(character) if is_combining(character) => {
            Face::text(format!("\u{25cc}{character}"))
        }
        Value::Char(character) => Face::text(character.to_string()),
        Value::Key(key) => match key {
            Key::Escape => Face::text("Esc"),
            Key::Tab => Face::glyph(ICON_ARROW_RIGHT_TO_LINE),
            Key::Enter => Face::glyph(ICON_CORNER_DOWN_LEFT),
            Key::Backspace => Face::glyph(ICON_DELETE),
            Key::Delete => Face::text("»"),
            Key::Insert => Face::text("Ins"),
            Key::Home => Face::glyph(ICON_CHEVRON_FIRST),
            Key::End => Face::text("End"),
            Key::PageUp => Face::text("PgUp"),
            Key::PageDown => Face::text("PgDn"),
            Key::ArrowLeft => Face::glyph(ICON_CHEVRON_LEFT),
            Key::ArrowRight => Face::glyph(ICON_CHEVRON_RIGHT),
            Key::ArrowUp => Face::glyph(ICON_CHEVRON_UP),
            Key::ArrowDown => Face::glyph(ICON_CHEVRON_DOWN),
            key => Face::text(format!("{key:?}")),
        },
        Value::Modifier(modifier) => match modifier {
            Modifier::Shift => Face::glyph(ICON_ARROW_BIG_UP),
            Modifier::Ctrl => Face::text("Ctrl"),
            Modifier::Alt => Face::text("Alt"),
            Modifier::Meta => Face::text("Meta"),
            Modifier::Fn => Face::text("Fn"),
            Modifier::Superscript => Face::glyph(ICON_SUPERSCRIPT),
            Modifier::Subscript => Face::glyph(ICON_SUBSCRIPT),
        },
        Value::CapsLock => Face::glyph(ICON_ARROW_BIG_UP_DASH),
        Value::Action(Action::DeleteWordLeft) => Face::around("«", ICON_WHOLE_WORD, ""),
        Value::Action(Action::DeleteWordRight) => Face::around("", ICON_WHOLE_WORD, "»"),
        Value::Action(action) => Face::glyph(match action {
            Action::Copy => ICON_COPY,
            Action::Paste => ICON_CLIPBOARD,
            Action::PastePlain => ICON_CLIPBOARD_PASTE,
            Action::SelectAll => ICON_SQUARE_DASHED_MOUSE_POINTER,
            Action::Undo => ICON_UNDO_2,
            Action::Redo => ICON_REDO_2,
            Action::DeleteWordLeft | Action::DeleteWordRight => ICON_WHOLE_WORD,
        }),
        Value::Layer(layer) => match layer {
            Layer::Text => Face::text("ABC"),
            Layer::Greek => Face::text("αβγ"),
            Layer::Numpad => Face::text("123"),
            Layer::Emoji => Face::glyph(ICON_FACE_SLIGHTLY_SMILING),
        },
        Value::Compose => Face::glyph(ICON_LANGUAGES),
        Value::Slider(Slider::CursorLeft | Slider::SelectWords) => Face::glyph(ICON_CHEVRON_LEFT),
        Value::Slider(Slider::CursorRight) => Face::glyph(ICON_CHEVRON_RIGHT),
    }
}

pub fn composed_face(text: &str) -> Face {
    let mut characters = text.chars();
    match (characters.next(), characters.next()) {
        (Some(character), None) => face(Value::Char(character)),
        _ => Face::text(text),
    }
}

pub fn description(value: Value) -> String {
    match value {
        Value::Char(' ') => "Space".to_owned(),
        Value::Char(character) => character.to_string(),
        Value::Key(key) => format!("{key:?}"),
        Value::Modifier(modifier) => format!("{modifier:?}"),
        Value::CapsLock => "Caps lock".to_owned(),
        Value::Action(action) => match action {
            Action::Copy => "Copy",
            Action::Paste => "Paste",
            Action::PastePlain => "Paste as plain text",
            Action::SelectAll => "Select all",
            Action::Undo => "Undo",
            Action::Redo => "Redo",
            Action::DeleteWordLeft => "Delete word left",
            Action::DeleteWordRight => "Delete word right",
        }
        .to_owned(),
        Value::Layer(layer) => match layer {
            Layer::Text => "Letters",
            Layer::Greek => "Greek and math symbols",
            Layer::Numpad => "Numbers",
            Layer::Emoji => "Emoji",
        }
        .to_owned(),
        Value::Compose => "Compose".to_owned(),
        Value::Slider(Slider::CursorLeft | Slider::CursorRight) => "Move the cursor".to_owned(),
        Value::Slider(Slider::SelectWords) => "Select words to delete".to_owned(),
    }
}

fn is_combining(character: char) -> bool {
    matches!(character, '\u{300}'..='\u{36f}' | '\u{20d0}'..='\u{20ff}')
}

pub const EMOJI_GROUPS: [(emojis::Group, &str, &str); 8] = [
    (emojis::Group::SmileysAndEmotion, "\u{1f600}", "Smileys"),
    (emojis::Group::PeopleAndBody, "\u{1f44b}", "People"),
    (emojis::Group::AnimalsAndNature, "\u{1f43b}", "Nature"),
    (emojis::Group::FoodAndDrink, "\u{1f354}", "Food"),
    (emojis::Group::TravelAndPlaces, "\u{1f697}", "Travel"),
    (emojis::Group::Activities, "\u{26bd}", "Activities"),
    (emojis::Group::Objects, "\u{1f4a1}", "Objects"),
    (emojis::Group::Symbols, "\u{1f523}", "Symbols"),
];

pub fn emojis_in(group: usize) -> Vec<&'static str> {
    let Some((group, _, _)) = EMOJI_GROUPS.get(group) else {
        return Vec::new();
    };
    emojis::iter()
        .filter(|emoji| {
            emoji.group() == *group && beui_components_unstyled::text_area::emoji::drawable(emoji)
        })
        .map(emojis::Emoji::as_str)
        .collect()
}
