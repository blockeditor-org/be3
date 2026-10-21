use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::action_row::ActionRowFace;
use crate::border::Separator;
use crate::button::{ButtonFace, ButtonVariant};
use crate::sheet::ModalSheet;
use crate::text::{Icon, Text, TextColor, TextSize, Weight};
use crate::theme::{SCROLL_FADE, ThemeStore, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    ButtonHandle, MenuItem, MenuRowHandle, MenuRowKind, MenuSheetHandle, MenuStyle, ScrollbarStyle,
};
use beui_core::base::TextAlign;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::{Pos2, Rect};
use beui_core::icons::{
    ICON_CHECK, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_KEYBOARD_ARROW_DOWN,
    ICON_KEYBOARD_ARROW_RIGHT,
};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PADDING_HORIZONTAL: f32 = 14.0;
const PADDING_VERTICAL: f32 = 6.0;
const MENU_PADDING: f32 = 4.0;
const MENU_WIDTH: f32 = 200.0;
const SUBMENU_SPACING: f32 = 8.0;
const SHEET_PADDING: f32 = 8.0;
const SHEET_STOPS: [f32; 2] = [0.5, 0.9];
const SHEET_SEPARATOR_SPACING: f32 = 4.0;
const SHEET_CAPTION_PADDING: f32 = 12.0;
const SHEET_TILE_PADDING: f32 = 14.0;
const CAPTION_PADDING_VERTICAL: f32 = 4.0;
const TILE_PADDING: f32 = 6.0;
const TILE_SPACING: f32 = 4.0;
const TREE_INDENT: f32 = 12.0;
const TREE_CHEVRON: f32 = 16.0;
pub(crate) const SHEET_TREE_INDENT: f32 = 20.0;
pub(crate) const SHEET_TREE_CHEVRON: f32 = 28.0;
const MENU_MAX_HEIGHT: f32 = 420.0;

#[component]
pub fn ContextMenu(
    children: Child,
    items: Children<MenuItem>,
    #[prop(default = ItemSize::Intrinsic)] child_size: Prop<ItemSize>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = None)] open_at: Prop<Option<Pos2>>,
    #[prop(default = true)] open_at_focuses: bool,
    #[prop(default = false)] open_at_pointer: Prop<bool>,
    #[prop(default = false)] selection: bool,
    #[prop(default = None)] toolbar_at: Prop<Option<Rect>>,
    #[prop(default = true)] opens_on_hold: bool,
    on_close: ClickCallback,
    on_select: Callback<Vec<usize>>,
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
            open_at_pointer
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
    #[prop(default = MENU_WIDTH)] width: f32,
    on_dismiss: ClickCallback,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let open = create_memo(move || open.get());
    let sheet = create_memo(move || sheet.get());
    view! {
        <unstyled::MenuPopup
            items={items.into_run()}
            menu={sized_menu_style(width)}
            anchor
            placement
            open={open}
            touched={sheet}
            on_dismiss={on_dismiss}
            on_select={on_select}
        />
    }
}

pub fn menu_style() -> MenuStyle {
    sized_menu_style(MENU_WIDTH)
}

