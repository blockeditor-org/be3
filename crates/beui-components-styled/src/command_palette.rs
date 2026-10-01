use beui_macros::{component, view};

use crate::context_menu::text_menu;
use crate::scroll::scrollbar_style;
use crate::text::Icon;
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, FONT_BODY, FONT_SMALL, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{CommandRowHandle, TextInputHandle};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Child, ClickCallback, Direction, Frame, ItemSize, List, Prop, Text, clone, create_memo,
};

const WIDTH: f32 = 520.0;
const LIST_HEIGHT: f32 = 320.0;
const MARGIN: f32 = 12.0;
const PADDING: f32 = 6.0;
const SEARCH_HEIGHT: f32 = 36.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const ROW_PADDING_VERTICAL: f32 = 7.0;
const ROW_SPACING: f32 = 10.0;
const ICON_WIDTH: f32 = 20.0;
const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 90);

#[component]
pub fn CommandPalette(open: Prop<bool>, on_close: ClickCallback) -> NodeId {
    let theme = use_theme();
    view! {
        <unstyled::CommandPalette
            open
            on_close={move || on_close.call()}
            list_height=LIST_HEIGHT
            scrim=SCRIM
            search_placeholder="Run a command"
            search_font_size=FONT_BODY
            search_color={theme.text.clone()}
            search_placeholder_color={theme.text_muted.clone()}
            search_selection_color={theme.accent_soft.clone()}
            search_caret_color={theme.accent.clone()}
            search_padding_horizontal=PADDING_HORIZONTAL
            search_content={|handle| view! {
                <PaletteSearch handle />
            }}
            search_menu={text_menu()}
            scrollbar={scrollbar_style()}
            row={|handle| view! {
                <PaletteRow handle />
            }}
        >
            {|content| view! {
                <PalettePanel>{content}</PalettePanel>
            }}
        </unstyled::CommandPalette>
    }
}

#[component]
fn PalettePanel(children: Child) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=MARGIN padding_vertical=MARGIN>
            <Frame
                max_width=WIDTH
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=CARD_RADIUS
                padding_horizontal=PADDING
                padding_vertical=PADDING
            >
                {children}
            </Frame>
        </Frame>
    }
}

#[component]
fn PaletteSearch(handle: TextInputHandle) -> NodeId {
    let TextInputHandle { field, .. } = handle;
    let theme = use_theme();
    view! {
        <Frame
            height=SEARCH_HEIGHT
            color={theme.surface.clone()}
            outline={theme.accent.clone()}
            outline_width=BORDER_WIDTH
            radius=RADIUS
            outline_visible=true
        >
            {field}
        </Frame>
    }
}

#[component]
fn PaletteRow(handle: CommandRowHandle) -> NodeId {
    let CommandRowHandle {
        label,
        glyph,
        shortcut,
        disabled,
        highlighted,
        hovered,
    } = handle;
    let theme = use_theme();
    let fill = create_memo(clone!(theme disabled -> move || {
        match (disabled.get(), highlighted.get(), hovered.get()) {
            (true, _, _) | (false, false, false) => Color32::TRANSPARENT,
            (false, true, _) => theme.accent_soft.get(),
            (false, false, true) => theme.surface.get(),
        }
    }));
    let text_color = create_memo(clone!(theme disabled -> move || match disabled.get() {
        true => theme.text_muted.get(),
        false => theme.text.get(),
    }));
    let icon_color = text_color.clone();
    let shortcut = shortcut.unwrap_or_default();
    view! {
        <Frame
            color={fill}
            radius=RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=ROW_PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame width=ICON_WIDTH>
                    <Icon glyph={glyph} color={icon_color} />
                </Frame>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={label}
                    font_size=FONT_BODY
                    color={text_color}
                    align=TextAlign::Start
                />
                <Text
                    string={shortcut}
                    font_size=FONT_SMALL
                    color={theme.text_muted.clone()}
                    align=TextAlign::End
                />
            </List>
        </Frame>
    }
}
