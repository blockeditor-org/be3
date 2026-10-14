use accesskit::Role;
use beui_core::base::overlay::Placement;
use beui_core::document::Document;
use beui_core::input::{Key, KeyPress};
use beui_core::layer::Dismiss;
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;

use crate as unstyled;
use crate::button::ButtonHandle;
use beui_view::reactive::{
    Action, Callback, Child, ChildScope, ChildValue, Children, ClickCallback, Interactive,
    IntoProp, List, Memo, NodeRef, Prop, ReadSignal, RenderFn, Run, Scope, Selector, Show, Stored,
    WriteSignal, action_disabled, action_glyph, action_label, clone, create_effect, create_memo,
    create_selector, create_signal, set_component_state, store, with_document,
};
use std::cell::RefCell;
use std::rc::Rc;

pub struct MenuRowHandle {
    pub label: Prop<String>,
    pub glyph: Prop<String>,
    pub detail: Prop<String>,
    pub disabled: Prop<bool>,
    pub danger: Prop<bool>,
    pub separated: Prop<bool>,
    pub has_submenu: Prop<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct MenuItem {
    label: Memo<String>,
    glyph: Memo<String>,
    detail: Memo<String>,
    disabled: Memo<bool>,
    danger: Memo<bool>,
    separated: Memo<bool>,
    test_id: Option<String>,
    action: Option<Action>,
    on_click: ClickCallback,
    children: Run<MenuItem>,
    scope: ChildScope,
}

impl ChildValue for MenuItem {
    fn adopt_scope(&mut self, scope: Scope) {
        self.scope.adopt(scope);
    }
}

beui_view::value_child_type!(MenuItem);

#[component]
pub fn MenuItem(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] danger: Prop<bool>,
    #[prop(default = false)] separated: Prop<bool>,
    row_test_id: Option<String>,
    action: Option<Action>,
    on_click: ClickCallback,
    children: Children<MenuItem>,
) -> MenuItem {
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let disabled = action_disabled(action.as_ref(), disabled);
    MenuItem {
        label: create_memo(move || label.get()),
        glyph: create_memo(move || glyph.get()),
        detail: create_memo(move || detail.get()),
        disabled: create_memo(move || disabled.get()),
        danger: create_memo(move || danger.get()),
        separated: create_memo(move || separated.get()),
        test_id: row_test_id,
        action,
        on_click,
        children: children.into_run(),
        scope: ChildScope::default(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Focus {
    Away,
    Root,
    Row(usize),
}

#[derive(Clone, Copy, Default)]
pub struct MenuParent(Option<Callback<()>>);

impl MenuParent {
    fn is_some(&self) -> bool {
        self.0.is_some()
    }

    fn leave(&self) {
        if let Some(leave) = self.0 {
            leave.call(());
        }
    }
}

struct Submenu {
    open: ReadSignal<bool>,
    set_open: WriteSignal<bool>,
    content: NodeRef,
}

impl Submenu {
    fn new() -> Self {
        let (open, set_open) = create_signal(false);
        Self {
            open,
            set_open,
            content: NodeRef::new(),
        }
    }
}

struct Row {
    button: NodeRef,
    item: Rc<MenuItem>,
    submenu: Submenu,
}

impl Row {
    fn has_children(&self) -> bool {
        !self.item.children.peek().is_empty()
    }
}

#[derive(Clone, Copy)]
struct State {
    rows: Stored<RefCell<Vec<Row>>>,
    root: NodeRef,
    focus: ReadSignal<Focus>,
    set_focus: WriteSignal<Focus>,
    on_select: Callback<Vec<usize>>,
}

#[component]
pub fn MenuList(
    items: Run<MenuItem>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<Child>>,
    #[prop(default = MenuParent::default())] parent: MenuParent,
    active: Prop<bool>,
    #[prop(default = false)] focus_first: bool,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let row = row.expect("menu_list requires a `row` builder");
    let panel = panel.expect("menu_list requires a `panel` builder");
    let entry = items.clone();
    let active_focus = active.map(move |active| match (active, entry.is_empty()) {
        (false, _) => Focus::Away,
        (true, true) => Focus::Root,
        (true, false) if focus_first => Focus::Row(0),
        (true, false) => Focus::Root,
    });
    let (focus, set_focus) = create_signal(active_focus.peek());
    create_effect(move || set_focus.set(active_focus.get()));
    let focused = create_selector(move || focus.get());
    let root_tab_stop = create_memo(move || !matches!(focus.get(), Focus::Row(_)));

    let state = State {
        rows: store(RefCell::default()),
        root: NodeRef::new(),
        focus,
        set_focus,
        on_select,
    };
    set_component_state(state);

    let blur_focus = focus;
    let rows_focused = focused;
    let lines = items.build_each(
        state.rows,
        |_, item| Row {
            button: NodeRef::new(),
            item,
            submenu: Submenu::new(),
        },
        move |index| {
            view! {
                <MenuRow
                    state={state}
                    index
                    focused={rows_focused}
                    row={row}
                    panel={panel}
                    parent={parent}
                />
            }
        },
    );
    view! {
        <List spacing=0.0>
            <Interactive
                focusable=true
                @node_ref={&state.root}
                tab_stop={root_tab_stop}
                focused={focused.memo(Focus::Root)}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && blur_focus.get_untracked() == Focus::Root {
                        set_focus.set(Focus::Away);
                    }
                }}
                on_key={move |press: KeyPress| root_key(state, press)}
            />
            <List spacing=2.0 children={lines} />
        </List>
    }
}

#[component]
fn MenuRow(
    state: State,
    index: usize,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    #[prop(default = MenuParent::default())] parent: MenuParent,
) -> NodeId {
    let (item, button, open, set_open, content_ref) = {
        let rows = state.rows.get();
        let rows = rows.borrow();
        let held = &rows[index];
        (
            held.item.clone(),
            held.button,
            held.submenu.open,
            held.submenu.set_open,
            held.submenu.content,
        )
    };
    let disabled = item.disabled;
    let (spoken, unavailable) = (item.label, item.disabled);
    let test_id = item.test_id.clone();
    let tagged = button;
    let action = item.action.clone();
    let clicked = item.on_click;
    let children = item.children.clone();
    let has_children = create_memo(clone!(children -> move || !children.is_empty()));
    let row_has_children = has_children;
    let content = {
        let (row, item) = (row, item);
        move |handle: ButtonHandle| {
            let hovered = handle.hovered;
            let mut was_hovered = false;
            create_effect(move || {
                let hovering = hovered.get();
                match (was_hovered, hovering) {
                    (_, true) => hover_row(state, index),
                    (true, false) => leave_row(state, index),
                    (false, false) => {}
                }
                was_hovered = hovering;
            });
            row.call(MenuRowHandle {
                label: item.label.into_prop(),
                glyph: item.glyph.into_prop(),
                detail: item.detail.into_prop(),
                disabled: item.disabled.into_prop(),
                danger: item.danger.into_prop(),
                separated: item.separated.into_prop(),
                has_submenu: row_has_children.into_prop(),
                hovered: handle.hovered,
                active: handle.active,
                focused: handle.focused,
            })
        }
    };
    let (dismiss, submenu_open, leave_open) = (set_open, open, set_open);

    let built = view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&button
                label={spoken}
                role=Role::MenuItem
                disabled={unavailable}
                tab_stop={focused.memo(Focus::Row(index))}
                focused={focused.memo(Focus::Row(index))}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && state.focus.get_untracked() == Focus::Row(index) {
                        state.set_focus.set(Focus::Away);
                    }
                }}
                content
                on_click={move || {
                    if disabled.get_untracked() {
                        return;
                    }
                    if !open_submenu(state, index) {
                        select(state, vec![index]);
                        if let Some(action) = &action {
                            action.run();
                        }
                        clicked.call();
                    }
                }}
                on_key={move |press: KeyPress| key(state, index, parent, press)}
            />
            <Show condition={has_children}>
                {move || {
                let (dismiss, submenu_open, children) =
                    (dismiss, submenu_open, children.clone());
                let (row, panel, content_ref) = (row, panel, content_ref);
                let leave = MenuParent(Some(Callback::new(move |()| {
                    leave_open.set(false);
                    state.set_focus.set(Focus::Row(index));
                })));
                view! {
                    <Overlay
                        anchor=button
                        placement=Placement::RightStart
                        open={submenu_open}
                        on_dismiss={move |_: Dismiss| dismiss.set(false)}
                        content={move || panel.call(view! {
                            <MenuList
                                @node_ref={&content_ref}
                                items={children}
                                row
                                panel={panel}
                                parent={leave}
                                active={submenu_open}
                                focus_first=true
                                on_select={move |mut path: Vec<usize>| {
                                    path.insert(0, index);
                                    select(state, path);
                                }}
                            />
                        })}
                    />
                }
                }}
            </Show>
        </List>
    };
    if let Some(test_id) = test_id {
        with_document(|document| document.set_test_id(tagged.get(), test_id));
    }
    built
}

