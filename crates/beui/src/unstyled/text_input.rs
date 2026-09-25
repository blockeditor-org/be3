use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use accesskit::{Node, Role};

use text_editor_core::{
    CopyMode, Core, CursorLeftRightStop, DragSelectionMode, EditorCommand, LRDirection, MoveMode,
    Position, TextBuffer, TextLanguage,
};

use crate::color::Color32;
use crate::geometry::{Pos2, Vec2, pos2};
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};

use crate::base::TextAlign;
use crate::base::overlay::{Overlay, OverlayAnchor};
use crate::base::text::TextHandle;
use crate::base::text_index_at;
use crate::document::Document;
use crate::node::NodeId;
use crate::unstyled::MenuRowHandle;
use beui_macros::{component, view};

use crate::reactive::{
    Callback, Child, ClickCatcher, Dynamic, Focusable, ForEach, Frame, IntoProp, ItemSize, List,
    Memo, NodeRef, Prop, ReadSignal, Render, RenderFn, Show, Text, WriteSignal, clone,
    component_accessibility, copy_text, create_effect, create_memo, create_signal,
    set_component_state, with_document,
};

const FONT_SIZE: f32 = 14.0;
const PLACEHOLDER_COLOR: Color32 = Color32::from_gray(140);
const WORD_CLICKS: u32 = 2;
const LINE_CLICKS: u32 = 3;
const ALL_CLICKS: u32 = 4;
const MENU_SPACING: f32 = 2.0;
const AUTOSCROLL_STEP: Duration = Duration::from_millis(40);

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

#[derive(Clone, Copy, PartialEq, Debug)]
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

#[derive(Clone, Copy)]
enum Grab {
    Selection(Position),
    Caret,
}

struct Editor {
    core: Core,
    dragging: bool,
    touch: bool,
    caret_handle: bool,
    grab: Option<(Grab, Vec2)>,
    last_step: Instant,
    text: NodeRef,
    menu_rows: Vec<NodeRef>,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    set_caret: WriteSignal<Option<usize>>,
    set_selection: WriteSignal<Vec<Range<usize>>>,
    set_handles: WriteSignal<bool>,
    set_autoscroll: WriteSignal<bool>,
    set_menu: WriteSignal<Option<Pos2>>,
    set_menu_actions: WriteSignal<Vec<MenuAction>>,
    focused: ReadSignal<bool>,
    on_change: Callback<String>,
    on_submit: Callback<String>,
}

type Handle = Rc<RefCell<Editor>>;

