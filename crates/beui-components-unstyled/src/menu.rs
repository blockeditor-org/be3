use accesskit::{Node, Role, Toggled};
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::NodeId;
use beui_view::types::Dismiss;
use beui_view::types::Placement;
use beui_view::types::Rect;
use beui_view::types::Track;
use beui_view::types::icons::Glyph;
use beui_view::types::{Key, KeyPress};

use crate as unstyled;
use crate::button::ButtonHandle;
use beui_view::primitives::*;
use beui_view::reactive::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuRowKind {
    Row,
    Caption,
    Tile,
    Back,
    Separator,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum MenuThen {
    #[default]
    Close,
    Back,
    Stay,
}

pub struct MenuRowHandle {
    pub kind: MenuRowKind,
    pub label: Prop<String>,
    pub lead: Prop<String>,
    pub glyph: Prop<Option<Glyph>>,
    pub glyph_color: Prop<Option<beui_view::types::Color>>,
    pub detail: Prop<String>,
    pub checked: Prop<Option<bool>>,
    pub radio: bool,
    pub glyphs: Prop<bool>,
    pub shortcut: Prop<String>,
    pub disabled: Prop<bool>,
    pub danger: Prop<bool>,
    pub separated: Prop<bool>,
    pub has_submenu: Prop<bool>,
    pub depth: Prop<Option<usize>>,
    pub expanded: Prop<Option<bool>>,
    pub toggle: ClickCallback,
    pub pointed: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Row,
    Caption,
    Grid(usize),
}

pub struct MenuItem {
    kind: Kind,
    pub(crate) label: Memo<String>,
    lead: Memo<String>,
    pub(crate) glyph: Memo<Option<Glyph>>,
    glyph_color: Memo<Option<beui_view::types::Color>>,
    detail: Memo<String>,
    checked: Memo<Option<bool>>,
    radio: bool,
    shortcut: Memo<String>,
    pub(crate) disabled: Memo<bool>,
    danger: Memo<bool>,
    separated: Memo<bool>,
    depth: Memo<Option<usize>>,
    expanded: Binding<Option<bool>>,
    page: bool,
    then: MenuThen,
    test_id: Option<String>,
    pub(crate) action: Option<Action>,
    pub(crate) on_click: ClickCallback,
    pub(crate) children: Run<MenuItem>,
}

beui_view::value_child_type!(MenuItem);

impl MenuItem {
    fn is_row(&self) -> bool {
        self.kind == Kind::Row
    }

    fn opens(&self) -> bool {
        self.is_row() && !self.children.get_untracked().is_empty()
    }

    fn spoken(&self) -> Memo<String> {
        let (lead, label) = (self.lead, self.label);
        create_memo(move || {
            let lead = lead.get();
            match lead.is_empty() {
                true => label.get(),
                false => format!("{lead} {}", label.get()),
            }
        })
    }

    fn toggle(&self) {
        if let Some(expanded) = self.expanded.get_untracked() {
            self.expanded.set(Some(!expanded));
        }
    }
}

#[component]
pub fn MenuItem(
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] lead: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = None)] glyph_color: Prop<Option<beui_view::types::Color>>,
    #[prop(default = String::new())] detail: Prop<String>,
    #[prop(default = None)] checked: Prop<Option<bool>>,
    #[prop(default = false)] radio: bool,
    #[prop(default = String::new())] shortcut: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] danger: Prop<bool>,
    #[prop(default = false)] separated: Prop<bool>,
    #[prop(default = None)] depth: Prop<Option<usize>>,
    #[prop(default = None)] expanded: Binding<Option<bool>>,
    #[prop(default = false)] page: bool,
    #[prop(default = MenuThen::Close)] then: MenuThen,
    row_test_id: Option<String>,
    action: Option<Action>,
    on_click: ClickCallback,
    children: Children<MenuItem>,
) -> MenuItem {
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let disabled = action_disabled(action.as_ref(), disabled);
    let checked = action_checked(action.as_ref(), checked);
    let radio = radio || action.as_ref().is_some_and(Action::is_radio);
    let shortcut = match (&shortcut, action.as_ref().and_then(Action::shortcut_label)) {
        (Prop::Static(set), Some(chord)) if set.is_empty() => Prop::Static(chord),
        _ => shortcut,
    };
    MenuItem {
        kind: Kind::Row,
        label: create_memo(move || label.get()),
        lead: create_memo(move || lead.get()),
        glyph: create_memo(move || glyph.get()),
        glyph_color: create_memo(move || glyph_color.get()),
        detail: create_memo(move || detail.get()),
        checked: create_memo(move || checked.get()),
        radio,
        shortcut: create_memo(move || shortcut.get()),
        disabled: create_memo(move || disabled.get()),
        danger: create_memo(move || danger.get()),
        separated: create_memo(move || separated.get()),
        depth: create_memo(move || depth.get()),
        expanded,
        page,
        then,
        test_id: row_test_id,
        action,
        on_click,
        children: children.into_run(),
    }
}