fn select(state: State, path: Vec<usize>) {
    state.on_select.call(path);
}

pub fn menu_list_len(document: &Document, menu: NodeId) -> usize {
    document
        .component_state::<State>(menu)
        .rows
        .get()
        .borrow()
        .len()
}

pub fn menu_list_row_button(document: &Document, menu: NodeId, index: usize) -> NodeId {
    document.component_state::<State>(menu).rows.get().borrow()[index]
        .button
        .get()
}

pub fn menu_list_row_submenu_content(
    document: &Document,
    menu: NodeId,
    index: usize,
) -> Option<NodeId> {
    document.component_state::<State>(menu).rows.get().borrow()[index]
        .submenu
        .content
        .try_get()
}

pub fn menu_list_root_focusable(document: &Document, menu: NodeId) -> NodeId {
    document.component_state::<State>(menu).root.get()
}

fn open_submenu(state: State, index: usize) -> bool {
    let rows = state.rows.get();
    let rows = rows.borrow();
    let Some(row) = rows.get(index) else {
        return false;
    };
    if !row.has_children() {
        return false;
    }
    let opening = (!row.item.disabled.get_untracked()).then_some(row.submenu.set_open);
    drop(rows);
    if let Some(set_open) = opening {
        set_open.set(true);
    }
    true
}