fn sized_menu_style(width: f32) -> MenuStyle {
    MenuStyle::new(
        |handle| {
            view! {
                <MenuRow handle />
            }
        },
        move |content| {
            view! {
                <MenuPanel width>{content}</MenuPanel>
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
                <MenuSheetPanel handle />
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
    let glyph = handle.glyph;
    let icon_only = create_memo(move || !glyph.get().is_empty());
    view! {
        <ButtonFace handle variant=ButtonVariant::Ghost icon_only />
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
            <MenuTileFace handle padding={Sides::symmetric(TILE_PADDING, PADDING_VERTICAL)} />
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
        detail,
        checked,
        depth,
        expanded,
        toggle,
        state,
        ..
    } = handle;
    let theme = use_theme();
    let color = create_memo(move || {
        let state = state.get();
        match (state.disabled, state.danger) {
            (true, _) => TextColor::Muted,
            (false, true) => TextColor::Danger,
            (false, false) => TextColor::Default,
        }
    });
    let icon_color = create_memo(move || {
        let state = state.get();
        match (state.disabled, state.danger) {
            (false, true) => TextColor::Danger,
            _ => TextColor::Muted,
        }
    });
    let separated = create_memo(move || state.get().separated);
    let has_submenu = create_memo(move || state.get().has_submenu);
    let glyph = create_memo(move || match kind {
        MenuRowKind::Back => ICON_CHEVRON_LEFT.to_owned(),
        _ => glyph.get(),
    });
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let lead = create_memo(move || lead.get());
    let has_lead = create_memo(move || !lead.get().is_empty());
    let detail = create_memo(move || detail.get());
    let has_detail = create_memo(move || !detail.get().is_empty());
    let chevron_color = TextColor::Muted;
    let fill_color = create_memo(move || {
        let state = state.get();
        row_background(&theme, state.focused, state.hovered)
    });
    view! {
        <List spacing=MENU_PADDING>
            <Show condition={separated}>
                <Separator />
            </Show>
            <Frame
                fill={ColorRole::custom(fill_color)}
                radius=Radius::Md
                padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=SUBMENU_SPACING>
                    <MenuTreeLead depth expanded toggle indent=TREE_INDENT chevron=TREE_CHEVRON />
                    <Show condition={has_lead}>
                        <Text string={lead} size=TextSize::Sm color=TextColor::Muted />
                    </Show>
                    <Show condition={has_glyph}>
                        <Text color={icon_color} align=TextAlign::Center>
                            <Icon glyph={glyph} />
                        </Text>
                    </Show>
                    <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                        <Text string={label} color align=TextAlign::Start ellipsis=true />
                        <Show condition={has_detail}>
                            <Text
                                string={detail}
                                size=TextSize::Sm
                                color=TextColor::Muted
                                align=TextAlign::Start
                                ellipsis=true
                            />
                        </Show>
                    </List>
                    <MenuCheck checked />
                    <Frame visible={has_submenu}>
                        <Text size=TextSize::Sm color={chevron_color} align=TextAlign::Center>
                            <Icon glyph=ICON_CHEVRON_RIGHT />
                        </Text>
                    </Frame>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn MenuCheck(checked: Prop<Option<bool>>) -> DynamicSegment<ListChild> {
    let ticked = create_memo(move || checked.get() == Some(true));
    view! {
        <Show condition={ticked}>
            <Text size=TextSize::Sm color=TextColor::Accent align=TextAlign::Center>
                <Icon glyph=ICON_CHECK />
            </Text>
        </Show>
    }
}

#[component]
pub(crate) fn MenuTreeLead(
    depth: Prop<Option<usize>>,
    expanded: Prop<Option<bool>>,
    toggle: ClickCallback,
    indent: f32,
    chevron: f32,
) -> DynamicSegment<ListChild> {
    let depth = create_memo(move || depth.get());
    let in_tree = create_memo(move || depth.get().is_some());
    let expanded = create_memo(move || expanded.get());
    let width = create_memo(move || Extent::px(depth.get().unwrap_or_default() as f32 * indent));
    let expandable = create_memo(move || expanded.get().is_some());
    let leaf = create_memo(move || !expandable.get());
    let glyph = create_memo(move || match expanded.get() {
        Some(true) => ICON_KEYBOARD_ARROW_DOWN.to_owned(),
        Some(false) => ICON_KEYBOARD_ARROW_RIGHT.to_owned(),
        None => String::new(),
    });
    let label = create_memo(move || match expanded.get() {
        Some(true) => "Collapse".to_owned(),
        _ => "Expand".to_owned(),
    });
    view! {
        <Show condition={in_tree}>
            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
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
fn MenuCaptionFace(handle: MenuRowHandle, padding: f32, spacing: f32) -> NodeId {
    let MenuRowHandle { label, state, .. } = handle;
    let separated = create_memo(move || state.get().separated);
    view! {
        <List spacing>
            <Show condition={separated}>
                <Separator />
            </Show>
            <Frame padding={Sides::symmetric(padding, CAPTION_PADDING_VERTICAL)}>
                <Text
                    string={label}
                    size=TextSize::Sm
                    weight=Weight::Bold
                    color=TextColor::Muted
                    align=TextAlign::Start
                    ellipsis=true
                />
            </Frame>
        </List>
    }
}

#[component]
fn MenuTileFace(handle: MenuRowHandle, padding: Sides) -> NodeId {
    let MenuRowHandle {
        label,
        glyph,
        state,
        ..
    } = handle;
    let theme = use_theme();
    let color = create_memo(move || match state.get().disabled {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let fill_color = create_memo(move || {
        let state = state.get();
        row_background(&theme, state.focused, state.hovered)
    });
    view! {
        <Frame fill={ColorRole::custom(fill_color)} radius=Radius::Md padding>
            <List align=Align::Center spacing=TILE_SPACING>
                <Text size=TextSize::Xl color align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Text
                    string={label}
                    size=TextSize::Sm
                    color
                    align=TextAlign::Center
                    ellipsis=true
                />
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
            <MenuTileFace handle padding={Sides::symmetric(TILE_PADDING, SHEET_TILE_PADDING)} />
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
        detail,
        checked,
        depth,
        expanded,
        toggle,
        state,
    } = handle;
    let separated = create_memo(move || state.get().separated);
    let glyph = match kind {
        MenuRowKind::Back => Prop::Static(ICON_CHEVRON_LEFT.to_owned()),
        _ => glyph,
    };
    view! {
        <List spacing=SHEET_SEPARATOR_SPACING>
            <Show condition={separated}>
                <Separator />
            </Show>
            <ActionRowFace
                hovered={Prop::dynamic(move || state.get().hovered)}
                active={Prop::dynamic(move || state.get().active)}
                focused={Prop::dynamic(move || state.get().focused)}
                label
                lead
                glyph
                detail
                disabled={Prop::dynamic(move || state.get().disabled)}
                danger={Prop::dynamic(move || state.get().danger)}
                submenu={Prop::dynamic(move || state.get().has_submenu)}
                checked
                depth
                expanded
                toggle
            />
        </List>
    }
}

#[component]
fn MenuSheetPanel(handle: MenuSheetHandle) -> NodeId {
    let MenuSheetHandle {
        open,
        on_close,
        menu,
    } = handle;
    view! {
        <ModalSheet
            open={open}
            rest={SHEET_STOPS[0]}
            stops={SHEET_STOPS.to_vec()}
            fit=true
            on_close={on_close}
        >
            <Frame padding=SHEET_PADDING>{menu}</Frame>
        </ModalSheet>
    }
}

#[component]
fn MenuPanel(width: f32, children: Child) -> NodeId {
    view! {
        <Frame width fill=ColorRole::SurfaceRaised border={Border::DEFAULT} radius=Radius::Md>
            <List spacing=0.0>
                <unstyled::Scroll
                    @sizing={ItemSize::Intrinsic.max(MENU_MAX_HEIGHT).shrink(1.0)}
                    fit=true
                    tab_stop=false
                    scrollbar={ScrollbarStyle::default().fading(SCROLL_FADE)}
                >
                    <Frame padding=MENU_PADDING>{children}</Frame>
                </unstyled::Scroll>
            </List>
        </Frame>
    }
}

fn row_background(theme: &ThemeStore, focused: bool, hovered: bool) -> Color32 {
    match (focused, hovered) {
        (true, _) => theme.accent_soft.get(),
        (false, true) => theme.pressed.get(),
        (false, false) => Color32::TRANSPARENT,
    }
}
