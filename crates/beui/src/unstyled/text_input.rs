use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use accesskit::{Node, Role};

use text_editor_core::{CopyMode, EditorCommand, TextBuffer, TextLanguage};

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Pos2, Vec2};
use crate::input::KeyPress;

use crate::node::NodeId;
use crate::unstyled::{
    ContextMenu, MenuItem, MenuRowHandle, SyntaxColors, TextArea, TextAreaColors, TextAreaState,
    context_menu_menu, context_menu_overlay, menu_list_len, menu_list_row_button,
    text_area_index_at, text_area_shown,
};
use beui_macros::{component, view};

use crate::reactive::{
    Callback, Child, ForEach, Frame, List, Memo, NodeRef, Prop, ReadSignal, Render, RenderFn,
    clone, copy_text, create_effect, create_memo, create_signal, request_paste,
    set_component_state,
};

const FONT_SIZE: f32 = 14.0;
const PLACEHOLDER_COLOR: Color32 = Color32::from_gray(140);

pub struct TextInputHandle {
    pub field: Child,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[derive(Clone, Default)]
pub struct TextInputMenu(Option<(RenderFn<MenuRowHandle>, RenderFn<Child>)>);

impl TextInputMenu {
    pub fn new(
        row: impl Fn(MenuRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
    ) -> Self {
        Self(Some((RenderFn::new(row), RenderFn::new(panel))))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum MenuAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
}

impl MenuAction {
    fn label(self) -> &'static str {
        match self {
            MenuAction::Copy => "Copy",
            MenuAction::Cut => "Cut",
            MenuAction::Paste => "Paste",
            MenuAction::SelectAll => "Select All",
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
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] select_on_focus: Prop<bool>,
    #[prop(children)] content: Option<Render<TextInputHandle>>,
    placeholder: Prop<String>,
    #[prop(default = FONT_SIZE)] font_size: Prop<f32>,
    #[prop(default = Color32::WHITE)] color: Prop<Color32>,
    #[prop(default = PLACEHOLDER_COLOR)] placeholder_color: Prop<Color32>,
    selection_color: Prop<Color32>,
    caret_color: Prop<Color32>,
    padding_horizontal: Prop<f32>,
    #[prop(default = 0.0)] padding_vertical: Prop<f32>,
    #[prop(default = TextInputMenu::default())] menu: TextInputMenu,
    on_change: Callback<String>,
    on_submit: Callback<String>,
    on_hover_change: Callback<bool>,
    on_focus_change: Callback<bool>,
    on_key_override: Callback<KeyPress, bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let initial = value.peek();
    let state = plain_text(&initial);
    let (hovered, set_hovered) = create_signal(false);
    let (is_focused, set_focused) = create_signal(false);
    let (menu_at, set_menu_at) = create_signal(None::<Pos2>);
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
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::TextInput)));

    let cursors = state.cursors();
    let actions = create_memo(clone!(state masked -> move || {
        cursors.get();
        let selected = state.selection_ranges().iter().any(|range| !range.is_empty());
        match selected && !masked.get() {
            true => vec![
                MenuAction::Copy,
                MenuAction::Cut,
                MenuAction::Paste,
                MenuAction::SelectAll,
            ],
            false => vec![MenuAction::Paste, MenuAction::SelectAll],
        }
    }));
    let chosen = actions.clone();
    let no_menu = menu.0.is_none();
    let (row, panel) = menu.0.unwrap_or_else(|| {
        (
            RenderFn::new(|_| {
                view! {
                    <List spacing=0.0 />
                }
            }),
            RenderFn::new(|content| content),
        )
    });
    let menu_off = create_memo(clone!(disabled -> move || no_menu || disabled.get()));
    let open_menu = clone!(menu_off set_menu_at -> move |at: Pos2| {
        if !menu_off.get_untracked() {
            set_menu_at.set(Some(at));
        }
    });
    let submit_state = state.clone();
    let focus_state = state.clone();
    let menu_state = state.clone();
    let close_menu = set_menu_at.clone();
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
            disabled={disabled}
            font_size
            padding=Vec2::ZERO
            accessibility
            on_menu={open_menu}
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
                    <ContextMenu
                        @node_ref=&menu_node
                        row
                        panel
                        disabled={menu_off}
                        open_at={menu_at}
                        open_at_focuses=false
                        on_close={move || close_menu.set(None)}
                        items={view! {
                            <ForEach keys={actions}>
                                {move |action: MenuAction| view! {
                                    <MenuItem label={action.label().to_owned()} />
                                }}
                            </ForEach>
                        }}
                        on_select={move |path: Vec<usize>| {
                            let Some(action) = path
                                .first()
                                .and_then(|index| chosen.get_untracked().get(*index).copied())
                            else {
                                return;
                            };
                            menu_state.focus();
                            menu_action(&menu_state, action);
                        }}
                    >
                        {match content {
                            Some(build) => build.call(TextInputHandle {
                                field,
                                hovered,
                                focused: handle_focused,
                                disabled: field_disabled,
                            }),
                            None => field,
                        }}
                    </ContextMenu>
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

fn menu_action(state: &TextAreaState, action: MenuAction) {
    match action {
        MenuAction::Copy | MenuAction::Cut => {
            let mode = match action {
                MenuAction::Cut => CopyMode::Cut,
                _ => CopyMode::Copy,
            };
            let text = state.copy(mode);
            if !text.is_empty() {
                copy_text(text);
            }
        }
        MenuAction::Paste => request_paste(),
        MenuAction::SelectAll => state.execute(EditorCommand::SelectAll),
    }
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

#[cfg(test)]
pub(crate) fn text_input_handles(document: &Document, input_node: NodeId) -> Vec<Pos2> {
    crate::unstyled::text_area_handles(document, input(document, input_node).area.get())
}