fn close_sibling_submenus(state: State, index: usize) {
    let rows = state.rows.get();
    let rows = rows.borrow();
    let closing: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != index)
        .map(|(_, row)| row.submenu.set_open)
        .collect();
    drop(rows);
    for set_open in closing {
        set_open.set(false);
    }
}

fn hover_row(state: State, index: usize) {
    close_sibling_submenus(state, index);
    if open_submenu(state, index) {
        return;
    }
    focus_row(state, index);
}

fn leave_row(state: State, index: usize) {
    let open = state
        .rows
        .get()
        .borrow()
        .get(index)
        .is_some_and(|row| row.submenu.open.get_untracked());
    if !open && state.focus.get_untracked() == Focus::Row(index) {
        state.set_focus.set(Focus::Root);
    }
}

fn focus_row(state: State, index: usize) {
    close_sibling_submenus(state, index);
    state.set_focus.set(Focus::Row(index));
}

fn row_count(state: State) -> usize {
    state.rows.get().borrow().len()
}

fn has_submenu(state: State, index: usize) -> bool {
    state
        .rows
        .get()
        .borrow()
        .get(index)
        .is_some_and(Row::has_children)
}

fn root_key(state: State, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = row_count(state);
    if count == 0 {
        return false;
    }
    match press.key {
        Key::ArrowDown | Key::Home => {
            if press.pressed {
                focus_row(state, 0);
            }
            true
        }
        Key::ArrowUp | Key::End => {
            if press.pressed {
                focus_row(state, count - 1);
            }
            true
        }
        _ => false,
    }
}

fn key(state: State, index: usize, parent: MenuParent, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let count = row_count(state);
    match press.key {
        Key::ArrowUp => {
            if press.pressed {
                focus_row(state, (index + count - 1) % count);
            }
            true
        }
        Key::ArrowDown => {
            if press.pressed {
                focus_row(state, (index + 1) % count);
            }
            true
        }
        Key::Home => {
            if press.pressed {
                focus_row(state, 0);
            }
            true
        }
        Key::End => {
            if press.pressed {
                focus_row(state, count - 1);
            }
            true
        }
        Key::ArrowRight if has_submenu(state, index) => {
            if press.pressed {
                open_submenu(state, index);
            }
            true
        }
        Key::ArrowLeft if parent.is_some() => {
            if press.pressed {
                parent.leave();
            }
            true
        }
        _ => false,
    }
}
