use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use accesskit::{Node, Role};

use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

use beui_core::base::ItemSize;
use beui_core::base::text::DEFAULT_FONT_SIZE;
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Vec2};

use crate::Prose;
use crate::RemoteTextCursor;
use crate::SyntaxColors;
use crate::TextArea;
use crate::TextAreaColors;
use crate::TextAreaState;
use crate::context_menu_menu;
use crate::context_menu_open;
use crate::menu_list_len;
use crate::menu_list_row_button;
use crate::menu_popup::MenuStyle;
use crate::text_area_index_at;
use crate::text_area_shown;
use crate::text_menu::TextContextMenu;
use beui_core::node::NodeId;
use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::*;

const PLACEHOLDER_COLOR: Color32 = Color32::from_gray(140);
const SELECTION_COLOR: Color32 = Color32::from_rgba_unmultiplied(120, 160, 255, 90);

pub struct TextInputHandle {
    pub field: Child,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
    pub empty: Memo<bool>,
    pub clear: ClickCallback,
}

#[derive(Clone, Copy)]
pub struct TextInputStyle {
    pub font_size: Prop<f32>,
    pub color: Prop<Color32>,
    pub placeholder_color: Prop<Color32>,
    pub selection_color: Prop<Color32>,
    pub caret_color: Prop<Color32>,
    pub padding_horizontal: Prop<f32>,
    pub padding_vertical: Prop<f32>,
    pub prose: Prop<Option<Prose>>,
    pub menu: MenuStyle,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            font_size: Prop::Static(DEFAULT_FONT_SIZE),
            color: Prop::Static(Color32::WHITE),
            placeholder_color: Prop::Static(PLACEHOLDER_COLOR),
            selection_color: Prop::Static(SELECTION_COLOR),
            caret_color: Prop::Static(Color32::WHITE),
            padding_horizontal: Prop::Static(0.0),
            padding_vertical: Prop::Static(0.0),
            prose: Prop::Static(None),
            menu: MenuStyle::default(),
        }
    }
}

struct Input {
    state: TextAreaState,
    area: NodeRef,
    menu: NodeRef,
    focused: ReadSignal<bool>,
}

#[component]
pub fn TextInput(
    value: Prop<String>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = true)] keyboard_on_focus: bool,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] select_on_focus: Prop<bool>,
    #[prop(default = false)] multiline: bool,
    #[prop(children)] content: Option<Render<TextInputHandle>>,
    placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = TextInputStyle::default())] style: TextInputStyle,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_hover_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let TextInputStyle {
        font_size,
        color,
        placeholder_color,
        selection_color,
        caret_color,
        padding_horizontal,
        padding_vertical,
        prose,
        menu,
    } = style;
    let initial = value.peek();
    let (state, buffer) = plain_text(&initial);
    let (hovered, set_hovered) = create_signal(false);
    let (is_focused, set_focused) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let masked = create_memo(move || password.get());
    let (area, menu_node) = (NodeRef::new(), NodeRef::new());
    set_component_state(Input {
        state,
        area,
        menu: menu_node,
        focused: is_focused,
    });

    create_effect(move || {
        if buffer.adopt(value.get().as_bytes()) {
            state.external_edit();
        }
    });
    let edited = state.content();
    let empty = create_memo(move || {
        edited.get();
        text_of(state).is_empty()
    });
    let clear = ClickCallback::new(move || {
        state.execute(EditorCommand::ReplaceWholeFile(b""));
    });
    let reported = Rc::new(RefCell::new(initial));
    let content_changed = state.content();
    create_effect(move || {
        content_changed.get();
        let value = text_of(state);
        if *reported.borrow() == value {
            return;
        }
        reported.replace(value.clone());
        on_change.call(value);
    });

    let colors = create_memo(move || TextAreaColors {
        surface: Color32::TRANSPARENT,
        selection: selection_color.get(),
        caret: caret_color.get(),
        placeholder: placeholder_color.get(),
        syntax: SyntaxColors::uniform(color.get()),
        ..TextAreaColors::DEFAULT
    });
    let role = match multiline {
        true => Role::MultilineTextInput,
        false => Role::TextInput,
    };
    let accessibility = crate::labelled_node(role, accessibility, label);

    let opener = store(Cell::new(None::<Callback<Pos2>>));
    let open_menu = move |at: Pos2| {
        if let Some(open) = opener.with(Cell::get) {
            open.call(at);
        }
    };
    let masked_menu = masked;
    let menu_disabled = disabled;
    let field_disabled = disabled;
    let handle_focused = is_focused;
    let surround = move |field: Child, padded: bool| {
        let field = match padded {
            true => view! {
                <Frame
                    padding={create_memo(move || Sides::symmetric(padding_horizontal.get(), padding_vertical.get()))}
                >
                    {field}
                </Frame>
            },
            false => field,
        };
        view! {
            <TextContextMenu
                @node_ref=&menu_node
                state={state}
                menu
                masked={masked_menu}
                disabled={menu_disabled}
                child_size={match padded {
                    true => ItemSize::Intrinsic,
                    false => ItemSize::Percent(100.0),
                }}
            >
                {move |open: Callback<Pos2>| {
                    opener.with(|opener| opener.set(Some(open)));
                    match content {
                        Some(build) => build.call(TextInputHandle {
                            field,
                            hovered,
                            focused: handle_focused,
                            disabled: field_disabled,
                            empty,
                            clear,
                        }),
                        None => field,
                    }
                }}
            </TextContextMenu>
        }
    };
    let surround = store(RefCell::new(Some(surround)));
    let surround = move |field: Child, padded: bool| {
        let wrap = surround
            .with(|surround| surround.borrow_mut().take())
            .expect("a text input wraps its field once");
        wrap(field, padded)
    };
    let padding = create_memo(move || match multiline {
        true => Vec2::new(padding_horizontal.get(), padding_vertical.get()),
        false => Vec2::ZERO,
    });
    let area_node = view! {
        <TextArea
            @node_ref=&area
            state={state}
            single_line={!multiline}
            line_numbers=false
            colors
            placeholder
            password={masked}
            focused
            keyboard_on_focus
            disabled={disabled}
            font_size
            padding
            prose
            remote_cursors
            accessibility
            on_menu={open_menu}
            on_submit={move || on_submit.call(text_of(state))}
            on_focus_change={move |focused: bool| {
                if focused && select_on_focus.peek() {
                    state.execute(EditorCommand::SelectAll);
                }
                set_focused.set(focused);
                on_focus_change.call(focused);
            }}
            on_hover_change={move |is_hovered: bool| {
                set_hovered.set(is_hovered);
                on_hover_change.call(is_hovered);
            }}
            frame={move |field: Child| surround(field, true)}
        />
    };
    match multiline {
        true => surround(area_node, false),
        false => area_node,
    }
}

