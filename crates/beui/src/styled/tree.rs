use std::cell::Cell;
use std::hash::Hash;
use std::rc::Rc;

use accesskit::{Node as AccessNode, Role};
use beui_macros::{component, view};

use crate::color::Color32;
use crate::geometry::Pos2;
use crate::icons::{ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT};
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{
    Align, Callback, ClickCatcher, Direction, Frame, Func, ItemSize, List, Memo, Prop, ReadSignal,
    RenderFn, Show, Spacer, clone, create_memo, create_signal,
};
use crate::styled::text::IconSized;
use crate::styled::theme::{FONT_SMALL, RADIUS, ThemeStore, use_theme};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{ButtonHandle, TreeItem, TreeRowHandle};

const INDENT: f32 = 12.0;
const CHEVRON_WIDTH: f32 = 16.0;
const CHEVRON_RADIUS: u8 = 3;
const ROW_HEIGHT: f32 = 22.0;
const SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 4.0;
const OUTLINE_WIDTH: f32 = 1.0;
const DRAG_THRESHOLD: f32 = 6.0;

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
    #[prop(default = None)] reveal: Prop<Option<K>>,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = true)] expand_on_select: Prop<bool>,
    #[prop(default = None)] row_test_id: Option<Func<K, String>>,
    #[prop(default = None)] outline: Option<Func<K, Option<Color32>>>,
    on_select: Callback<K>,
    on_expand: Callback<(K, bool)>,
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
    let dragging = on_drag_start;
    view! {
        <unstyled::Tree
            keys
            item
            selected
            reveal
            spacing
            expand_on_select
            on_select={move |key| on_select.call(key)}
            on_expand={move |expansion| on_expand.call(expansion)}
            on_hover_change={move |hover| on_hover_change.call(hover)}
        >
            {move |handle: TreeRowHandle<K>| {
                let named = row_test_id.call(handle.key.clone());
                let dragging = dragging.clone();
                view! {
                    <TreeRowChrome
                        handle
                        named
                        outline={outline.clone()}
                        content={content.clone()}
                        on_drag_start={move |key: K| dragging.call(key)}
                    />
                }
            }}
        </unstyled::Tree>
    }
}

#[component]
fn TreeRowChrome<K>(
    handle: TreeRowHandle<K>,
    named: String,
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
                <Chevron item={item} toggle={toggle} named={chevron_id(&named)} />
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
fn Chevron(item: Memo<TreeItem>, toggle: Rc<dyn Fn()>, named: String) -> NodeId {
    let expandable = create_memo(clone!(item -> move || item.get().expandable));
    let glyph = create_memo(clone!(item -> move || marker(&item.get())));
    let marked = create_memo(clone!(item -> move || item.get().marked));
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
