use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};
use beui_view::types::Overflow;
use beui_view::types::icons::Glyph;

use crate::border::Separator;
use crate::button::{ButtonFace, ButtonVariant};
use crate::list::List;
use crate::row::{RowFace, RowSize, RowTrailing};
use crate::sheet::AnchoredSheet;
use crate::text::{Icon, Text, TextColor, TextSize, Weight};
use crate::theme::SCROLL_FADE;
use crate::tokens::{Padding, Size, Space, use_size, use_space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, MenuItem, MenuRowHandle, MenuRowKind, MenuStyle, ScrollbarStyle, SheetWhen,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Placement;
use beui_view::types::TextAlign;
use beui_view::types::icons::{
    ICON_CHECK, ICON_CHEVRON_DOWN, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_DOT,
};
use beui_view::types::{Align, Align2, Direction, ItemSize};
use beui_view::types::{Pos2, Rect};

const PADDING_HORIZONTAL: Space = Space::Lg;
const PADDING_VERTICAL: Space = Space::Sm;
const MENU_PADDING: Space = Space::Xs;
const MENU_MAX_WIDTH: f32 = 400.0;
const SUBMENU_SPACING: Space = Space::Md;
const SHORTCUT_SPACING: Space = Space::Xl3;
const SHEET_PADDING: Space = Space::Md;
const MENU_ROW_SPACING: Space = Space::Xs2;
const SHEET_SEPARATOR_SPACING: Space = Space::Xs;
const SHEET_CAPTION_PADDING: Space = Space::Lg;
const SHEET_TILE_PADDING: Space = Space::Lg;
const CAPTION_PADDING_VERTICAL: Space = Space::Xs;
const TILE_PADDING: Space = Space::Sm;
const TILE_SPACING: Space = Space::Xs;
const TREE_INDENT: Space = Space::Lg;
const TREE_CHEVRON: Size = Size::Xs;

#[component]
pub fn ContextMenu(
    children: Child,
    items: Children<MenuItem>,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = None)] open_at: Prop<Option<Pos2>>,
    #[prop(default = true)] open_at_focuses: bool,
    #[prop(default = false)] selection: bool,
    #[prop(default = None)] toolbar_at: Prop<Option<Rect>>,
    #[prop(default = true)] opens_on_hold: bool,
    on_close: ClickCallback,
    #[prop(default = Callback::default())] on_select: Callback<Vec<usize>>,
    on_toolbar_close: ClickCallback,
) -> NodeId {
    view! {
        <unstyled::ContextMenu
            items
            menu={menu_style()}
            child_size={child_size}
            disabled={disabled}
            open_at={open_at}
            open_at_focuses
            selection
            toolbar_at
            opens_on_hold
            on_close={on_close}
            on_select={on_select}
            on_toolbar_close={on_toolbar_close}
        >
            {children}
        </unstyled::ContextMenu>
    }
}

#[component]
pub fn MenuPopup(
    items: Children<MenuItem>,
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    open: Prop<bool>,
    #[prop(default = false)] sheet: Prop<bool>,
    #[prop(default = Size::Menu)] min_width: Prop<Size>,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    on_dismiss: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let open = create_memo(move || open.get());
    let sheet = create_memo(move || match sheet.get() {
        true => SheetWhen::Always,
        false => SheetWhen::Touched,
    });
    view! {
        <unstyled::MenuPopup
            items={items.into_run()}
            menu={sized_menu_style(min_width.get_untracked())}
            anchor
            placement
            open={open}
            sheet={sheet}
            overlay
            on_dismiss={on_dismiss}
            on_select={on_select}
        />
    }
}

pub fn menu_style() -> MenuStyle {
    sized_menu_style(Size::Menu)
}

fn sized_menu_style(min_width: Size) -> MenuStyle {
    MenuStyle::new(
        MENU_ROW_SPACING.px(),
        |handle| {
            view! {
                <MenuRow handle />
            }
        },
        move |content| {
            view! {
                <MenuPanel min_width>{content}</MenuPanel>
            }
        },
    )
    .with_sheet(
        |handle| {
            view! {
                <MenuSheetRow handle />
            }
        },
        |handle| {
            view! {
                <AnchoredSheet handle padding=SHEET_PADDING />
            }
        },
    )
    .with_toolbar(
        |handle| {
            view! {
                <ToolbarButton handle />
            }
        },
        |content| {
            view! {
                <ToolbarPanel>{content}</ToolbarPanel>
            }
        },
    )
}

