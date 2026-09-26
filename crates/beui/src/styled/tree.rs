use std::cell::Cell;
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node as AccessNode, Role};
use beui_macros::{component, view};

use crate::color::Color32;
use crate::document::Document;
use crate::geometry::{Pos2, Rect};
use crate::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
};
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Align, Callback, ClickCatcher, Direction, Frame, Func, ItemSize, List, Memo, NodeRef, Prop,
    ReadSignal, RenderFn, Show, Spacer, clone, create_effect, create_memo, create_selector,
    create_signal, node_placed, node_rect, set_component_state, with_document,
};
use crate::styled::button::{Button, ButtonVariant};
use crate::styled::scroll::Scroll;
use crate::styled::text::IconSized;
use crate::styled::theme::{FONT_SMALL, RADIUS, ThemeStore, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{ButtonHandle, Edge, Floating, TreeItem, TreeRowHandle};

const INDENT: f32 = 12.0;
const CHEVRON_WIDTH: f32 = 16.0;
const CHEVRON_RADIUS: u8 = 3;
const ROW_HEIGHT: f32 = 22.0;
const SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 4.0;
const OUTLINE_WIDTH: f32 = 1.0;
const DRAG_THRESHOLD: f32 = 6.0;
const REVEAL_PADDING: f32 = 8.0;

struct Parts {
    rows: NodeRef,
}

pub struct TreeRowFace<K> {
    pub key: K,
    pub item: Memo<TreeItem>,
    pub selected: Memo<bool>,
    pub focused: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
}

#[component]
pub fn Tree<K>(
    keys: Prop<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Prop<Option<K>>,
    #[prop(default = None)] ancestors: Option<Func<K, Vec<K>>>,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = 0.0)] padding: f32,
    #[prop(default = None)] focus_color: Prop<Option<Color32>>,
    #[prop(default = None)] row_test_id: Option<Func<K, String>>,
    #[prop(default = String::new())] reveal_test_id: String,
    #[prop(default = None)] outline: Option<Func<K, Option<Color32>>>,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_reveal: Callback<K>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
    #[prop(children)] content: Option<RenderFn<TreeRowFace<K>>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let content = content.expect("tree requires a `content` builder");
    let row_test_id = row_test_id.unwrap_or_else(|| Func::new(|_| String::new()));
    let outline = outline.unwrap_or_else(|| Func::new(|_| None));
    let ancestors = ancestors.unwrap_or_else(|| Func::new(|_| Vec::new()));
    let keys = create_memo(move || keys.get());
    let selected = create_memo(move || selected.get());
    let shown = create_memo(clone!(keys selected -> move || {
        let selected = selected.get()?;
        let lineage = ancestors.call(selected.clone());
        keys.with(|keys| {
            std::iter::once(selected)
                .chain(lineage.into_iter().rev())
                .find(|key| keys.contains(key))
        })
    }));
    let buried = create_memo(clone!(shown selected -> move || {
        shown.get().filter(|key| selected.get().as_ref() != Some(key))
    }));
    let burial = create_selector(clone!(buried -> move || buried.get()));
    let rows = NodeRef::new();
    let viewport = NodeRef::new();
    set_component_state(Parts { rows: rows.clone() });
    let dragging = on_drag_start;
    let (tracked, listed, described, chosen) =
        (rows.clone(), keys.clone(), item.clone(), selected.clone());
    view! {
        <List spacing=0.0>
            <Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&viewport} focus_color>
                <Frame padding_horizontal={padding} padding_vertical={padding}>
                    <unstyled::Tree
                        @node_ref={&tracked}
                        keys={listed}
                        item={described}
                        selected={chosen}
                        spacing
                        on_select={move |key| on_select.call(key)}
                        on_expand={move |expansion| on_expand.call(expansion)}
                        on_hover_change={move |hover| on_hover_change.call(hover)}
                    >
                        {move |handle: TreeRowHandle<K>| {
                            let named = row_test_id.call(handle.key.clone());
                            let marked = burial.memo(Some(handle.key.clone()));
                            let dragging = dragging.clone();
                            view! {
                                <TreeRowChrome
                                    handle
                                    named
                                    marked
                                    outline={outline.clone()}
                                    content={content.clone()}
                                    on_drag_start={move |key: K| dragging.call(key)}
                                />
                            }
                        }}
                    </unstyled::Tree>
                </Frame>
            </Scroll>
            <RevealSelection
                rows
                viewport
                keys
                item
                selected
                shown
                buried
                named={reveal_test_id}
                on_reveal={move |key| on_reveal.call(key)}
            />
        </List>
    }
}