#[component]
pub fn TextInput(
    value: Prop<String>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] password: Prop<bool>,
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
    let focus_request = focused;
    let (hovered, set_hovered) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (text_value, set_value) = create_signal(initial.clone());
    let (caret, set_caret) = create_signal(None);
    let (selection, set_selection) = create_signal(Vec::new());
    let (handles, set_handles) = create_signal(false);
    let (autoscroll, set_autoscroll) = create_signal(false);
    let (menu_at, set_menu) = create_signal(None);
    let (menu_actions, set_menu_actions) = create_signal(Vec::new());
    let text = NodeRef::new();
    let placeholder = create_memo(move || placeholder.get());
    let password = create_memo(move || password.get());
    let string = shown_string(&text_value, placeholder.clone(), password.clone());
    let color = shown_color(&text_value, color, placeholder_color);
    let disabled = create_memo(move || disabled.get());
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::TextInput)));
    component_accessibility(create_memo(
        clone!(text_value placeholder disabled password -> move || {
            let mut node = accessibility.get();
            match password.get() {
                true => node.set_value(mask(&text_value.get())),
                false => node.set_value(text_value.get()),
            }
            node.set_placeholder(placeholder.get());
            if disabled.get() {
                node.set_disabled();
            } else {
                node.clear_disabled();
            }
            node
        }),
    ));

    let editor: Handle = Rc::new(RefCell::new(Editor {
        core: core(&initial),
        dragging: false,
        touch: false,
        caret_handle: false,
        grab: None,
        last_step: Instant::now(),
        text: text.clone(),
        menu_rows: Vec::new(),
        value: text_value.clone(),
        set_value,
        set_caret,
        set_selection,
        set_handles,
        set_autoscroll: set_autoscroll.clone(),
        set_menu,
        set_menu_actions,
        focused: focused.clone(),
        on_change,
        on_submit,
    }));
    set_component_state(editor.clone());
    create_effect(clone!(editor -> move || {
        let value = value.get();
        if text_of(&editor.borrow().core) != value {
            replace_all(&editor, value);
        }
    }));

    let cursor = create_memo(clone!(disabled -> move || match disabled.get() {
        true => CursorIcon::Default,
        false => CursorIcon::Text,
    }));
    let tab_stop = disabled.clone().into_prop().map(|disabled: bool| !disabled);
    let (capture_off, press_off, tap_off, drag_off, field_off) = (
        disabled.clone(),
        disabled.clone(),
        disabled.clone(),
        disabled.clone(),
        disabled.clone(),
    );
    let menu = menu.0;
    view! {
        <Focusable
            focused={focus_request}
            tab_stop
            ime={create_memo(clone!(disabled -> move || !disabled.get()))}
            on_focus_change={{
                let editor = editor.clone();
                move |is_focused: bool| {
                    set_focused.set(is_focused);
                    if !is_focused {
                        let set_autoscroll = {
                            let mut editor = editor.borrow_mut();
                            editor.dragging = false;
                            editor.grab = None;
                            editor.caret_handle = false;
                            editor.core.external_edit();
                            editor.set_autoscroll.clone()
                        };
                        set_autoscroll.set(false);
                        close_menu(&editor);
                    }
                    show(&editor);
                    on_focus_change.call(is_focused);
                }
            }}
            on_text={{
                let editor = editor.clone();
                let disabled = disabled.clone();
                move |typed: String| {
                    if !disabled.get_untracked() {
                        insert(&editor, &typed);
                    }
                }
            }}
            on_key={{
                let editor = editor.clone();
                let disabled = disabled.clone();
                move |press: KeyPress| {
                    !disabled.get_untracked() && (on_key_override.call(press) || key(&editor, press))
                }
            }}
        >
            <List spacing=0.0>
                <ClickCatcher
                    @sizing=ItemSize::Percent(100.0)
                    cursor
                    repeat_drag={autoscroll}
                    capture_at={{
                        let editor = editor.clone();
                        move |pos: Pos2| !capture_off.get_untracked() && handle_at(&editor, pos).is_some()
                    }}
                    on_press={{
                        let editor = editor.clone();
                        move |press: PointerPress| {
                            if !press_off.get_untracked() {
                                point(&editor, press);
                            }
                        }
                    }}
                    on_click_at={{
                        let editor = editor.clone();
                        move |press: PointerPress| {
                            if !tap_off.get_untracked() {
                                tap(&editor, press);
                            }
                        }
                    }}
                    on_drag={{
                        let editor = editor.clone();
                        move |press: PointerPress| {
                            if !drag_off.get_untracked() {
                                extend(&editor, press);
                            }
                        }
                    }}
                    on_active_change={move |active: bool| {
                        if !active {
                            set_autoscroll.set(false);
                        }
                    }}
                    on_hover_change={move |is_hovered: bool| {
                        set_hovered.set(is_hovered);
                        on_hover_change.call(is_hovered);
                    }}
                >
                    {{
                        let field = view! {
                            <Frame
                                padding_horizontal={padding_horizontal}
                                padding_vertical={padding_vertical}
                            >
                                <Text
                                    @node_ref=&text
                                    string
                                    font_size
                                    color
                                    selection_color
                                    caret_color
                                    caret
                                    selection
                                    handles
                                    align=TextAlign::Start
                                    clip=true
                                />
                            </Frame>
                        };
                        match content {
                            Some(build) => build.call(TextInputHandle {
                                field,
                                hovered,
                                focused,
                                disabled: field_off.clone(),
                            }),
                            None => field,
                        }
                    }}
                </ClickCatcher>
                <Show condition={menu.is_some()}>
                    {move || {
                        let (row, panel) =
                            menu.expect("a touch menu is only shown when it has builders");
                        view! {
                            <TouchMenu
                                editor={editor.clone()}
                                at={menu_at}
                                actions={menu_actions}
                                row
                                panel
                            />
                        }
                    }}
                </Show>
            </List>
        </Focusable>
    }
}

