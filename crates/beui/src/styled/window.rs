use beui_macros::{component, view};

use crate::base::{Align, Direction, ItemSize};
use crate::geometry::{Pos2, Vec2, pos2, vec2};
use crate::icons::ICON_CLOSE;
use crate::input::{CursorIcon, PointerPress};
use crate::node::NodeId;
use crate::reactive::{Callback, Child, ClickCallback, ClickCatcher, Frame, List, Prop};
use crate::styled::border::Separator;
use crate::styled::icon_button::{IconButton, IconButtonSize};
use crate::styled::text::Heading;
use crate::styled::theme::{BORDER_WIDTH, CARD_RADIUS, use_theme};
use crate::unstyled::{self, WindowHandle};

const BAR_PADDING_HORIZONTAL: f32 = 12.0;
const BAR_PADDING_VERTICAL: f32 = 6.0;
const BODY_PADDING_HORIZONTAL: f32 = 12.0;
const BODY_PADDING_VERTICAL: f32 = 10.0;

#[component]
pub fn Window(
    open: Prop<bool>,
    title: Prop<String>,
    #[prop(default = pos2(96.0, 96.0))] position: Pos2,
    #[prop(default = vec2(480.0, 360.0))] size: Vec2,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    view! {
        <unstyled::Window
            open
            position
            size
            content={move |handle: WindowHandle| {
                let WindowHandle { grab, drag } = handle;
                view! {
                    <WindowSurface
                        title
                        grab={move |press: PointerPress| grab.call(press)}
                        drag={move |press: PointerPress| drag.call(press)}
                        on_close={move || on_close.call()}
                    >
                        {children}
                    </WindowSurface>
                }
            }}
        />
    }
}

#[component]
fn WindowSurface(
    title: Prop<String>,
    grab: Callback<PointerPress>,
    drag: Callback<PointerPress>,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=CARD_RADIUS
        >
            <List spacing=0.0>
                <ClickCatcher
                    cursor=CursorIcon::Grab
                    on_press={move |press: PointerPress| grab.call(press)}
                    on_drag={move |press: PointerPress| drag.call(press)}
                >
                    <Frame
                        padding_horizontal=BAR_PADDING_HORIZONTAL
                        padding_vertical=BAR_PADDING_VERTICAL
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Heading @sizing=ItemSize::Percent(100.0) content={title} />
                            <IconButton
                                glyph={ICON_CLOSE.to_owned()}
                                label="Close"
                                size=IconButtonSize::Compact
                                on_click={move || on_close.call()}
                            />
                        </List>
                    </Frame>
                </ClickCatcher>
                <Separator />
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    padding_horizontal=BODY_PADDING_HORIZONTAL
                    padding_vertical=BODY_PADDING_VERTICAL
                >
                    {children}
                </Frame>
            </List>
        </Frame>
    }
}