#[component]
fn RevealSelection<K>(
    rows: NodeRef,
    viewport: NodeRef,
    keys: Memo<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Memo<Option<K>>,
    shown: Memo<Option<K>>,
    buried: Memo<Option<K>>,
    named: String,
    on_reveal: Callback<K>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let (list, scroll) = (rows.get(), viewport.get());
    let astray = create_memo(clone!(shown buried keys -> move || {
        let key = shown.get()?;
        keys.with(|_| ());
        node_rect(list).get();
        let row = with_document(|document| unstyled::tree_row_node::<K>(document, list, &key))?;
        if !node_placed(row).get() || !node_placed(scroll).get() {
            return None;
        }
        stray_edge(node_rect(row).get(), node_rect(scroll).get(), buried.get().is_some())
    }));
    let (pending, set_pending) = create_signal(None::<K>);
    create_effect(clone!(keys set_pending -> move || {
        let Some(key) = pending.get() else {
            return;
        };
        keys.with(|_| ());
        node_rect(list).get();
        let Some(row) = with_document(|document| unstyled::tree_row_node::<K>(document, list, &key))
        else {
            return;
        };
        if !node_placed(row).get() {
            return;
        }
        with_document(|document| document.reveal_node(row));
        set_pending.set(None);
    }));
    let open = create_memo(clone!(astray -> move || astray.get().is_some()));
    let edge = create_memo(clone!(astray -> move || astray.get().unwrap_or(Edge::Bottom)));
    let glyph = create_memo(clone!(edge -> move || match edge.get() {
        Edge::Top => ICON_ARROW_UPWARD.to_owned(),
        Edge::Bottom => ICON_ARROW_DOWNWARD.to_owned(),
    }));
    let label = create_memo(clone!(selected -> move || {
        let label = selected.get().map(|key| item.call(key).label).unwrap_or_default();
        match label.is_empty() {
            true => "Show the selection".to_owned(),
            false => label,
        }
    }));
    let reveal = move || {
        let Some(key) = selected.get_untracked() else {
            return;
        };
        on_reveal.call(key.clone());
        set_pending.set(Some(key));
    };
    view! {
        <Floating anchor={viewport} edge={edge} open={open}>
            <Frame padding_horizontal=REVEAL_PADDING padding_vertical=REVEAL_PADDING>
                <Button
                    @test_id={named}
                    glyph={glyph}
                    label={label}
                    variant=ButtonVariant::Secondary
                    on_click={reveal}
                />
            </Frame>
        </Floating>
    }
}

pub fn tree_row_node<K>(document: &Document, tree: NodeId, key: &K) -> Option<NodeId>
where
    K: Clone + Eq + Hash + 'static,
{
    unstyled::tree_row_node::<K>(document, tree_rows(document, tree), key)
}

pub fn tree_focused<K>(document: &Document, tree: NodeId) -> Option<K>
where
    K: Clone + Eq + Hash + 'static,
{
    unstyled::tree_focused::<K>(document, tree_rows(document, tree))
}

pub fn tree_rows(document: &Document, tree: NodeId) -> NodeId {
    document.component_state::<Parts>(tree).rows.get()
}

fn stray_edge(row: Rect, viewport: Rect, buried: bool) -> Option<Edge> {
    if row.bottom() <= viewport.top() {
        return Some(Edge::Top);
    }
    if row.top() >= viewport.bottom() || buried {
        return Some(Edge::Bottom);
    }
    None
}

