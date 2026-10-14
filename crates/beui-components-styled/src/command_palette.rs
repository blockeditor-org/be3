use beui_macros::{component, view};

use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::text_input::text_input_style;
use crate::theme::{BORDER_WIDTH, CARD_RADIUS, RADIUS, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{CommandRowHandle, TextInputHandle};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Child, ClickCallback, Direction, Frame, ItemSize, List, Prop, create_memo,
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
    view! {
        <unstyled::CommandPalette
            open
            on_close={on_close}
            list_height=LIST_HEIGHT
            scrim=SCRIM
            search_placeholder="Run a command"
            search_content={|handle| view! {
                <PaletteSearch handle />
            }}
            search_style={text_input_style()}
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
                color={theme.surface_raised}
                outline={theme.border}
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
            color={theme.surface}
            outline={theme.accent}
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
    let fill = create_memo(
        move || match (disabled.get(), highlighted.get(), hovered.get()) {
            (true, _, _) | (false, false, false) => Color32::TRANSPARENT,
            (false, true, _) => theme.accent_soft.get(),
            (false, false, true) => theme.surface.get(),
        },
    );
    let text_color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    let icon_color = text_color;
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
                    <Text color={icon_color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={label}
                    color={text_color}
                    align=TextAlign::Start
                />
                <Text
                    string={shortcut}
                    size=TextSize::Sm
                    color=TextColor::Muted
                    align=TextAlign::End
                />
            </List>
        </Frame>
    }
}
