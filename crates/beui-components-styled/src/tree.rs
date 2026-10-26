use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use std::hash::Hash;

use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::context_menu::menu_style;
use crate::scroll::Scroll;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::text_input::text_input_style;
use crate::theme::{Density, ThemeStore, use_density, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, Edge, PopupTriggerHandle, TextInputHandle, TreeItem, TreeMenu, TreeRevealHandle,
    TreeRowEdit, TreeRowHandle, TreeToggleHandle,
};
use beui_core::color::Color32;
use beui_core::font::TextAlign;
use beui_core::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CHEVRON_DOWN, ICON_CHEVRON_RIGHT, ICON_ELLIPSIS,
};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const COMPACT: Metrics = Metrics {
    row: 22.0,
    indent: 12.0,
    chevron: 16.0,
    menu: 20.0,
    glyph: TextSize::Xs,
};
const COMFORTABLE: Metrics = Metrics {
    row: 44.0,
    indent: 20.0,
    chevron: 36.0,
    menu: 40.0,
    glyph: TextSize::Lg,
};
const CHEVRON_RADIUS: f32 = 3.0;
const SPACING: f32 = 6.0;
const PADDING_HORIZONTAL: f32 = 4.0;
const OUTLINE_WIDTH: f32 = 1.0;
const REVEAL_PADDING: f32 = 8.0;
const EDIT_PADDING: f32 = 3.0;

#[derive(Clone, Copy, PartialEq)]
struct Metrics {
    row: f32,
    indent: f32,
    chevron: f32,
    menu: f32,
    glyph: TextSize,
}