#[component]
fn TreeRowChrome<K>(
    handle: TreeRowHandle<K>,
    named: String,
    marked: Memo<bool>,
    outline: Func<K, Option<Color32>>,
    content: RenderFn<TreeRowFace<K>>,
    on_drag_start: Callback<K>,
) -> NodeId
where
    K: Clone + 'static,
{
    let TreeRowHandle {
        key,
        item,
        selected,
        focused,
        select,
        toggle,
        hover,
    } = handle;
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let gesture: Rc<Cell<Option<(Pos2, bool)>>> = Rc::default();
    let pressed = clone!(gesture -> move |press: PointerPress| {
        gesture.set(Some((press.pos, false)));
    });
    let dragged = clone!(gesture key -> move |at: PointerPress| {
        let Some((origin, started)) = gesture.get() else {
            return;
        };
        if started || (at.pos - origin).length() < DRAG_THRESHOLD {
            return;
        }
        gesture.set(Some((origin, true)));
        on_drag_start.call(key.clone());
    });
    let settled = clone!(gesture -> move |down: bool| {
        set_active.set(down);
        if !down {
            gesture.set(None);
        }
    });
    let chose = clone!(gesture -> move || {
        if gesture.get().is_none_or(|(_, started)| !started) {
            select();
        }
    });
    let theme = use_theme();
    let indent = create_memo(clone!(item -> move || {
        ItemSize::Fixed(item.get().depth as f32 * INDENT)
    }));
    let fill = create_memo(clone!(theme selected hovered active -> move || {
        background(&theme, selected.get(), hovered.get(), active.get())
    }));
    let highlight = create_memo(clone!(outline key -> move || outline.call(key.clone())));
    let outline_color = create_memo(clone!(theme highlight -> move || {
        highlight.get().unwrap_or_else(|| theme.accent.get())
    }));
    let outlined = create_memo(clone!(highlight focused -> move || {
        highlight.get().is_some() || focused.get()
    }));
    let face = TreeRowFace {
        key,
        item: item.clone(),
        selected,
        focused,
        hovered,
    };
    view! {
        <Frame height=ROW_HEIGHT>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Spacer @sizing={indent} />
                <Chevron item={item} marked={marked} toggle={toggle} named={chevron_id(&named)} />
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={row_id(&named)}
                    height=ROW_HEIGHT
                    color={fill}
                    outline={outline_color}
                    outline_width=OUTLINE_WIDTH
                    outline_visible={outlined}
                    radius=RADIUS
                    padding_horizontal=PADDING_HORIZONTAL
                >
                    <ClickCatcher
                        cursor=CursorIcon::PointingHand
                        on_press={pressed}
                        on_drag={dragged}
                        on_click={chose}
                        on_active_change={settled}
                        on_hover_change={move |over: bool| {
                            set_hovered.set(over);
                            hover(over);
                        }}
                    >
                        {content.call(face)}
                    </ClickCatcher>
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn Chevron(
    item: Memo<TreeItem>,
    marked: Memo<bool>,
    toggle: Rc<dyn Fn()>,
    named: String,
) -> NodeId {
    let expandable = create_memo(clone!(item -> move || item.get().expandable));
    let glyph = create_memo(clone!(item -> move || marker(&item.get())));
    let label = create_memo(clone!(item -> move || expansion_label(item.get().expanded)));
    view! {
        <Frame width=CHEVRON_WIDTH>
            <List spacing=0.0>
                <Show condition={expandable}>
                    <unstyled::Button
                        @test_id={named}
                        tab_stop=false
                        accessibility={chevron_accessibility(item)}
                        on_click={move || toggle()}
                        content={move |button: ButtonHandle| view! {
                            <ChevronFace
                                handle={button}
                                glyph={glyph}
                                marked={marked}
                                label={label}
                            />
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ChevronFace(
    handle: ButtonHandle,
    glyph: Memo<String>,
    marked: Memo<bool>,
    label: Memo<String>,
) -> NodeId {
    let ButtonHandle {
        hovered,
        active,
        focused,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme marked hovered active -> move || {
        match (active.get(), marked.get(), hovered.get()) {
            (true, _, _) => theme.pressed.get(),
            (false, true, _) => theme.accent_soft.get(),
            (false, false, true) => theme.hover.get(),
            (false, false, false) => Color32::TRANSPARENT,
        }
    }));
    let outlined = create_memo(clone!(marked focused -> move || marked.get() || focused.get()));
    let color = create_memo(clone!(theme marked hovered -> move || {
        match marked.get() || hovered.get() {
            true => theme.text.get(),
            false => theme.text_muted.get(),
        }
    }));
    view! {
        <Tooltip label={label}>
            <Frame
                width=CHEVRON_WIDTH
                height=ROW_HEIGHT
                color={fill}
                outline={theme.accent.clone()}
                outline_width=OUTLINE_WIDTH
                outline_visible={outlined}
                radius=CHEVRON_RADIUS
            >
                <IconSized glyph={glyph} font_size=FONT_SMALL color={color} />
            </Frame>
        </Tooltip>
    }
}

fn chevron_accessibility(item: Memo<TreeItem>) -> Memo<AccessNode> {
    create_memo(move || {
        let mut node = AccessNode::new(Role::Button);
        node.set_expanded(item.get().expanded);
        node.set_label(expansion_label(item.get().expanded));
        node
    })
}

fn expansion_label(expanded: bool) -> String {
    match expanded {
        true => "Collapse".to_owned(),
        false => "Expand".to_owned(),
    }
}

fn marker(item: &TreeItem) -> String {
    match (item.expandable, item.expanded) {
        (false, _) => String::new(),
        (true, true) => ICON_KEYBOARD_ARROW_DOWN.to_owned(),
        (true, false) => ICON_KEYBOARD_ARROW_RIGHT.to_owned(),
    }
}

fn row_id(named: &str) -> String {
    match named.is_empty() {
        true => String::new(),
        false => format!("{named}.row"),
    }
}

fn chevron_id(named: &str) -> String {
    match named.is_empty() {
        true => String::new(),
        false => format!("{named}.chevron"),
    }
}

fn background(theme: &ThemeStore, selected: bool, hovered: bool, active: bool) -> Color32 {
    match (selected, hovered, active) {
        (_, _, true) => theme.pressed.get(),
        (true, _, _) => theme.accent_soft.get(),
        (false, true, _) => theme.hover.get(),
        (false, false, false) => Color32::TRANSPARENT,
    }
}
