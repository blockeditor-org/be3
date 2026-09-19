use crate::base::overlay::{Overlay, Placement};
use crate::document::Document;
use crate::input::{Key, KeyPress};
use crate::node::NodeId;
use beui_macros::{component, view};

use crate::reactive::{
    Callback, Child, ChildScope, ChildValue, Children, Focusable, IntoProp, List, Memo, NodeRef,
    Prop, ReadSignal, RenderFn, Run, Scope, Selector, Show, WriteSignal, clone, create_effect,
    create_memo, create_selector, create_signal, intrinsic, set_component_state,
};
use crate::unstyled;
use crate::unstyled::button::ButtonHandle;
use std::cell::RefCell;
use std::rc::Rc;

pub struct MenuRowHandle {
    pub label: Prop<String>,
    pub disabled: Prop<bool>,
    pub has_submenu: Prop<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

pub struct MenuItem {
    label: Memo<String>,
    disabled: Memo<bool>,
    children: Run<MenuItem>,
    scope: ChildScope,
}

impl ChildValue for MenuItem {
    fn anchor(&self) -> Option<NodeId> {
        None
    }

    fn adopt_scope(&mut self, scope: Scope) {
        self.scope.adopt(scope);
    }
}

crate::value_child_type!(MenuItem);

#[component]
pub fn MenuItem(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    children: Children<MenuItem>,
) -> MenuItem {
    MenuItem {
        label: create_memo(move || label.get()),
        disabled: create_memo(move || disabled.get()),
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

#[derive(Clone, Default)]
pub(crate) struct MenuParent(Option<Rc<dyn Fn()>>);

impl MenuParent {
    fn is_some(&self) -> bool {
        self.0.is_some()
    }

    fn leave(&self) {
        if let Some(leave) = &self.0 {
            leave();
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

struct State {
    rows: RefCell<Vec<Row>>,
    root: NodeRef,
    focus: ReadSignal<Focus>,
    set_focus: WriteSignal<Focus>,
    on_select: Callback<Vec<usize>>,
}

type Handle = Rc<State>;

#[component]
pub(crate) fn MenuList(
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
    create_effect(clone!(set_focus -> move || set_focus.set(active_focus.get())));
    let focused = create_selector(clone!(focus -> move || focus.get()));
    let root_tab_stop = create_memo(clone!(focus -> move || !matches!(focus.get(), Focus::Row(_))));

    let state: Handle = Rc::new(State {
        rows: RefCell::new(Vec::new()),
        root: NodeRef::new(),
        focus: focus.clone(),
        set_focus: set_focus.clone(),
        on_select,
    });
    set_component_state(state.clone());

    let (rows_state, key_state, blur_focus) = (state.clone(), state.clone(), focus);
    let rows_focused = focused.clone();
    let lines = items.build(move |items: Vec<Rc<MenuItem>>| {
        let state = rows_state.clone();
        *state.rows.borrow_mut() = items
            .into_iter()
            .map(|item| Row {
                button: NodeRef::new(),
                item,
                submenu: Submenu::new(),
            })
            .collect();
        let count = state.rows.borrow().len();
        (0..count)
            .map(|index| {
                intrinsic(view! {
                    <MenuRow
                        state={state.clone()}
                        index
                        focused={rows_focused.clone()}
                        row={row.clone()}
                        panel={panel.clone()}
                        parent={parent.clone()}
                    />
                })
            })
            .collect()
    });
    view! {
        <List spacing=0.0>
            <Focusable
                @node_ref={&state.root}
                tab_stop={root_tab_stop}
                focused={focused.memo(Focus::Root)}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && blur_focus.get_untracked() == Focus::Root {
                        set_focus.set(Focus::Away);
                    }
                }}
                on_key={move |press: KeyPress| root_key(&key_state, press)}
            />
            <List spacing=2.0 children={lines} />
        </List>
    }
}

#[component]
fn MenuRow(
    state: Handle,
    index: usize,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
    #[prop(default = MenuParent::default())] parent: MenuParent,
) -> NodeId {
    let (item, button, open, set_open, content_ref) = {
        let rows = state.rows.borrow();
        let held = &rows[index];
        (
            held.item.clone(),
            held.button.clone(),
            held.submenu.open.clone(),
            held.submenu.set_open.clone(),
            held.submenu.content.clone(),
        )
    };
    let disabled = item.disabled.clone();
    let children = item.children.clone();
    let has_children = create_memo(clone!(children -> move || !children.is_empty()));
    let row_has_children = has_children.clone();
    let content = {
        let (row, state, item) = (row.clone(), state.clone(), item);
        move |handle: ButtonHandle| {
            let hovered = handle.hovered.clone();
            create_effect(move || {
                if hovered.get() {
                    hover_row(&state, index);
                }
            });
            row.call(MenuRowHandle {
                label: item.label.clone().into_prop(),
                disabled: item.disabled.clone().into_prop(),
                has_submenu: row_has_children.into_prop(),
                hovered: handle.hovered,
                focused: handle.focused,
            })
        }
    };
    let (click_state, key_state, blur_state, select_state, leave_state) = (
        state.clone(),
        state.clone(),
        state.clone(),
        state.clone(),
        state,
    );
    let (dismiss, submenu_open, leave_open) = (set_open.clone(), open.clone(), set_open);

    view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&button
                tab_stop={focused.memo(Focus::Row(index))}
                focused={focused.memo(Focus::Row(index))}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && blur_state.focus.get_untracked() == Focus::Row(index) {
                        blur_state.set_focus.set(Focus::Away);
                    }
                }}
                content
                on_click={move || {
                    if disabled.get_untracked() {
                        return;
                    }
                    if !open_submenu(&click_state, index) {
                        select(&click_state, vec![index]);
                    }
                }}
                on_key={move |press: KeyPress| key(&key_state, index, parent.clone(), press)}
            />
            <Show condition={has_children}>
                {move || {
                let leave = MenuParent(Some(Rc::new(move || {
                    leave_open.set(false);
                    leave_state.set_focus.set(Focus::Row(index));
                })));
                intrinsic(view! {
                    <Overlay
                        anchor=&button
                        placement=Placement::RightStart
                        open={submenu_open.clone()}
                        on_dismiss={move || dismiss.set(false)}
                    >
                        {panel.call(view! {
                            <MenuList
                                @node_ref={&content_ref}
                                items={children}
                                row
                                panel={panel.clone()}
                                parent={leave}
                                active={submenu_open}
                                focus_first=true
                                on_select={move |mut path: Vec<usize>| {
                                    path.insert(0, index);
                                    select(&select_state, path);
                                }}
                            />
                        })}
                    </Overlay>
                })
                }}
            </Show>
        </List>
    }
}

fn select(state: &State, path: Vec<usize>) {
    state.on_select.call(path);
}

pub fn menu_list_len(document: &Document, menu: NodeId) -> usize {
    document.component_state::<Handle>(menu).rows.borrow().len()
}

pub fn menu_list_row_button(document: &Document, menu: NodeId, index: usize) -> NodeId {
    document.component_state::<Handle>(menu).rows.borrow()[index]
        .button
        .get()
}

pub fn menu_list_row_submenu_content(
    document: &Document,
    menu: NodeId,
    index: usize,
) -> Option<NodeId> {
    document.component_state::<Handle>(menu).rows.borrow()[index]
        .submenu
        .content
        .try_get()
}

pub fn menu_list_root_focusable(document: &Document, menu: NodeId) -> NodeId {
    document.component_state::<Handle>(menu).root.get()
}

fn open_submenu(state: &State, index: usize) -> bool {
    let rows = state.rows.borrow();
    let Some(row) = rows.get(index) else {
        return false;
    };
    if !row.has_children() {
        return false;
    }
    let opening = (!row.item.disabled.get_untracked()).then(|| row.submenu.set_open.clone());
    drop(rows);
    if let Some(set_open) = opening {
        set_open.set(true);
    }
    true
}

fn close_sibling_submenus(state: &State, index: usize) {
    let rows = state.rows.borrow();
    let closing: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(other, _)| *other != index)
        .map(|(_, row)| row.submenu.set_open.clone())
        .collect();
    drop(rows);
    for set_open in closing {
        set_open.set(false);
    }
}

fn hover_row(state: &State, index: usize) {
    close_sibling_submenus(state, index);
    if open_submenu(state, index) {
        return;
    }
    focus_row(state, index);
}

fn focus_row(state: &State, index: usize) {
    close_sibling_submenus(state, index);
    state.set_focus.set(Focus::Row(index));
}

fn row_count(state: &State) -> usize {
    state.rows.borrow().len()
}

fn has_submenu(state: &State, index: usize) -> bool {
    state
        .rows
        .borrow()
        .get(index)
        .is_some_and(Row::has_children)
}

fn root_key(state: &State, press: KeyPress) -> bool {
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

fn key(state: &State, index: usize, parent: MenuParent, press: KeyPress) -> bool {
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