#[component]
fn TouchMenu(
    editor: Handle,
    at: ReadSignal<Option<Pos2>>,
    actions: ReadSignal<Vec<MenuAction>>,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
) -> NodeId {
    let open = create_memo(clone!(at -> move || at.get().is_some()));
    let anchor = create_memo(move || OverlayAnchor::Point(at.get().unwrap_or(Pos2::ZERO)));
    view! {
        <List spacing=0.0>
            <Show condition={open.clone()}>
                {move || {
                    let dismiss = editor.borrow().set_menu.clone();
                    view! {
                        <Overlay
                            anchor
                            open
                            traps_focus=false
                            on_dismiss={move || dismiss.set(None)}
                        >
                            {panel.call(view! {
                                <List spacing=0.0>
                                    <Dynamic value={actions}>
                                        {move |actions: Vec<MenuAction>| view! {
                                            <TouchMenuRows
                                                editor={editor.clone()}
                                                actions
                                                row={row.clone()}
                                            />
                                        }}
                                    </Dynamic>
                                </List>
                            })}
                        </Overlay>
                    }
                }}
            </Show>
        </List>
    }
}

#[component]
fn TouchMenuRows(editor: Handle, actions: Vec<MenuAction>, row: RenderFn<MenuRowHandle>) -> NodeId {
    let slots: Vec<NodeRef> = actions.iter().map(|_| NodeRef::new()).collect();
    editor.borrow_mut().menu_rows = slots.clone();
    let rows: Rc<Vec<(MenuAction, NodeRef)>> = Rc::new(actions.into_iter().zip(slots).collect());
    let keys: Vec<usize> = (0..rows.len()).collect();
    view! {
        <List spacing=MENU_SPACING>
            <ForEach keys>
                {move |index: usize| {
                    let (action, slot) = rows[index].clone();
                    view! {
                        <TouchMenuRow editor={editor.clone()} slot action row={row.clone()} />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TouchMenuRow(
    editor: Handle,
    slot: NodeRef,
    action: MenuAction,
    row: RenderFn<MenuRowHandle>,
) -> NodeId {
    let (hovered, set_hovered) = create_signal(false);
    let (focused, _) = create_signal(false);
    view! {
        <ClickCatcher
            @node_ref=&slot
            capture_presses=true
            on_click={move || menu_action(&editor, action)}
            on_hover_change={move |is_hovered: bool| set_hovered.set(is_hovered)}
        >
            {row.call(MenuRowHandle {
                label: Prop::Static(action.label().to_owned()),
                disabled: Prop::Static(false),
                has_submenu: Prop::Static(false),
                hovered,
                focused,
            })}
        </ClickCatcher>
    }
}

fn shown_string(
    value: &ReadSignal<String>,
    placeholder: Memo<String>,
    password: Memo<bool>,
) -> Memo<String> {
    let value = value.clone();
    create_memo(move || match value.get() {
        text if text.is_empty() => placeholder.get(),
        text if password.get() => mask(&text),
        text => text,
    })
}

fn mask(text: &str) -> String {
    "*".repeat(text.len())
}

fn shown_color(
    value: &ReadSignal<String>,
    color: Prop<Color32>,
    placeholder_color: Prop<Color32>,
) -> Memo<Color32> {
    let value = value.clone();
    create_memo(move || {
        if value.get().is_empty() {
            placeholder_color.get()
        } else {
            color.get()
        }
    })
}

fn handle(document: &Document, input: NodeId) -> &Handle {
    document.component_state::<Handle>(input)
}

pub fn text_input_text(document: &Document, input: NodeId) -> NodeId {
    handle(document, input).borrow().text.get()
}

pub fn text_input_value(document: &Document, input: NodeId) -> String {
    text_of(&handle(document, input).borrow().core)
}

pub fn text_input_focused(document: &Document, input: NodeId) -> ReadSignal<bool> {
    handle(document, input).borrow().focused.clone()
}

pub fn text_input_caret(document: &Document, input: NodeId) -> usize {
    caret(&handle(document, input).borrow().core)
}

pub fn text_input_selection(document: &Document, input: NodeId) -> Vec<Range<usize>> {
    selection(&handle(document, input).borrow().core)
}

pub fn text_input_menu_row(document: &Document, input: NodeId, index: usize) -> Option<NodeId> {
    handle(document, input)
        .borrow()
        .menu_rows
        .get(index)?
        .try_get()
}

#[cfg(test)]
pub(crate) fn text_input_handles(document: &Document, input: NodeId) -> Vec<Pos2> {
    document.text_handle_centers(text_input_text(document, input))
}

fn replace_all(editor: &Handle, value: String) {
    command(editor, EditorCommand::ReplaceWholeFile(value.as_bytes()));
}

fn core(value: &str) -> Core {
    let buffer = Arc::new(TextBuffer::new(value.as_bytes()));
    let mut core = Core::new(buffer as Arc<dyn text_editor_core::Document>);
    core.execute_command(EditorCommand::SetLanguage(TextLanguage::PlainText));
    let end = core.position(value.len());
    core.execute_command(EditorCommand::SetSelection {
        anchor: end,
        focus: end,
    });
    core
}

fn text_of(core: &Core) -> String {
    let Some(read) = core.document().read() else {
        return String::new();
    };
    String::from_utf8_lossy(&read.slice(0..read.len())).into_owned()
}

fn caret(core: &Core) -> usize {
    core.cursor_positions()
        .first()
        .and_then(|cursor| core.position_index(cursor.pos.focus))
        .unwrap_or(0)
}

fn selection(core: &Core) -> Vec<Range<usize>> {
    core.cursor_positions()
        .iter()
        .filter_map(|cursor| core.selection_range(cursor))
        .filter(|range| range.start < range.end)
        .collect()
}

fn command(editor: &Handle, command: EditorCommand<'_>) {
    editor.borrow_mut().core.execute_command(command);
    close_menu(editor);
    show(editor);
}

fn show(editor: &Handle) {
    let (on_change, value) = {
        let editor = editor.borrow();
        let value = text_of(&editor.core);
        let focused = editor.focused.get_untracked();
        let selection = selection(&editor.core);
        editor.set_caret.set(focused.then(|| caret(&editor.core)));
        editor
            .set_handles
            .set(editor.touch && focused && (editor.caret_handle || !selection.is_empty()));
        editor.set_selection.set(selection);
        if editor.value.get_untracked() == value {
            return;
        }
        editor.set_value.set(value.clone());
        (editor.on_change.clone(), value)
    };
    on_change.call(value);
}

fn insert(editor: &Handle, typed: &str) {
    let typed: String = typed
        .chars()
        .filter(|letter| !letter.is_control())
        .collect();
    if typed.is_empty() {
        return;
    }
    editor.borrow_mut().caret_handle = false;
    command(editor, EditorCommand::InsertText(typed.as_bytes()));
}

fn click_mode(clicks: u32) -> DragSelectionMode {
    match clicks {
        WORD_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Word),
        LINE_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Line),
        _ => DragSelectionMode::move_to(CursorLeftRightStop::UnicodeGraphemeCluster),
    }
}

fn handle_at(editor: &Handle, pos: Pos2) -> Option<TextHandle> {
    let text = editor.borrow().text.get();
    with_document(|document| document.text_handle_at(text, pos))
}

fn grab_handle(editor: &Handle, pos: Pos2) -> bool {
    let Some(handle) = handle_at(editor, pos) else {
        return false;
    };
    let (text, range, caret_index) = {
        let state = editor.borrow();
        (
            state.text.get(),
            selection(&state.core).first().cloned(),
            caret(&state.core),
        )
    };
    let (fixed, moving) = match (handle, range) {
        (TextHandle::Start, Some(range)) => (Some(range.end), range.start),
        (TextHandle::End, Some(range)) => (Some(range.start), range.end),
        (TextHandle::Caret, _) => (None, caret_index),
        _ => return false,
    };
    let anchor = with_document(|document| document.text_caret_middle(text, moving));
    let mut state = editor.borrow_mut();
    let grab = match fixed {
        Some(fixed) => Grab::Selection(state.core.position(fixed)),
        None => Grab::Caret,
    };
    state.dragging = false;
    state.last_step = Instant::now();
    state.grab = Some((grab, pos - anchor));
    true
}

fn point(editor: &Handle, press: PointerPress) {
    editor.borrow_mut().grab = None;
    if grab_handle(editor, press.pos) {
        return;
    }
    editor.borrow_mut().touch = press.touch;
    if press.touch {
        editor.borrow_mut().dragging = false;
        show(editor);
        return;
    }
    let index = {
        let mut state = editor.borrow_mut();
        state.caret_handle = false;
        let text = state.text.clone();
        drop(state);
        text_index_at(&text, press.pos)
    };
    let position = editor.borrow().core.position(index);
    let dragging = press.clicks < ALL_CLICKS;
    editor.borrow_mut().dragging = dragging;
    if !dragging {
        command(editor, EditorCommand::SelectAll);
        return;
    }
    command(
        editor,
        EditorCommand::Click {
            position,
            mode: click_mode(press.clicks),
            extend: press.modifiers.shift,
            select_syntax_node: false,
        },
    );
}

fn tap(editor: &Handle, press: PointerPress) {
    let (text, focused, grab) = {
        let state = editor.borrow();
        (
            state.text.clone(),
            state.focused.get_untracked(),
            state.grab.map(|(grab, _)| grab),
        )
    };
    if !press.touch {
        return;
    }
    match grab {
        Some(Grab::Caret) => return open_menu(editor, press.pos),
        Some(Grab::Selection(_)) => return,
        None => {}
    }
    let index = text_index_at(&text, press.pos);
    let inside_selection = selection(&editor.borrow().core)
        .iter()
        .any(|range| range.start <= index && index <= range.end);
    if focused && press.clicks == 1 && inside_selection {
        return open_menu(editor, press.pos);
    }
    editor.borrow_mut().caret_handle = true;
    if press.clicks >= ALL_CLICKS {
        command(editor, EditorCommand::SelectAll);
        return;
    }
    let position = editor.borrow().core.position(index);
    command(
        editor,
        EditorCommand::Click {
            position,
            mode: click_mode(press.clicks),
            extend: false,
            select_syntax_node: false,
        },
    );
}

fn extend(editor: &Handle, press: PointerPress) {
    let (text, grab, dragging) = {
        let state = editor.borrow();
        (state.text.clone(), state.grab, state.dragging)
    };
    if let Some((grab, offset)) = grab {
        drag_handle(editor, &text, grab, press.pos - offset);
        return;
    }
    if !dragging || press.touch {
        return;
    }
    let index = text_index_at(&text, press.pos);
    let position = editor.borrow().core.position(index);
    command(editor, EditorCommand::Drag(position));
}

fn drag_handle(editor: &Handle, text: &NodeRef, grab: Grab, target: Pos2) {
    let node = text.get();
    let Some((rect, line)) = with_document(|document| {
        document
            .node_rect(node)
            .map(|rect| (rect, document.text_caret_middle(node, 0).y))
    }) else {
        return;
    };
    let beyond = if target.x < rect.left() {
        Some(LRDirection::Left)
    } else if target.x > rect.right() {
        Some(LRDirection::Right)
    } else {
        None
    };
    let index = text_index_at(text, pos2(target.x.clamp(rect.left(), rect.right()), line));
    let (position, set_autoscroll) = {
        let state = editor.borrow();
        (state.core.position(index), state.set_autoscroll.clone())
    };
    set_autoscroll.set(beyond.is_some());
    command(
        editor,
        match grab {
            Grab::Selection(fixed) => EditorCommand::DragSelectionHandle { fixed, position },
            Grab::Caret => EditorCommand::SetSelection {
                anchor: position,
                focus: position,
            },
        },
    );
    let Some(direction) = beyond else {
        return;
    };
    {
        let mut state = editor.borrow_mut();
        if state.last_step.elapsed() < AUTOSCROLL_STEP {
            return;
        }
        state.last_step = Instant::now();
    }
    command(
        editor,
        EditorCommand::MoveCursorLeftRight {
            mode: match grab {
                Grab::Selection(_) => MoveMode::Select,
                Grab::Caret => MoveMode::Move,
            },
            direction,
            stop: CursorLeftRightStop::UnicodeGraphemeCluster,
        },
    );
}

fn open_menu(editor: &Handle, at: Pos2) {
    let (set_actions, set_menu, selected) = {
        let state = editor.borrow();
        (
            state.set_menu_actions.clone(),
            state.set_menu.clone(),
            !selection(&state.core).is_empty(),
        )
    };
    set_actions.set(if selected {
        vec![
            MenuAction::Copy,
            MenuAction::Cut,
            MenuAction::Paste,
            MenuAction::SelectAll,
        ]
    } else {
        vec![MenuAction::Paste, MenuAction::SelectAll]
    });
    set_menu.set(Some(at));
}

fn close_menu(editor: &Handle) {
    let set_menu = editor.borrow().set_menu.clone();
    set_menu.set(None);
}

fn menu_action(editor: &Handle, action: MenuAction) {
    match action {
        MenuAction::Copy => copy(editor, CopyMode::Copy),
        MenuAction::Cut => copy(editor, CopyMode::Cut),
        MenuAction::Paste => {
            editor.borrow_mut().caret_handle = false;
            with_document(|document| document.request_paste());
            show(editor);
        }
        MenuAction::SelectAll => command(editor, EditorCommand::SelectAll),
    }
    close_menu(editor);
}

fn copy(editor: &Handle, mode: CopyMode) {
    let copied = {
        let mut state = editor.borrow_mut();
        if selection(&state.core).is_empty() {
            None
        } else {
            Some(state.core.copy_utf8(mode))
        }
    };
    if let Some(copied) = copied {
        copy_text(copied);
        show(editor);
    }
}

fn key(editor: &Handle, press: KeyPress) -> bool {
    if !press.pressed {
        return matches!(press.key, Key::Enter | Key::Space);
    }
    let modifiers = press.modifiers;
    let by_word = modifiers.ctrl || modifiers.alt;
    let stop = if by_word {
        CursorLeftRightStop::Word
    } else {
        CursorLeftRightStop::UnicodeGraphemeCluster
    };
    match press.key {
        Key::ArrowLeft | Key::ArrowRight | Key::Home | Key::End => command(
            editor,
            EditorCommand::MoveCursorLeftRight {
                mode: if modifiers.shift {
                    MoveMode::Select
                } else {
                    MoveMode::Move
                },
                direction: if matches!(press.key, Key::ArrowLeft | Key::Home) {
                    LRDirection::Left
                } else {
                    LRDirection::Right
                },
                stop: if matches!(press.key, Key::Home | Key::End) {
                    CursorLeftRightStop::Line
                } else {
                    stop
                },
            },
        ),
        Key::Backspace | Key::Delete => {
            editor.borrow_mut().caret_handle = false;
            command(
                editor,
                EditorCommand::Delete {
                    direction: if press.key == Key::Backspace {
                        LRDirection::Left
                    } else {
                        LRDirection::Right
                    },
                    stop,
                },
            );
        }
        Key::C if modifiers.ctrl && !modifiers.alt => copy(editor, CopyMode::Copy),
        Key::X if modifiers.ctrl && !modifiers.alt => copy(editor, CopyMode::Cut),
        Key::A if modifiers.ctrl => command(editor, EditorCommand::SelectAll),
        Key::Z if modifiers.ctrl && modifiers.shift => command(editor, EditorCommand::Redo),
        Key::Z if modifiers.ctrl => command(editor, EditorCommand::Undo),
        Key::Y if modifiers.ctrl => command(editor, EditorCommand::Redo),
        Key::Enter => submit(editor),
        Key::Space => {}
        _ => return false,
    }
    true
}

fn submit(editor: &Handle) {
    let (on_submit, value) = {
        let mut state = editor.borrow_mut();
        state.core.external_edit();
        (state.on_submit.clone(), text_of(&state.core))
    };
    on_submit.call(value);
}
