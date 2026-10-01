use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node as AccessNode, Role};
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::scroll::Scroll;
use crate::text::IconSized;
use crate::theme::{FONT_SMALL, RADIUS, ThemeStore, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ButtonHandle, Edge, TreeItem, TreeRevealHandle, TreeRowHandle};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Direction, Frame, Func, ItemSize, List, Memo, NodeRef, Prop, ReadSignal,
    RenderFn, Show, Spacer, clone, create_memo, focus_ring, set_component_state,
};

const INDENT: f32 = 12.0;
const CHEVRON_WIDTH: f32 = 16.0;
const CHEVRON_RADIUS: u8 = 3;
const ROW_HEIGHT: f32 = 22.0;
const SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 4.0;
const OUTLINE_WIDTH: f32 = 1.0;
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
    #[prop(default = false)] selection_follows_focus: bool,
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
    let rows = NodeRef::new();
    let viewport = NodeRef::new();
    set_component_state(Parts { rows: rows.clone() });
    let tracked = rows.clone();
    let ancestors = ancestors.unwrap_or_else(|| Func::new(|_| Vec::new()));
    view! {
        <List spacing=0.0>
            <Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&viewport} focus_color>
                <Frame padding_horizontal={padding} padding_vertical={padding}>
                    <unstyled::Tree
                        @node_ref={&tracked}
                        keys
                        item
                        selected
                        ancestors
                        spacing
                        selection_follows_focus
                        on_select={move |key| on_select.call(key)}
                        on_expand={move |expansion| on_expand.call(expansion)}
                        on_hover_change={move |hover| on_hover_change.call(hover)}
                        on_drag_start={move |key| on_drag_start.call(key)}
                    >
                        {move |handle: TreeRowHandle<K>| {
                            let named = row_test_id.call(handle.key.clone());
                            view! {
                                <TreeRowChrome
                                    handle
                                    named
                                    outline={outline.clone()}
                                    content={content.clone()}
                                />
                            }
                        }}
                    </unstyled::Tree>
                </Frame>
            </Scroll>
            <unstyled::TreeReveal
                tree={rows}
                viewport={viewport}
                on_reveal={move |key: K| on_reveal.call(key)}
                button={move |handle: TreeRevealHandle| view! {
                    <RevealButton handle named={reveal_test_id} />
                }}
            />
        </List>
    }
}

#[component]
fn RevealButton(handle: TreeRevealHandle, named: String) -> NodeId {
    let TreeRevealHandle {
        edge,
        label,
        reveal,
    } = handle;
    let glyph = create_memo(move || match edge.get() {
        Edge::Top | Edge::TopEnd => ICON_ARROW_UPWARD.to_owned(),
        Edge::Bottom | Edge::BottomEnd => ICON_ARROW_DOWNWARD.to_owned(),
    });
    view! {
        <Frame padding_horizontal=REVEAL_PADDING padding_vertical=REVEAL_PADDING>
            <Button
                @test_id={named}
                glyph={glyph}
                label={label}
                variant=ButtonVariant::Secondary
                on_click={move || reveal.call()}
            />
        </Frame>
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

#[component]
fn TreeRowChrome<K>(
    handle: TreeRowHandle<K>,
    named: String,
    outline: Func<K, Option<Color32>>,
    content: RenderFn<TreeRowFace<K>>,
) -> NodeId
where
    K: Clone + 'static,
{
    let TreeRowHandle {
        key,
        item,
        selected,
        marked,
        focused,
        hovered,
        active,
        toggle,
        target,
        ..
    } = handle;
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
    let ring = focus_ring(focused.clone());
    let outlined = create_memo(clone!(highlight -> move || {
        highlight.get().is_some() || ring.get()
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
                    <unstyled::TreeRowArea target>{content.call(face)}</unstyled::TreeRowArea>
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
                        press_focus=false
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
    let ring = focus_ring(focused);
    let outlined = create_memo(clone!(marked -> move || marked.get() || ring.get()));
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
