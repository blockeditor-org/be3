use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use accesskit::{Node, Role};

use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::KeyPress;

use crate::SyntaxColors;
use crate::TextArea;
use crate::TextAreaColors;
use crate::TextAreaState;
use crate::context_menu::MenuStyle;
use crate::context_menu_menu;
use crate::context_menu_overlay;
use crate::context_menu_toolbar;
use crate::menu_list_len;
use crate::menu_list_row_button;
use crate::text_area_index_at;
use crate::text_area_shown;
use crate::text_menu::TextContextMenu;
use beui_core::node::NodeId;
use beui_macros::{component, view};

use beui_view::reactive::{
    Callback, Child, ClickCallback, Frame, Memo, NodeRef, Prop, ReadSignal, Render, clone,
    create_effect, create_memo, create_signal, set_component_state,
};

const FONT_SIZE: f32 = 14.0;
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

#[derive(Clone)]
pub struct TextInputStyle {
    pub font_size: Prop<f32>,
    pub color: Prop<Color32>,
    pub placeholder_color: Prop<Color32>,
    pub selection_color: Prop<Color32>,
    pub caret_color: Prop<Color32>,
    pub padding_horizontal: Prop<f32>,
    pub padding_vertical: Prop<f32>,
    pub menu: MenuStyle,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            font_size: Prop::Static(FONT_SIZE),
            color: Prop::Static(Color32::WHITE),
            placeholder_color: Prop::Static(PLACEHOLDER_COLOR),
            selection_color: Prop::Static(SELECTION_COLOR),
            caret_color: Prop::Static(Color32::WHITE),
            padding_horizontal: Prop::Static(0.0),
            padding_vertical: Prop::Static(0.0),
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
    #[prop(children)] content: Option<Render<TextInputHandle>>,
    placeholder: Prop<String>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = TextInputStyle::default())] style: TextInputStyle,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_hover_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    on_key_override: Callback<KeyPress, bool>,
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
        menu,
    } = style;
    let initial = value.peek();
    let state = plain_text(&initial);
    let (hovered, set_hovered) = create_signal(false);
    let (is_focused, set_focused) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let masked = create_memo(move || password.get());
    let (area, menu_node) = (NodeRef::new(), NodeRef::new());
    set_component_state(Input {
        state: state.clone(),
        area: area.clone(),
        menu: menu_node.clone(),
        focused: is_focused.clone(),
    });

    create_effect(clone!(state -> move || {
        let value = value.get();
        if text_of(&state) != value {
            state.execute(EditorCommand::ReplaceWholeFile(value.as_bytes()));
        }
    }));
    let edited = state.content();
    let empty = create_memo(clone!(state -> move || {
        edited.get();
        text_of(&state).is_empty()
    }));
    let clear = ClickCallback::new(clone!(state -> move || {
        state.execute(EditorCommand::ReplaceWholeFile(b""));
    }));
    let reported = Rc::new(RefCell::new(initial));
    let content_changed = state.content();
    create_effect(clone!(state -> move || {
        content_changed.get();
        let value = text_of(&state);
        if *reported.borrow() == value {
            return;
        }
        reported.replace(value.clone());
        on_change.call(value);
    }));

    let colors = create_memo(move || TextAreaColors {
        surface: Color32::TRANSPARENT,
        selection: selection_color.get(),
        caret: caret_color.get(),
        placeholder: placeholder_color.get(),
        syntax: SyntaxColors::uniform(color.get()),
        ..TextAreaColors::DEFAULT
    });
    let accessibility = crate::labelled_node(Role::TextInput, accessibility, label);

    let opener: Rc<RefCell<Option<Callback<Option<Rect>>>>> = Rc::default();
    let open_menu = clone!(opener -> move |at: Option<Rect>| {
        if let Some(open) = opener.borrow().clone() {
            open.call(at);
        }
    });
    let submit_state = state.clone();
    let focus_state = state.clone();
    let menu_state = state.clone();
    let masked_menu = masked.clone();
    let menu_disabled = disabled.clone();
    let field_disabled = disabled.clone();
    let handle_focused = is_focused.clone();
    view! {
        <TextArea
            @node_ref=&area
            state={state}
            single_line=true
            colors
            placeholder
            password={masked}
            focused
            keyboard_on_focus
            disabled={disabled}
            font_size
            padding=Vec2::ZERO
            accessibility
            on_toolbar={open_menu}
            on_key_override={move |press: KeyPress| on_key_override.call(press)}
            on_submit={move || on_submit.call(text_of(&submit_state))}
            on_focus_change={move |focused: bool| {
                if focused && select_on_focus.peek() {
                    focus_state.execute(EditorCommand::SelectAll);
                }
                set_focused.set(focused);
                on_focus_change.call(focused);
            }}
            on_hover_change={move |is_hovered: bool| {
                set_hovered.set(is_hovered);
                on_hover_change.call(is_hovered);
            }}
            frame={move |field: Child| {
                let field = view! {
                    <Frame padding_horizontal padding_vertical>{field}</Frame>
                };
                view! {
                    <TextContextMenu
                        @node_ref=&menu_node
                        state={menu_state}
                        menu
                        masked={masked_menu}
                        disabled={menu_disabled}
                    >
                        {move |open: Callback<Option<Rect>>| {
                            opener.replace(Some(open));
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
            }}
        />
    }
}

fn plain_text(value: &str) -> TextAreaState {
    let buffer = Arc::new(TextBuffer::new(value.as_bytes()));
    let state = TextAreaState::new(buffer as Arc<dyn text_editor_core::Document>);
    state.execute(EditorCommand::SetLanguage(TextLanguage::PlainText));
    let end = state.core().position(value.len());
    state.execute(EditorCommand::SetSelection {
        anchor: end,
        focus: end,
    });
    state
}

fn text_of(state: &TextAreaState) -> String {
    let core = state.core();
    let Some(read) = core.document().read() else {
        return String::new();
    };
    String::from_utf8_lossy(&read.slice(0..read.len())).into_owned()
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
    text_of(&input(document, input_node).state)
}

pub fn text_input_focused(document: &Document, input_node: NodeId) -> ReadSignal<bool> {
    input(document, input_node).focused.clone()
}

pub fn text_input_caret(document: &Document, input_node: NodeId) -> usize {
    input(document, input_node)
        .state
        .caret_indices()
        .first()
        .copied()
        .unwrap_or(0)
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
    if !document.is_overlay_open(context_menu_overlay(document, menu)) {
        return None;
    }
    let list = context_menu_menu(document, menu);
    (index < menu_list_len(document, list)).then(|| menu_list_row_button(document, list, index))
}

pub fn text_input_toolbar(document: &Document, input_node: NodeId) -> Option<NodeId> {
    let menu = input(document, input_node).menu.try_get()?;
    let toolbar = context_menu_toolbar(document, menu);
    document.is_overlay_open(toolbar).then_some(toolbar)
}

pub fn text_input_handles(document: &Document, input_node: NodeId) -> Vec<Pos2> {
    crate::text_area_handles(document, input(document, input_node).area.get())
}