#[component]
pub fn MenuCaption(
    label: Prop<String>,
    #[prop(default = false)] separated: Prop<bool>,
) -> MenuItem {
    MenuItem {
        kind: Kind::Caption,
        separated: create_memo(move || separated.get()),
        ..plain(label, Run::default())
    }
}

#[component]
pub fn MenuGrid(
    #[prop(default = 2)] columns: usize,
    #[prop(default = false)] separated: Prop<bool>,
    row_test_id: Option<String>,
    children: Children<MenuItem>,
) -> MenuItem {
    MenuItem {
        kind: Kind::Grid(columns.max(1)),
        separated: create_memo(move || separated.get()),
        test_id: row_test_id,
        ..plain(Prop::Static(String::new()), children.into_run())
    }
}

fn plain(label: Prop<String>, children: Run<MenuItem>) -> MenuItem {
    MenuItem {
        kind: Kind::Row,
        label: create_memo(move || label.get()),
        lead: create_memo(String::new),
        glyph: create_memo(|| None),
        glyph_color: create_memo(|| None),
        detail: create_memo(String::new),
        checked: create_memo(|| None),
        radio: false,
        shortcut: create_memo(String::new),
        disabled: create_memo(|| false),
        danger: create_memo(|| false),
        separated: create_memo(|| false),
        depth: create_memo(|| None),
        expanded: Binding::new(None, Callback::empty()),
        page: false,
        then: MenuThen::Close,
        test_id: None,
        action: None,
        on_click: ClickCallback::default(),
        children,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Focus {
    Away,
    Root,
    Back,
    Row(usize),
    Tile(usize, usize),
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
    open: Signal<bool>,
    content: NodeRef,
}

impl Submenu {
    fn new() -> Self {
        let open = create_signal(false);
        Self {
            open,
            content: NodeRef::new(),
        }
    }
}

struct Tile {
    button: NodeRef,
}

struct Row {
    button: NodeRef,
    item: Rc<MenuItem>,
    submenu: Submenu,
    tiles: Stored<RefCell<Vec<Tile>>>,
}

impl Row {
    fn has_children(&self) -> bool {
        self.item.opens()
    }
}

#[derive(Clone, Copy)]
struct State {
    rows: Stored<RefCell<Vec<Row>>>,
    items: Stored<Run<MenuItem>>,
    root: NodeRef,
    back: NodeRef,
    page_content: NodeRef,
    focus: Signal<Focus>,
    page: Signal<Option<usize>>,
    parent: MenuParent,
    titled: bool,
    pages: bool,
    glyphs: Memo<bool>,
    spacing: f32,
    bounds: Memo<Option<Rect>>,
    on_select: Callback<(Vec<usize>, MenuThen)>,
}

#[component]
pub(crate) fn MenuList(
    #[prop(default = ComponentRef::new())] api: ComponentRef<MenuListApi>,
    items: Run<MenuItem>,
    row: Option<RenderFn<MenuRowHandle>>,
    panel: Option<RenderFn<Child>>,
    spacing: f32,
    #[prop(default = MenuParent::default())] parent: MenuParent,
    #[prop(default = None)] title: Option<Memo<String>>,
    #[prop(default = String::new())] title_test_id: String,
    active: Prop<bool>,
    #[prop(default = false)] focus_first: bool,
    #[prop(default = false)] pages: bool,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    on_select: Callback<(Vec<usize>, MenuThen)>,
) -> NodeId {
    let row = row.expect("menu_list requires a `row` builder");
    let panel = panel.expect("menu_list requires a `panel` builder");
    let focus = create_signal(Focus::Away);
    let page = create_signal(None::<usize>);
    let state = State {
        rows: store(RefCell::default()),
        items: store(items.clone()),
        root: NodeRef::new(),
        back: NodeRef::new(),
        page_content: NodeRef::new(),
        focus,
        page,
        parent,
        titled: title.is_some(),
        pages,
        glyphs: {
            let items = items.clone();
            create_memo(move || {
                title.is_some()
                    || items.items().iter().any(|item| {
                        item.is_row()
                            && (item.glyph.get().is_some() || item.checked.get().is_some())
                    })
            })
        },
        spacing,
        bounds: create_memo(move || bounds.get()),
        on_select,
    };
    api.fill(MenuListApi(state));
    let active_focus = active.map(move |active| match active {
        false => Focus::Away,
        true => first_focus(state, focus_first),
    });
    create_effect(move || {
        let focus_now = active_focus.get();
        if focus_now == Focus::Away {
            page.set(None);
        }
        focus.set(focus_now);
    });
    let focused = create_selector(move || focus.get());
    let root_tab_stop = create_memo(move || {
        !matches!(focus.get(), Focus::Row(_) | Focus::Tile(..) | Focus::Back)
            && page.get().is_none()
    });
    let showing_own = create_memo(move || page.get().is_none());
    let paged = create_memo(move || page.get().is_some());

    let lines = move || {
        items.build_each(
            state.rows,
            |_, item| Row {
                button: NodeRef::new(),
                item,
                submenu: Submenu::new(),
                tiles: store(RefCell::default()),
            },
            move |index| {
                let kind = state.rows.value().borrow()[index].item.kind;
                match kind {
                    Kind::Row => view! {
                        <MenuRow state={state} index focused={focused} row={row} panel={panel} />
                    },
                    Kind::Caption => view! {
                        <MenuCaptionRow state={state} index row={row} />
                    },
                    Kind::Grid(columns) => view! {
                        <MenuGridRow state={state} index columns focused={focused} row={row} />
                    },
                }
            },
        )
    };
    let title = title.unwrap_or_else(|| create_memo(String::new));
    view! {
        <List spacing=0.0>
            <Interactive
                focusable=true
                @node_ref={&state.root}
                tab_stop={root_tab_stop}
                focused={focused.memo(Focus::Root)}
                on_focus_change={move |has_focus: bool| {
                    if !has_focus && focus.get_untracked() == Focus::Root {
                        focus.set(Focus::Away);
                    }
                }}
                on_key={move |press: KeyPress| key(state, Focus::Root, press)}
            />
            <Show condition={showing_own}>|| {
                let title_test_id = title_test_id.clone();
                let lines = lines();
                view! {
                    <List spacing={state.spacing}>
                        <Show condition={state.titled}>
                            <MenuBackRow
                                state={state}
                                title
                                test_id={title_test_id.clone()}
                                focused={focused}
                                row={row}
                            />
                        </Show>
                        {lines}
                    </List>
                }
            }</Show>
            <Show condition={paged}>
                <MenuPage state={state} row={row} panel={panel} />
            </Show>
        </List>
    }
}

#[component]
fn MenuPage(state: State, row: RenderFn<MenuRowHandle>, panel: RenderFn<Child>) -> NodeId {
    let index = state.page.get_untracked().unwrap_or_default();
    let item = state.rows.value().borrow()[index].item.clone();
    let leave = MenuParent(Some(Callback::new(move |()| {
        state.page.set(None);
        state.focus.set(Focus::Row(index));
    })));
    let title_test_id = item
        .test_id
        .as_ref()
        .map_or_else(String::new, |id| format!("{id}.back"));
    view! {
        <MenuList
            @node_ref={&state.page_content}
            items={item.children.clone()}
            row
            panel
            spacing={state.spacing}
            parent={leave}
            title={Some(item.spoken())}
            title_test_id
            active=true
            focus_first=true
            pages={state.pages}
            bounds={state.bounds}
            on_select={move |(mut path, then): (Vec<usize>, MenuThen)| {
                path.insert(0, index);
                state.on_select.call((path, then));
            }}
        />
    }
}

fn quiet() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

#[component]
fn MenuBackRow(
    state: State,
    title: Memo<String>,
    test_id: String,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
) -> NodeId {
    let content = move |handle: ButtonHandle| {
        watch_hover(state, Focus::Back, handle.pointed);
        row.call(MenuRowHandle {
            kind: MenuRowKind::Back,
            pointed: handle.pointed,
            active: handle.active,
            focused: handle.focused,
            ..quiet_handle(MenuRowKind::Back, title.into_prop())
        })
    };
    let built = view! {
        <unstyled::Button
            @node_ref={&state.back}
            label={title}
            role=Role::MenuItem
            tab_stop={focused.memo(Focus::Back)}
            focused={focused.memo(Focus::Back)}
            on_focus_change={move |has_focus: bool| lose_focus(state, Focus::Back, has_focus)}
            content
            on_click={move || state.parent.leave()}
            on_key={move |press: KeyPress| key(state, Focus::Back, press)}
        />
    };
    if !test_id.is_empty() {
        bind_test_id(built, Prop::Static(test_id));
    }
    built
}

fn quiet_handle(kind: MenuRowKind, label: Prop<String>) -> MenuRowHandle {
    MenuRowHandle {
        kind,
        label,
        lead: Prop::Static(String::new()),
        glyph: Prop::Static(None),
        glyph_color: Prop::Static(None),
        detail: Prop::Static(String::new()),
        checked: Prop::Static(None),
        radio: false,
        glyphs: Prop::Static(false),
        shortcut: Prop::Static(String::new()),
        disabled: Prop::Static(false),
        danger: Prop::Static(false),
        separated: Prop::Static(false),
        has_submenu: Prop::Static(false),
        depth: Prop::Static(None),
        expanded: Prop::Static(None),
        toggle: ClickCallback::default(),
        pointed: quiet(),
        active: quiet(),
        focused: quiet(),
    }
}

fn item_handle(kind: MenuRowKind, item: &Rc<MenuItem>, handle: &ButtonHandle) -> MenuRowHandle {
    let toggled = item.clone();
    MenuRowHandle {
        kind,
        label: item.label.into_prop(),
        lead: item.lead.into_prop(),
        glyph: item.glyph.into_prop(),
        glyph_color: item.glyph_color.into_prop(),
        detail: item.detail.into_prop(),
        checked: item.checked.into_prop(),
        radio: item.radio,
        glyphs: Prop::Static(false),
        shortcut: item.shortcut.into_prop(),
        disabled: item.disabled.into_prop(),
        danger: item.danger.into_prop(),
        separated: item.separated.into_prop(),
        has_submenu: Prop::Static(false),
        depth: item.depth.into_prop(),
        expanded: item.expanded.into_prop(),
        toggle: ClickCallback::new(move || toggled.toggle()),
        pointed: handle.pointed,
        active: handle.active,
        focused: handle.focused,
    }
}

fn row_accessibility(item: &Rc<MenuItem>) -> Memo<Node> {
    let (checked, depth, expanded) = (item.checked, item.depth, item.expanded);
    let radio = item.radio;
    create_memo(move || {
        let mut node = match checked.get() {
            Some(on) => {
                let mut node = Node::new(match radio {
                    true => Role::MenuItemRadio,
                    false => Role::MenuItemCheckBox,
                });
                node.set_toggled(Toggled::from(on));
                node
            }
            None => Node::new(Role::MenuItem),
        };
        if let Some(depth) = depth.get() {
            node.set_level(depth + 1);
        }
        if let Some(expanded) = expanded.get() {
            node.set_expanded(expanded);
        }
        node
    })
}

#[component]
fn MenuCaptionRow(state: State, index: usize, row: RenderFn<MenuRowHandle>) -> NodeId {
    let item = state.rows.value().borrow()[index].item.clone();
    row.call(MenuRowHandle {
        separated: item.separated.into_prop(),
        ..quiet_handle(MenuRowKind::Caption, item.label.into_prop())
    })
}

#[component]
fn MenuGridRow(
    state: State,
    index: usize,
    columns: usize,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
) -> NodeId {
    let (item, tiles) = {
        let rows = state.rows.value();
        let rows = rows.borrow();
        (rows[index].item.clone(), rows[index].tiles)
    };
    let separated = item.separated;
    let cells = item.children.build_each(
        tiles,
        |_, _| Tile {
            button: NodeRef::new(),
        },
        move |tile| {
            view! {
                <MenuTile state={state} index tile focused={focused} row={row} />
            }
        },
    );
    let built = view! {
        <List spacing={state.spacing}>
            <Show condition={separated}>
                {row.call(quiet_handle(MenuRowKind::Separator, Prop::Static(String::new())))}
            </Show>
            <Grid
                columns={vec![Track::Fraction(1.0); columns]}
                column_spacing={state.spacing}
                row_spacing={state.spacing}
                children={cells}
            />
        </List>
    };
    if let Some(test_id) = item.test_id.clone() {
        bind_test_id(built, Prop::Static(test_id));
    }
    built
}

#[component]
fn MenuTile(
    state: State,
    index: usize,
    tile: usize,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
) -> NodeId {
    let (grid, button) = {
        let rows = state.rows.value();
        let rows = rows.borrow();
        let held = &rows[index];
        let button = held.tiles.value().borrow()[tile].button;
        (held.item.clone(), button)
    };
    let item = grid.children.get_untracked()[tile].clone();
    let at = Focus::Tile(index, tile);
    let test_id = item.test_id.clone();
    let face = item.clone();
    let content = move |handle: ButtonHandle| {
        watch_hover(state, at, handle.pointed);
        row.call(item_handle(MenuRowKind::Tile, &face, &handle))
    };
    let accessibility = row_accessibility(&item);
    let chosen = item.clone();
    let built = view! {
        <unstyled::Button
            @node_ref=&button
            label={item.label}
            accessibility
            disabled={item.disabled}
            tab_stop={focused.memo(at)}
            focused={focused.memo(at)}
            on_focus_change={move |has_focus: bool| lose_focus(state, at, has_focus)}
            content
            on_click={move || choose(state, vec![index, tile], &chosen)}
            on_key={move |press: KeyPress| key(state, at, press)}
        />
    };
    if let Some(test_id) = test_id {
        bind_test_id(built, Prop::Static(test_id));
    }
    built
}

#[component]
fn MenuRow(
    state: State,
    index: usize,
    focused: Selector<Focus>,
    row: RenderFn<MenuRowHandle>,
    panel: RenderFn<Child>,
) -> NodeId {
    let (item, button, open, content_ref) = {
        let rows = state.rows.value();
        let rows = rows.borrow();
        let held = &rows[index];
        (
            held.item.clone(),
            held.button,
            held.submenu.open,
            held.submenu.content,
        )
    };
    let at = Focus::Row(index);
    let (spoken, unavailable) = (item.spoken(), item.disabled);
    let test_id = item.test_id.clone();
    let children = item.children.clone();
    let cascades = !(item.page || state.pages);
    let has_children = create_memo({
        let children = children.clone();
        move || !children.is_empty()
    });
    let cascade = create_memo(move || cascades && has_children.get());
    let face = item.clone();
    let content = move |handle: ButtonHandle| {
        watch_hover(state, at, handle.pointed);
        row.call(MenuRowHandle {
            has_submenu: has_children.into_prop(),
            glyphs: state.glyphs.into_prop(),
            ..item_handle(MenuRowKind::Row, &face, &handle)
        })
    };
    let accessibility = row_accessibility(&item);
    let chosen = item.clone();

    let built = view! {
        <List spacing=0.0>
            <unstyled::Button
                @node_ref=&button
                label={spoken}
                accessibility
                disabled={unavailable}
                tab_stop={focused.memo(at)}
                focused={focused.memo(at)}
                on_focus_change={move |has_focus: bool| lose_focus(state, at, has_focus)}
                content
                on_click={move || {
                    if chosen.disabled.get_untracked() {
                        return;
                    }
                    if !open_submenu(state, index) {
                        choose(state, vec![index], &chosen);
                    }
                }}
                on_key={move |press: KeyPress| key(state, at, press)}
            />
            <Show condition={cascade}>|| {
                let children = children.clone();
                let leave = MenuParent(Some(Callback::new(move |()| {
                    open.set(false);
                    state.focus.set(Focus::Row(index));
                })));
                view! {
                    <Overlay
                        anchor=button
                        placement=Placement::RightStart
                        bounds={state.bounds}
                        open={open}
                        on_dismiss={move |_: Dismiss| open.set(false)}
                        content={move || {
                            let children = children.clone();
                            panel.call(view! {
                                <MenuList
                                    @node_ref={&content_ref}
                                    items={children}
                                    row
                                    panel={panel}
                                    spacing={state.spacing}
                                    parent={leave}
                                    active={open}
                                    focus_first=true
                                    bounds={state.bounds}
                                    on_select={move |(mut path, then): (Vec<usize>, MenuThen)| {
                                        path.insert(0, index);
                                        state.on_select.call((path, then));
                                    }}
                                />
                            })
                        }}
                    />
                }
            }</Show>
        </List>
    };
    if let Some(test_id) = test_id {
        bind_test_id(button.get(), Prop::Static(test_id));
    }
    built
}

fn choose(state: State, path: Vec<usize>, item: &MenuItem) {
    if item.disabled.get_untracked() {
        return;
    }
    state.on_select.call((path, item.then));
    if let Some(action) = &item.action {
        action.run();
    }
    item.on_click.call();
    if item.then == MenuThen::Back {
        state.parent.leave();
    }
}

fn watch_hover(state: State, at: Focus, pointed: ReadSignal<bool>) {
    let mut was_pointed = false;
    create_effect(move || {
        let pointing = pointed.get();
        match (was_pointed, pointing) {
            (_, true) => hover(state, at),
            (true, false) => leave(state, at),
            (false, false) => {}
        }
        was_pointed = pointing;
    });
}

fn lose_focus(state: State, at: Focus, has_focus: bool) {
    if !has_focus && state.focus.get_untracked() == at {
        state.focus.set(Focus::Away);
    }
}

fn open_submenu(state: State, index: usize) -> bool {
    let rows = state.rows.value();
    let rows = rows.borrow();
    let Some(row) = rows.get(index) else {
        return false;
    };
    if !row.has_children() {
        return false;
    }
    let enabled = !row.item.disabled.get_untracked();
    let (page, submenu) = (row.item.page || state.pages, row.submenu.open);
    drop(rows);
    match (enabled, page) {
        (false, _) => {}
        (true, true) => {
            state.focus.set(Focus::Away);
            state.page.set(Some(index));
        }
        (true, false) => submenu.set(true),
    }
    true
}

fn close_sibling_submenus(state: State, index: Option<usize>) {
    let rows = state.rows.value();
    let rows = rows.borrow();
    let closing: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(other, _)| Some(*other) != index)
        .map(|(_, row)| row.submenu.open)
        .collect();
    drop(rows);
    for submenu in closing {
        submenu.set(false);
    }
}

fn hover(state: State, at: Focus) {
    if let Focus::Row(index) = at {
        let cascades = state
            .rows
            .value()
            .borrow()
            .get(index)
            .is_some_and(|row| row.has_children() && !row.item.page && !state.pages);
        if cascades {
            close_sibling_submenus(state, Some(index));
            open_submenu(state, index);
            return;
        }
    }
    focus_on(state, at);
}

fn leave(state: State, at: Focus) {
    let open = match at {
        Focus::Row(index) => state
            .rows
            .value()
            .borrow()
            .get(index)
            .is_some_and(|row| row.submenu.open.get_untracked()),
        _ => false,
    };
    if !open && state.focus.get_untracked() == at {
        state.focus.set(Focus::Root);
    }
}

fn focus_on(state: State, at: Focus) {
    let index = match at {
        Focus::Row(index) => Some(index),
        _ => None,
    };
    close_sibling_submenus(state, index);
    state.focus.set(at);
}

struct Stops {
    stops: Vec<Focus>,
    grids: Vec<(usize, usize, usize)>,
}

impl Stops {
    fn of(state: State) -> Self {
        let items = state.items.with(|items| items.get_untracked());
        let mut stops = Vec::new();
        let mut grids = Vec::new();
        if state.titled {
            stops.push(Focus::Back);
        }
        for (index, item) in items.iter().enumerate() {
            match item.kind {
                Kind::Row => stops.push(Focus::Row(index)),
                Kind::Caption => {}
                Kind::Grid(columns) => {
                    let count = item.children.get_untracked().len();
                    grids.push((index, columns, count));
                    stops.extend((0..count).map(|tile| Focus::Tile(index, tile)));
                }
            }
        }
        Self { stops, grids }
    }

    fn grid(&self, index: usize) -> (usize, usize) {
        self.grids
            .iter()
            .find(|(grid, ..)| *grid == index)
            .map_or((1, 0), |(_, columns, count)| (*columns, *count))
    }

    fn position(&self, at: Focus) -> Option<usize> {
        self.stops.iter().position(|stop| *stop == at)
    }

    fn entering(&self, stop: Focus, from_below: bool) -> Focus {
        match stop {
            Focus::Tile(index, _) if from_below => {
                let (columns, count) = self.grid(index);
                Focus::Tile(index, (count.saturating_sub(1) / columns) * columns)
            }
            stop => stop,
        }
    }

    fn first(&self) -> Option<Focus> {
        self.stops.first().copied()
    }

    fn last(&self) -> Option<Focus> {
        self.stops.last().map(|stop| self.entering(*stop, true))
    }

    fn down(&self, at: Focus) -> Option<Focus> {
        let leaving = match at {
            Focus::Tile(index, tile) => {
                let (columns, count) = self.grid(index);
                if tile / columns < count.saturating_sub(1) / columns {
                    return Some(Focus::Tile(index, (tile + columns).min(count - 1)));
                }
                Focus::Tile(index, count - 1)
            }
            Focus::Away | Focus::Root => return self.first(),
            at => at,
        };
        let count = self.stops.len();
        let next = self.position(leaving).map_or(0, |at| (at + 1) % count);
        self.stops.get(next).copied()
    }

    fn up(&self, at: Focus) -> Option<Focus> {
        let leaving = match at {
            Focus::Tile(index, tile) => {
                let (columns, _) = self.grid(index);
                if tile >= columns {
                    return Some(Focus::Tile(index, tile - columns));
                }
                Focus::Tile(index, 0)
            }
            Focus::Away | Focus::Root => return self.last(),
            at => at,
        };
        let count = self.stops.len();
        let previous = self
            .position(leaving)
            .map_or(count.saturating_sub(1), |at| (at + count - 1) % count);
        let stop = self.stops.get(previous).copied()?;
        Some(self.entering(stop, true))
    }

    fn sideways(&self, at: Focus, forward: bool) -> Option<Focus> {
        let Focus::Tile(index, tile) = at else {
            return None;
        };
        let (columns, count) = self.grid(index);
        let column = tile % columns;
        match forward {
            true if column + 1 < columns && tile + 1 < count => Some(Focus::Tile(index, tile + 1)),
            false if column > 0 => Some(Focus::Tile(index, tile - 1)),
            _ => Some(at),
        }
    }
}

fn first_focus(state: State, focus_first: bool) -> Focus {
    if !focus_first {
        return Focus::Root;
    }
    let items = state.items.with(|items| items.get_untracked());
    let checked = items
        .iter()
        .position(|item| item.is_row() && item.checked.get_untracked() == Some(true));
    if let Some(index) = checked {
        return Focus::Row(index);
    }
    let stops = Stops::of(state);
    stops
        .stops
        .iter()
        .find(|stop| **stop != Focus::Back)
        .or(stops.stops.first())
        .copied()
        .unwrap_or(Focus::Root)
}

fn item_at(state: State, index: usize) -> Option<Rc<MenuItem>> {
    state
        .rows
        .value()
        .borrow()
        .get(index)
        .map(|row| row.item.clone())
}

fn parent_row(state: State, index: usize, depth: usize) -> Option<usize> {
    let rows = state.rows.value();
    let rows = rows.borrow();
    (0..index).rev().find(|above| {
        let item = &rows[*above].item;
        item.is_row()
            && item
                .depth
                .get_untracked()
                .is_some_and(|other| other < depth)
    })
}

fn right(state: State, at: Focus, stops: &Stops) -> Option<Focus> {
    let Focus::Row(index) = at else {
        return stops.sideways(at, true);
    };
    let item = item_at(state, index)?;
    if item.opens() {
        open_submenu(state, index);
        return None;
    }
    match item.expanded.get_untracked() {
        Some(false) => {
            item.expanded.set(Some(true));
            None
        }
        Some(true) => {
            let depth = item.depth.get_untracked().unwrap_or_default();
            let next = item_at(state, index + 1)?;
            let deeper = next.depth.get_untracked().is_some_and(|next| next > depth);
            deeper.then_some(Focus::Row(index + 1))
        }
        None => None,
    }
}

fn left(state: State, at: Focus, stops: &Stops) -> Option<Focus> {
    if let Focus::Tile(..) = at {
        return stops.sideways(at, false);
    }
    if let Focus::Row(index) = at
        && let Some(item) = item_at(state, index)
    {
        if item.expanded.get_untracked() == Some(true) {
            item.expanded.set(Some(false));
            return None;
        }
        let depth = item.depth.get_untracked().unwrap_or_default();
        if let Some(parent) = parent_row(state, index, depth).filter(|_| depth > 0) {
            return Some(Focus::Row(parent));
        }
    }
    state.parent.leave();
    None
}

fn turns(state: State, at: Focus, forward: bool) -> bool {
    match at {
        Focus::Tile(..) => true,
        Focus::Row(index) => item_at(state, index).is_some_and(|item| match forward {
            true => item.opens() || item.expanded.get_untracked().is_some(),
            false => {
                item.expanded.get_untracked() == Some(true)
                    || item.depth.get_untracked().is_some_and(|depth| depth > 0)
                    || state.parent.is_some()
            }
        }),
        _ => !forward && state.parent.is_some(),
    }
}

fn key(state: State, at: Focus, press: KeyPress) -> bool {
    if press.modifiers.ctrl || press.modifiers.alt {
        return false;
    }
    let stops = Stops::of(state);
    let handled = match press.key {
        Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End => !stops.stops.is_empty(),
        Key::ArrowRight => turns(state, at, true),
        Key::ArrowLeft => turns(state, at, false),
        Key::Escape => state.parent.is_some(),
        _ => false,
    };
    if !handled {
        return false;
    }
    if !press.pressed {
        return true;
    }
    let target = match press.key {
        Key::ArrowDown => stops.down(at),
        Key::ArrowUp => stops.up(at),
        Key::Home => stops.first(),
        Key::End => stops.last(),
        Key::ArrowRight => right(state, at, &stops),
        Key::ArrowLeft => left(state, at, &stops),
        _ => {
            state.parent.leave();
            None
        }
    };
    if let Some(target) = target {
        focus_on(state, target);
    }
    true
}

#[component]
pub fn MenuNodes(
    nodes: Prop<Vec<MenuNode>>,
    #[prop(default = String::new())] test_prefix: String,
) -> DynamicSegment<MenuItem> {
    let nodes = create_memo(move || nodes.get());
    let keys = create_memo(move || {
        nodes.with(|nodes| nodes.iter().filter_map(MenuNode::key).collect::<Vec<u64>>())
    });
    view! {
        <ForEach keys>|key: u64|
            <MenuNodeItem nodes={nodes} key test_prefix={test_prefix.clone()} />
        </ForEach>
    }
}

#[component]
fn MenuNodeItem(nodes: Memo<Vec<MenuNode>>, key: u64, test_prefix: String) -> MenuItem {
    let node =
        nodes.with_untracked(|nodes| nodes.iter().find(|node| node.key() == Some(key)).cloned());
    let separated = create_memo(move || {
        nodes.with(|nodes| {
            let at = nodes.iter().position(|node| node.key() == Some(key));
            at.is_some_and(|at| at > 0 && matches!(nodes[at - 1], MenuNode::Separator))
        })
    });
    let row_test_id = node
        .as_ref()
        .and_then(MenuNode::id)
        .filter(|_| !test_prefix.is_empty())
        .map(|id| format!("{test_prefix}.{id}"));
    let mut item = match node {
        Some(MenuNode::Action { action, label }) => {
            let label = label.unwrap_or_default();
            view! {
                <MenuItem action label separated />
            }
        }
        Some(MenuNode::Submenu(submenu)) => {
            let items = submenu.items.clone();
            view! {
                <MenuItem label={submenu.label.clone()} glyph={submenu.glyph} separated>
                    <MenuNodes nodes={items} test_prefix />
                </MenuItem>
            }
        }
        Some(MenuNode::Separator) | None => view! {
            <MenuItem label="" disabled=true />
        },
    };
    item.test_id = row_test_id;
    item
}

#[derive(Clone, Copy)]
pub struct MenuListApi(State);

impl MenuListApi {
    pub fn row_count(&self) -> usize {
        self.0.rows.value().borrow().len()
    }

    pub fn row_button(&self, index: usize) -> NodeId {
        self.0.rows.value().borrow()[index].button.get()
    }

    pub fn tile_button(&self, index: usize, tile: usize) -> NodeId {
        let rows = self.0.rows.value();
        let rows = rows.borrow();
        let button = rows[index].tiles.value().borrow()[tile].button;
        button.get()
    }

    pub fn back_button(&self) -> Option<NodeId> {
        self.0.back.try_get()
    }

    pub fn page(&self) -> Option<NodeId> {
        let state = &self.0;
        state.page.get_untracked()?;
        state.page_content.try_get()
    }

    pub fn row_submenu_content(&self, index: usize) -> Option<NodeId> {
        self.0.rows.value().borrow()[index]
            .submenu
            .content
            .try_get()
    }

    pub fn root_focusable(&self) -> NodeId {
        self.0.root.get()
    }
}