fn metrics(density: Density) -> Metrics {
    match density {
        Density::Compact => COMPACT,
        Density::Comfortable => COMFORTABLE,
    }
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
    #[prop(default = ComponentRef::new())] api: ComponentRef<unstyled::TreeApi<K>>,
    keys: Prop<Vec<K>>,
    item: Func<K, TreeItem>,
    selected: Prop<Option<K>>,
    #[prop(default = None)] ancestors: Option<Func<K, Vec<K>>>,
    #[prop(default = 0.0)] spacing: f32,
    #[prop(default = 0.0)] padding: f32,
    #[prop(default = None)] row_test_id: Option<Func<K, String>>,
    #[prop(default = String::new())] reveal_test_id: String,
    #[prop(default = None)] outline: Option<Func<K, Option<Color32>>>,
    #[prop(default = None)] menu: Option<Func<K, Option<TreeMenu>>>,
    #[prop(default = None)] editing: Prop<Option<K>>,
    on_select: Callback<K>,
    on_open: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_reveal: Callback<K>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
    on_edit: Callback<K>,
    on_rename: Callback<(K, String)>,
    on_cancel_edit: Callback<K>,
    #[prop(children)] content: Option<RenderFn<TreeRowFace<K>>>,
) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let content = content.expect("tree requires a `content` builder");
    let row_test_id = row_test_id.unwrap_or_else(|| Func::new(|_| String::new()));
    let outline = outline.unwrap_or_else(|| Func::new(|_| None));
    let viewport = NodeRef::new();
    let ancestors = ancestors.unwrap_or_else(|| Func::new(|_| Vec::new()));
    view! {
        <List spacing=0.0>
            <Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&viewport}>
                <Frame padding=padding>
                    <unstyled::Tree
                        api
                        keys
                        item
                        selected
                        ancestors
                        spacing
                        editing
                        menu={menu.unwrap_or_else(|| Func::new(|_| None))}
                        menu_style={menu_style()}
                        on_select={on_select}
                        on_open={on_open}
                        on_expand={on_expand}
                        on_hover_change={on_hover_change}
                        on_drag_start={on_drag_start}
                        on_edit={on_edit}
                        on_rename={on_rename}
                        on_cancel_edit={on_cancel_edit}
                    >|handle: TreeRowHandle<K>| {
                        let named = row_test_id.call(handle.key.clone());
                        view! {
                            <TreeRowChrome handle named outline={outline} content={content} />
                        }
                    }</unstyled::Tree>
                </Frame>
            </Scroll>
            <unstyled::TreeReveal
                tree={api}
                viewport={viewport}
                on_reveal={on_reveal}
                button={move |handle: TreeRevealHandle| {
                    let named = reveal_test_id.clone();
                    view! {
                        <RevealButton handle named />
                    }
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
        Edge::Top | Edge::TopEnd => ICON_ARROW_UP,
        Edge::Bottom | Edge::BottomEnd => ICON_ARROW_DOWN,
    });
    view! {
        <Frame padding=REVEAL_PADDING>
            <Button
                @test_id={named}
                glyph={glyph}
                label={label}
                variant=ButtonVariant::Secondary
                on_click={reveal}
            />
        </Frame>
    }
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
        editing,
        edit,
        menu,
        ..
    } = handle;
    let theme = use_theme();
    let density = use_density();
    let sized = create_memo(move || metrics(density.get()));
    let height = create_memo(move || Extent::px(sized.get().row));
    let fill = create_memo(move || background(&theme, selected.get(), hovered.get(), active.get()));
    let highlight = create_memo({
        let key = key.clone();
        move || outline.call(key.clone())
    });
    let outline_color = create_memo(move || highlight.get().unwrap_or_else(|| theme.accent.get()));
    let ring = focus_ring(focused);
    let outlined = create_memo(move || highlight.get().is_some() || ring.get());
    let face = TreeRowFace {
        key,
        item,
        selected,
        focused,
        hovered,
    };
    let shown = create_memo(move || !editing.get());
    let renaming = edit_id(&named);
    let body = match menu {
        None => view! {
            <List align=Align::Stretch spacing=0.0>
                <ShowKeepAlive condition={shown}>
                    <unstyled::TreeRowArea @sizing=ItemSize::Percent(100.0) target>
                        {content.call(face)}
                    </unstyled::TreeRowArea>
                </ShowKeepAlive>
                <Show condition={editing}>
                    <RowEditField
                        @sizing=ItemSize::Percent(100.0)
                        @test_id={renaming.clone()}
                        edit
                        sized={sized}
                    />
                </Show>
            </List>
        },
        Some(menu) => {
            let more = more_id(&named);
            view! {
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <ShowKeepAlive condition={shown}>
                        <unstyled::TreeRowArea @sizing=ItemSize::Percent(100.0) target>
                            {content.call(face)}
                        </unstyled::TreeRowArea>
                    </ShowKeepAlive>
                    <Show condition={editing}>
                        <RowEditField
                            @sizing=ItemSize::Percent(100.0)
                            @test_id={renaming.clone()}
                            edit
                            sized={sized}
                        />
                    </Show>
                    <unstyled::TreeRowMenuButton @test_id={more} menu>|handle: PopupTriggerHandle|
                        <RowMenuFace handle sized={sized} />
                    </unstyled::TreeRowMenuButton>
                </List>
            }
        }
    };
    view! {
        <Frame height={height}>
            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                <Chevron
                    item={item}
                    marked={marked}
                    toggle={toggle}
                    sized={sized}
                    named={chevron_id(&named)}
                />
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={row_id(&named)}
                    height={height}
                    fill={ColorRole::custom(fill)}
                    border={create_memo(move || outlined.get().then(|| Border::new(ColorRole::Custom(outline_color.get())).width(OUTLINE_WIDTH)))}
                    radius=Radius::Md
                    padding={Sides::horizontal(PADDING_HORIZONTAL)}
                >
                    {body}
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn RowEditField(edit: TreeRowEdit, sized: Memo<Metrics>) -> NodeId {
    let height = create_memo(move || Extent::px(sized.get().row - 2.0 * OUTLINE_WIDTH));
    let style = unstyled::TextInputStyle {
        padding_horizontal: Prop::Static(EDIT_PADDING),
        ..text_input_style()
    };
    view! {
        <unstyled::TreeRowEditor
            edit
            style
            content={move |handle: TextInputHandle| view! {
                <Frame
                    height={height}
                    fill=ColorRole::SurfaceRaised
                    border={Border::new(ColorRole::Accent).width(OUTLINE_WIDTH)}
                    radius=Radius::Sm
                >
                    <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                        {handle.field} @sizing=ItemSize::Percent(100.0)
                    </List>
                </Frame>
            }}
        />
    }
}

#[component]
fn RowMenuFace(handle: PopupTriggerHandle, sized: Memo<Metrics>) -> NodeId {
    let PopupTriggerHandle {
        open,
        button:
            ButtonHandle {
                hovered,
                active,
                focused,
                tooltip,
                ..
            },
    } = handle;
    let theme = use_theme();
    let fill = create_memo(move || match (active.get() || open.get(), hovered.get()) {
        (true, _) => theme.pressed.get(),
        (false, true) => theme.hover.get(),
        (false, false) => Color32::TRANSPARENT,
    });
    let color = create_memo(move || match hovered.get() || active.get() || open.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let extent = create_memo(move || Extent::px(sized.get().menu));
    let glyph_size = create_memo(move || sized.get().glyph);
    view! {
        <Tooltip label={tooltip}>
            <Frame
                width={extent}
                height={extent}
                fill={ColorRole::custom(fill)}
                border={focus_border(focused, Border::new(ColorRole::Accent).width(OUTLINE_WIDTH))}
                radius={Radius::custom(CHEVRON_RADIUS)}
            >
                <Text size={glyph_size} color={color} align=TextAlign::Center>
                    <Icon glyph=ICON_ELLIPSIS />
                </Text>
            </Frame>
        </Tooltip>
    }
}

#[component]
fn Chevron(
    item: Memo<TreeItem>,
    marked: Memo<bool>,
    toggle: ClickCallback,
    sized: Memo<Metrics>,
    named: String,
) -> NodeId {
    let zone = create_memo(move || {
        let sized = sized.get();
        Extent::px(item.get().depth as f32 * sized.indent + sized.chevron + SPACING)
    });
    view! {
        <Frame width={zone}>
            <List spacing=0.0>
                <unstyled::TreeToggle
                    item
                    toggle={toggle}
                    button_test_id=named
                >|handle: TreeToggleHandle|
                    <ChevronFace handle item marked={marked} sized={sized} />
                </unstyled::TreeToggle>
            </List>
        </Frame>
    }
}

#[component]
fn ChevronFace(
    handle: TreeToggleHandle,
    item: Memo<TreeItem>,
    marked: Memo<bool>,
    sized: Memo<Metrics>,
) -> NodeId {
    let TreeToggleHandle {
        button:
            ButtonHandle {
                hovered,
                active,
                focused,
                label,
                ..
            },
        expanded,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(move || match (active.get(), marked.get(), hovered.get()) {
        (true, _, _) => theme.pressed.get(),
        (false, true, _) => theme.accent_soft.get(),
        (false, false, true) => theme.hover.get(),
        (false, false, false) => Color32::TRANSPARENT,
    });
    let ring = focus_ring(focused);
    let outlined = create_memo(move || marked.get() || ring.get());
    let color = create_memo(move || match marked.get() || hovered.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let glyph = create_memo(move || match expanded.get() {
        true => ICON_CHEVRON_DOWN,
        false => ICON_CHEVRON_RIGHT,
    });
    let indent = create_memo(move || ItemSize::Fixed(item.get().depth as f32 * sized.get().indent));
    let width = create_memo(move || Extent::px(sized.get().chevron));
    let height = create_memo(move || Extent::px(sized.get().row));
    let glyph_size = create_memo(move || sized.get().glyph);
    view! {
        <Tooltip label={label}>
            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                <Spacer @sizing={indent} />
                <Frame
                    width={width}
                    height={height}
                    fill={ColorRole::custom(fill)}
                    border={create_memo(move || outlined.get().then_some(Border::new(ColorRole::Accent).width(OUTLINE_WIDTH)))}
                    radius={Radius::custom(CHEVRON_RADIUS)}
                >
                    <Text size={glyph_size} color={color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
                <Spacer @sizing=ItemSize::Fixed(SPACING) />
            </List>
        </Tooltip>
    }
}

fn row_id(named: &str) -> String {
    match named.is_empty() {
        true => String::new(),
        false => format!("{named}.row"),
    }
}

fn more_id(named: &str) -> String {
    match named.is_empty() {
        true => String::new(),
        false => format!("{named}.more"),
    }
}

fn edit_id(named: &str) -> String {
    match named.is_empty() {
        true => String::new(),
        false => format!("{named}.rename"),
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