#[component]
fn ToolbarButton(handle: ButtonHandle) -> NodeId {
    view! {
        <ButtonFace handle variant=ButtonVariant::Ghost />
    }
}

#[component]
fn ToolbarPanel(children: Child) -> NodeId {
    view! {
        <Frame
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Md
            padding=MENU_PADDING
        >
            {children}
        </Frame>
    }
}

#[component]
fn MenuRow(handle: MenuRowHandle) -> NodeId {
    match handle.kind {
        MenuRowKind::Caption => view! {
            <MenuCaptionFace handle padding=PADDING_HORIZONTAL spacing=MENU_PADDING />
        },
        MenuRowKind::Separator => view! {
            <Separator />
        },
        MenuRowKind::Tile => view! {
            <MenuTileFace handle padding={Padding::symmetric(TILE_PADDING, PADDING_VERTICAL)} />
        },
        MenuRowKind::Row | MenuRowKind::Back => view! {
            <MenuRowFace handle />
        },
    }
}

#[component]
fn MenuRowFace(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        kind,
        label,
        lead,
        glyph,
        glyph_color,
        detail,
        checked,
        radio,
        glyphs,
        shortcut,
        disabled,
        danger,
        separated,
        has_submenu,
        depth,
        expanded,
        toggle,
        hovered,
        focused,
        ..
    } = handle;
    let has_submenu = create_memo(move || has_submenu.get());
    let shortcut = create_memo(move || shortcut.get());
    let has_shortcut = create_memo(move || !shortcut.get().is_empty());
    let disabled = create_memo(move || disabled.get());
    let danger = create_memo(move || danger.get());
    let color = create_memo(move || match (disabled.get(), danger.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Danger,
        (false, false) => TextColor::Default,
    });
    let glyph = leading_mark(kind, glyph, checked, radio);
    let icon_color = create_memo(move || match (disabled.get(), danger.get(), glyph.get()) {
        (true, _, _) => TextColor::Muted,
        (false, true, _) => TextColor::Danger,
        (false, false, LeadingMark::Mark(_)) => TextColor::Accent,
        _ => glyph_color
            .get()
            .map_or(TextColor::Muted, TextColor::Custom),
    });
    let has_glyph = create_memo(move || glyph.get() != LeadingMark::None || glyphs.get());
    let glyph = create_memo(move || glyph.get().glyph());
    let lead = create_memo(move || lead.get());
    let has_lead = create_memo(move || !lead.get().is_empty());
    let detail = create_memo(move || detail.get());
    let has_detail = create_memo(move || !detail.get().is_empty());
    let chevron_color = TextColor::Muted;
    let fill_color = create_memo(move || row_background(focused.get(), hovered.get()));
    view! {
        <List spacing=MENU_PADDING>
            <Show condition={separated}>
                <Separator />
            </Show>
            <Frame
                fill={fill_color}
                radius=Radius::Md
                padding={Padding::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SUBMENU_SPACING>
                    <MenuTreeLead depth expanded toggle indent=TREE_INDENT chevron=TREE_CHEVRON />
                    <Show condition={has_lead}>
                        <Text size=TextSize::Sm color=TextColor::Muted>{lead}</Text>
                    </Show>
                    <Show condition={has_glyph}>
                        <GlyphSlot glyph color={icon_color} />
                    </Show>
                    <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                        <Text color align=TextAlign::Start overflow=Overflow::Ellipsis>
                            {label}
                        </Text>
                        <Show condition={has_detail}>
                            <Text
                                size=TextSize::Sm
                                color=TextColor::Muted
                                align=TextAlign::Start
                                overflow=Overflow::Ellipsis
                            >
                                {detail}
                            </Text>
                        </Show>
                    </List>
                    <Show condition={has_shortcut}>
                        <Text
                            @sizing={ItemSize::Intrinsic.gap(SHORTCUT_SPACING.px())}
                            size=TextSize::Sm
                            color=TextColor::Muted
                            align=TextAlign::End
                        >
                            {shortcut}
                        </Text>
                    </Show>
                    <Show condition={has_submenu}>
                        <Text size=TextSize::Sm color={chevron_color} align=TextAlign::Center>
                            <Icon glyph=ICON_CHEVRON_RIGHT />
                        </Text>
                    </Show>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn GlyphSlot(glyph: Memo<Option<Glyph>>, color: Memo<TextColor>) -> NodeId {
    view! {
        <Frame width={Size::Custom(TextSize::Base.icon_px())}>
            <Text color align=TextAlign::Center>
                <Icon glyph={glyph} />
            </Text>
        </Frame>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LeadingMark {
    None,
    Icon(Glyph),
    Mark(Glyph),
    Unmarked,
}

impl LeadingMark {
    fn glyph(self) -> Option<Glyph> {
        match self {
            LeadingMark::Icon(glyph) | LeadingMark::Mark(glyph) => Some(glyph),
            LeadingMark::None | LeadingMark::Unmarked => None,
        }
    }
}

fn leading_mark(
    kind: MenuRowKind,
    glyph: Prop<Option<Glyph>>,
    checked: Prop<Option<bool>>,
    radio: bool,
) -> Memo<LeadingMark> {
    let tick = match radio {
        true => ICON_DOT,
        false => ICON_CHECK,
    };
    create_memo(move || match (kind, checked.get(), glyph.get()) {
        (MenuRowKind::Back, _, _) => LeadingMark::Icon(ICON_CHEVRON_LEFT),
        (_, Some(true), _) => LeadingMark::Mark(tick),
        (_, Some(false), _) => LeadingMark::Unmarked,
        (_, None, Some(glyph)) => LeadingMark::Icon(glyph),
        (_, None, None) => LeadingMark::None,
    })
}

#[component]
pub(crate) fn MenuTreeLead(
    depth: Prop<Option<usize>>,
    expanded: Prop<Option<bool>>,
    toggle: ClickCallback,
    indent: Space,
    chevron: Size,
) -> DynamicSegment<ListChild> {
    let indent = use_space(indent);
    let depth = create_memo(move || depth.get());
    let in_tree = create_memo(move || depth.get().is_some());
    let expanded = create_memo(move || expanded.get());
    let width =
        create_memo(move || Size::Custom(depth.get().unwrap_or_default() as f32 * indent.get()));
    let expandable = create_memo(move || expanded.get().is_some());
    let leaf = create_memo(move || !expandable.get());
    let glyph = create_memo(move || match expanded.get() {
        Some(true) => Some(ICON_CHEVRON_DOWN),
        Some(false) => Some(ICON_CHEVRON_RIGHT),
        None => None,
    });
    let label = create_memo(move || match expanded.get() {
        Some(true) => "Collapse".to_owned(),
        _ => "Expand".to_owned(),
    });
    view! {
        <Show condition={in_tree}>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                <Frame width />
                <unstyled::Button
                    label
                    disabled={leaf}
                    tab_stop=false
                    press_focus=false
                    on_click={toggle}
                >|_handle: ButtonHandle|
                    <Frame width=chevron align=Align2::CENTER>
                        <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </Frame>
                </unstyled::Button>
            </List>
        </Show>
    }
}

#[component]
fn MenuCaptionFace(handle: MenuRowHandle, padding: Space, spacing: Space) -> NodeId {
    let MenuRowHandle {
        label, separated, ..
    } = handle;
    view! {
        <List spacing>
            <Show condition={separated}>
                <Separator />
            </Show>
            <Frame padding={Padding::symmetric(padding, CAPTION_PADDING_VERTICAL)}>
                <Text
                    size=TextSize::Sm
                    weight=Weight::Bold
                    color=TextColor::Muted
                    align=TextAlign::Start
                    overflow=Overflow::Ellipsis
                >
                    {label}
                </Text>
            </Frame>
        </List>
    }
}

#[component]
fn MenuTileFace(handle: MenuRowHandle, padding: Padding) -> NodeId {
    let MenuRowHandle {
        label,
        glyph,
        glyph_color,
        checked,
        disabled,
        hovered,
        focused,
        ..
    } = handle;
    let disabled = create_memo(move || disabled.get());
    let ticked = create_memo(move || checked.get() == Some(true));
    let color = create_memo(move || match (disabled.get(), ticked.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Accent,
        (false, false) => TextColor::Default,
    });
    let icon_color = create_memo(move || match (disabled.get(), ticked.get()) {
        (true, _) => TextColor::Muted,
        (false, true) => TextColor::Accent,
        (false, false) => glyph_color
            .get()
            .map_or(TextColor::Default, TextColor::Custom),
    });
    let fill_color = create_memo(move || row_background(focused.get(), hovered.get()));
    let border = create_memo(move || ticked.get().then_some(Border::new(ColorRole::Accent)));
    view! {
        <Frame fill={fill_color} border radius=Radius::Md padding>
            <List align=Align::Center spacing=TILE_SPACING>
                <Text size=TextSize::Xl color={icon_color} align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Text size=TextSize::Sm color align=TextAlign::Center overflow=Overflow::Ellipsis>
                    {label}
                </Text>
            </List>
        </Frame>
    }
}

#[component]
fn MenuSheetRow(handle: MenuRowHandle) -> NodeId {
    match handle.kind {
        MenuRowKind::Caption => view! {
            <MenuCaptionFace handle padding=SHEET_CAPTION_PADDING spacing=SHEET_SEPARATOR_SPACING />
        },
        MenuRowKind::Separator => view! {
            <Separator />
        },
        MenuRowKind::Tile => view! {
            <MenuTileFace handle padding={Padding::symmetric(TILE_PADDING, SHEET_TILE_PADDING)} />
        },
        MenuRowKind::Row | MenuRowKind::Back => view! {
            <MenuSheetRowFace handle />
        },
    }
}

#[component]
fn MenuSheetRowFace(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        kind,
        label,
        lead,
        glyph,
        glyph_color,
        detail,
        checked,
        radio,
        glyphs,
        disabled,
        danger,
        separated,
        has_submenu,
        depth,
        expanded,
        toggle,
        hovered,
        active,
        focused,
        ..
    } = handle;
    let mark = leading_mark(kind, glyph, checked, radio);
    let leading = create_memo(move || glyphs.get() || mark.get() != LeadingMark::None);
    let glyph_color = create_memo(move || match mark.get() {
        LeadingMark::Mark(_) => Some(TextColor::Accent),
        _ => glyph_color.get().map(TextColor::Custom),
    });
    let glyph = create_memo(move || mark.get().glyph());
    let trailing = create_memo(move || match has_submenu.get() {
        true => RowTrailing::Chevron,
        false => RowTrailing::None,
    });
    view! {
        <List spacing=SHEET_SEPARATOR_SPACING>
            <Show condition={separated}>
                <Separator />
            </Show>
            <RowFace
                hovered
                active
                focused
                disabled
                danger
                size=RowSize::Touch
                label
                lead
                glyph
                glyph_color
                detail
                leading
                trailing={trailing}
                depth
                expanded
                toggle
            />
        </List>
    }
}

#[component]
fn MenuPanel(min_width: Size, children: Child) -> NodeId {
    let min_width = use_size(min_width).get_untracked();
    view! {
        <List direction=Direction::Horizontal spacing=Space::None>
            <Frame
                @sizing={ItemSize::Intrinsic.min(min_width).max(MENU_MAX_WIDTH).shrink(1.0)}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Md
            >
                <List spacing=Space::None>
                    <unstyled::Scroll
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        fit=true
                        scrollbar={ScrollbarStyle::default().fading(SCROLL_FADE)}
                    >
                        <Frame padding=MENU_PADDING>{children}</Frame>
                    </unstyled::Scroll>
                </List>
            </Frame>
        </List>
    }
}

fn row_background(focused: bool, hovered: bool) -> ColorRole {
    match (focused, hovered) {
        (true, _) => ColorRole::AccentSoft,
        (false, true) => ColorRole::Pressed,
        (false, false) => ColorRole::None,
    }
}
