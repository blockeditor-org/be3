use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::font::Overflow;
use beui_macros::{component, view};

use crate::context_menu::MenuCheck;
use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::text_input::text_input_style;
use crate::theme::use_theme;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{CommandRowHandle, TextInputHandle};
use beui_core::base::TextAlign;
use beui_core::color::Color32;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const WIDTH: f32 = 520.0;
const LIST_HEIGHT: f32 = 320.0;
const MARGIN: f32 = 12.0;
const PADDING: f32 = 6.0;
const SEARCH_HEIGHT: f32 = 36.0;
const PADDING_HORIZONTAL: f32 = 10.0;
const ROW_PADDING_VERTICAL: f32 = 7.0;
const ROW_SPACING: f32 = 10.0;
const ICON_WIDTH: f32 = 20.0;

#[component]
pub fn CommandPalette(open: Binding<bool>) -> NodeId {
    let theme = use_theme();
    let scrim = create_memo(move || theme.scrim_soft.get());
    view! {
        <unstyled::CommandPalette
            open
            list_height=LIST_HEIGHT
            scrim={scrim}
            search_placeholder="Run a command"
            search_content={|handle| view! {
                <PaletteSearch handle />
            }}
            search_style={text_input_style()}
            scrollbar={scrollbar_style()}
            row={|handle| view! {
                <PaletteRow handle />
            }}
        >|content|
            <PalettePanel>{content}</PalettePanel>
        </unstyled::CommandPalette>
    }
}

#[component]
fn PalettePanel(children: Child) -> NodeId {
    view! {
        <Frame padding=MARGIN>
            <Frame
                width={Extent::at_most(WIDTH)}
                fill=ColorRole::SurfaceRaised
                border={Border::DEFAULT}
                radius=Radius::Lg
                padding=PADDING
            >
                {children}
            </Frame>
        </Frame>
    }
}

#[component]
fn PaletteSearch(handle: TextInputHandle) -> NodeId {
    let TextInputHandle { field, .. } = handle;
    view! {
        <Frame
            height=SEARCH_HEIGHT
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Accent)}
            radius=Radius::Md
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
        checked,
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
    let shortcut = shortcut.unwrap_or_default();
    view! {
        <Frame
            fill={ColorRole::custom(fill)}
            radius=Radius::Md
            padding={Sides::symmetric(PADDING_HORIZONTAL, ROW_PADDING_VERTICAL)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                <Frame width=ICON_WIDTH>
                    <Text color={text_color} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
                <Text
                    overflow=Overflow::Ellipsis
                    @sizing=ItemSize::Percent(100.0)
                    color={text_color}
                    align=TextAlign::Start
                >
                    {label}
                </Text>
                <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::End>
                    {shortcut}
                </Text>
                <MenuCheck checked />
            </List>
        </Frame>
    }
}