pub fn measure_prose(
    value: &str,
    placeholder: &str,
    font_size: f32,
    prose: Prose,
    wrap_width: f32,
) -> Option<Vec2> {
    plain_text(value)
        .0
        .measure_prose(font_size, prose, placeholder, wrap_width)
}

fn plain_text(value: &str) -> (TextAreaState, Arc<TextBuffer>) {
    let buffer = Arc::new(TextBuffer::new(value.as_bytes()));
    let state = TextAreaState::new(buffer.clone() as Arc<dyn text_editor_core::Document>);
    state.execute(EditorCommand::SetLanguage(TextLanguage::PlainText));
    let end = state.with_core(|core| core.position(value.len()));
    state.execute(EditorCommand::SetSelection {
        anchor: end,
        focus: end,
    });
    (state, buffer)
}

fn text_of(state: TextAreaState) -> String {
    state.with_core(|core| {
        let Some(read) = core.document().read() else {
            return String::new();
        };
        String::from_utf8_lossy(&read.slice(0..read.len())).into_owned()
    })
}

fn input(document: &Document, input: NodeId) -> &Input {
    document.component_state::<Input>(input)
}

pub fn text_input_text(document: &Document, input_node: NodeId) -> NodeId {
    input(document, input_node).state.canvas().get()
}

pub fn text_input_shown(document: &Document, input_node: NodeId) -> String {
    text_area_shown(document, input(document, input_node).area.get())
}

pub fn text_input_index_at(document: &Document, input_node: NodeId, pos: Pos2) -> usize {
    text_area_index_at(document, input(document, input_node).area.get(), pos)
}

pub fn text_input_value(document: &Document, input_node: NodeId) -> String {
    text_of(input(document, input_node).state)
}

pub fn text_input_focused(document: &Document, input_node: NodeId) -> ReadSignal<bool> {
    input(document, input_node).focused
}

pub fn text_input_caret(document: &Document, input_node: NodeId) -> usize {
    input(document, input_node)
        .state
        .caret_indices()
        .first()
        .copied()
        .unwrap_or(0)
}

pub fn text_input_state(document: &Document, input_node: NodeId) -> TextAreaState {
    input(document, input_node).state
}

pub fn text_input_selection(document: &Document, input_node: NodeId) -> Vec<Range<usize>> {
    input(document, input_node)
        .state
        .selection_ranges()
        .into_iter()
        .filter(|range| range.start < range.end)
        .collect()
}

pub fn text_input_menu_row(
    document: &Document,
    input_node: NodeId,
    index: usize,
) -> Option<NodeId> {
    let menu = input(document, input_node).menu.try_get()?;
    if !context_menu_open(document, menu) {
        return None;
    }
    let list = context_menu_menu(document, menu);
    (index < menu_list_len(document, list)).then(|| menu_list_row_button(document, list, index))
}

pub fn text_input_handles(document: &Document, input_node: NodeId) -> Vec<Pos2> {
    crate::text_area_handles(document, input(document, input_node).area.get())
}
