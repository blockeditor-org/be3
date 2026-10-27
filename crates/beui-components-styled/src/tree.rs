use crate::frame::{Border, ColorRole, Frame, Radius, focus_border};
use std::hash::Hash;

use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::context_menu::menu_style;
use crate::list::List;
use crate::list::Spacer;
use crate::scroll::{Bleed, Scroll};
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::text_input::text_input_style;
use crate::theme::{Density, use_density};
use crate::tokens::{Padding, Size, Space, Stroke, use_space};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, Edge, PopupTriggerHandle, TextInputHandle, TreeItem, TreeMenu, TreeRevealHandle,
    TreeRowEdit, TreeRowHandle, TreeToggleHandle,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Pos2;
use beui_view::types::TextAlign;
use beui_view::types::icons::{
    ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CHEVRON_DOWN, ICON_CHEVRON_RIGHT, ICON_ELLIPSIS,
};
use beui_view::types::{Align, Direction, ItemSize};

const INDENT: Space = Space::Lg;
const CHEVRON_RADIUS: Radius = Radius::Sm;
const SPACING: Space = Space::Sm;
const PADDING_HORIZONTAL: Space = Space::Xs;
const OUTLINE_WIDTH: Stroke = Stroke::Hairline;
const REVEAL_PADDING: Space = Space::Md;

#[derive(Clone, Copy, PartialEq)]
struct Metrics {
    row: f32,
    indent: f32,
    spacing: f32,
    chevron: f32,
    menu: f32,
    glyph: TextSize,
}

fn metrics(density: Density) -> Metrics {
    let (chevron, menu, glyph) = match density {
        Density::Compact => (Size::Xs.px(), Size::Sm.px(), TextSize::Xs),
        Density::Comfortable => (36.0, Size::Xl.px(), TextSize::Lg),
    };
    Metrics {
        row: Size::Row.resolve(density),
        indent: INDENT.resolve(density),
        spacing: SPACING.resolve(density),
        chevron,
        menu,
        glyph,
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
    #[prop(default = Space::None)] spacing: Prop<Space>,
    #[prop(default = Space::None)] padding: Prop<Space>,
    #[prop(default = Bleed::NONE)] bleed: Bleed,
    #[prop(default = None)] row_test_id: Option<Func<K, String>>,
    #[prop(default = String::new())] reveal_test_id: String,
    #[prop(default = None)] outline: Option<Func<K, Option<ColorRole>>>,
    #[prop(default = None)] menu: Option<Func<K, Option<TreeMenu>>>,
    #[prop(default = None)] editing: Prop<Option<K>>,
    #[prop(default = false)] reorder: bool,
    on_select: Callback<K>,
    on_open: Callback<K>,
    on_expand: Callback<(K, bool)>,
    on_reveal: Callback<K>,
    on_hover_change: Callback<(K, bool)>,
    on_drag_start: Callback<K>,
    on_drag_move: Callback<(K, Pos2)>,
    on_drop: Callback<(K, Pos2)>,
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
    let spacing = use_space(spacing).get_untracked();
    view! {
        <List spacing=Space::None>
            <Scroll @sizing=ItemSize::Percent(100.0) @node_ref={&viewport} bleed>
                <Frame padding=padding>
                    <unstyled::Tree
                        api
                        keys
                        item
                        selected
                        ancestors
                        spacing
                        editing
                        reorder
                        menu={menu.map(|menu| unstyled::TreeMenus::new(menu_style(), menu))}
                        on_select={on_select}
                        on_open={on_open}
                        on_expand={on_expand}
                        on_hover_change={on_hover_change}
                        on_drag_start={on_drag_start}
                        on_drag_move={on_drag_move}
                        on_drop={on_drop}
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
    outline: Func<K, Option<ColorRole>>,
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
    let density = use_density();
    let sized = create_memo(move || metrics(density.get()));
    let height = create_memo(move || Size::Custom(sized.get().row));
    let fill = create_memo(move || background(selected.get(), hovered.get(), active.get()));
    let highlight = create_memo({
        let key = key.clone();
        move || outline.call(key.clone())
    });
    let ring = focus_ring(focused);
    let border = create_memo(move || {
        let color = highlight.get().unwrap_or(ColorRole::Accent);
        (highlight.get().is_some() || ring.get()).then(|| Border::new(color).width(OUTLINE_WIDTH))
    });
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
            <List align=Align::Stretch spacing=Space::None>
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
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
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
                    fill={fill}
                    border={border}
                    radius=Radius::Md
                    padding={Padding::horizontal(PADDING_HORIZONTAL)}
                >
                    {body}
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn RowEditField(edit: TreeRowEdit, sized: Memo<Metrics>) -> NodeId {
    let height = create_memo(move || Size::Custom(sized.get().row - 2.0 * OUTLINE_WIDTH.px()));
    let style = unstyled::TextInputStyle {
        padding_horizontal: use_space(PADDING_HORIZONTAL).map(|inset| inset - OUTLINE_WIDTH.px()),
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
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
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
    let fill = create_memo(move || match (active.get() || open.get(), hovered.get()) {
        (true, _) => ColorRole::Pressed,
        (false, true) => ColorRole::Hover,
        (false, false) => ColorRole::None,
    });
    let color = create_memo(move || match hovered.get() || active.get() || open.get() {
        true => TextColor::Default,
        false => TextColor::Muted,
    });
    let extent = create_memo(move || Size::Custom(sized.get().menu));
    let glyph_size = create_memo(move || sized.get().glyph);
    view! {
        <Tooltip label={tooltip}>
            <Frame
                width={extent}
                height={extent}
                fill={fill}
                border={focus_border(focused, Border::new(ColorRole::Accent).width(OUTLINE_WIDTH))}
                radius=CHEVRON_RADIUS
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
        Size::Custom(item.get().depth as f32 * sized.indent + sized.chevron + sized.spacing)
    });
    view! {
        <Frame width={zone}>
            <List spacing=Space::None>
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
    let fill = create_memo(move || match (active.get(), marked.get(), hovered.get()) {
        (true, _, _) => ColorRole::Pressed,
        (false, true, _) => ColorRole::AccentSoft,
        (false, false, true) => ColorRole::Hover,
        (false, false, false) => ColorRole::None,
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
    let indent = create_memo(move || Space::Custom(item.get().depth as f32 * sized.get().indent));
    let width = create_memo(move || Size::Custom(sized.get().chevron));
    let height = create_memo(move || Size::Custom(sized.get().row));
    let glyph_size = create_memo(move || sized.get().glyph);
    view! {
        <Tooltip label={label}>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                <Spacer width={indent} />
                <Frame
                    width={width}
                    height={height}
                    fill={fill}
                    border={create_memo(move || outlined.get().then_some(Border::new(ColorRole::Accent).width(OUTLINE_WIDTH)))}
                    radius=CHEVRON_RADIUS
                >
                    <Text size={glyph_size} color={color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
                <Spacer width=SPACING />
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

fn background(selected: bool, hovered: bool, active: bool) -> ColorRole {
    match (selected, hovered, active) {
        (_, _, true) => ColorRole::Pressed,
        (true, true, _) => ColorRole::AccentSoftHover,
        (true, false, _) => ColorRole::AccentSoft,
        (false, true, _) => ColorRole::Hover,
        (false, false, false) => ColorRole::None,
    }
}
