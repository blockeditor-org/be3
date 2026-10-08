use beui_macros::{component, view};

use crate::icon_button::{IconButton, IconButtonSize};
use crate::text::Icon;
use crate::theme::{BORDER_WIDTH, FONT_BODY, RADIUS, use_theme};
use beui_components_unstyled::{Edge, Floating};
use beui_core::icons::{ICON_CLOSE, ICON_ERROR, ICON_INFO};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, List, NodeRef, Prop, Text, create_memo,
    create_timer,
};
use std::time::Duration;

pub const TOAST_DURATION: Duration = Duration::from_secs(8);
const MARGIN: f32 = 16.0;
const WIDTH: f32 = 360.0;
const SPACING: f32 = 8.0;
const PADDING_HORIZONTAL: f32 = 12.0;
const PADDING_VERTICAL: f32 = 10.0;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub danger: bool,
}

#[component]
pub fn Toasts(
    anchor: NodeRef,
    toasts: Prop<Vec<Toast>>,
    #[prop(default = Some(TOAST_DURATION))] duration: Prop<Option<Duration>>,
    on_dismiss: Callback<u64>,
) -> NodeId {
    let listed = toasts.clone();
    let open = create_memo(move || !listed.get().is_empty());
    view! {
        <Floating anchor edge=Edge::BottomEnd open={open}>
            <Frame
                padding_horizontal=MARGIN
                padding_vertical=MARGIN
                width=Some(WIDTH + 2.0 * MARGIN)
            >
                <List spacing=SPACING>
                    <ForEach keys={toasts}>
                        {move |toast: Toast| {
                            let on_dismiss = on_dismiss.clone();
                            view! {
                                <ToastCard
                                    toast
                                    duration={duration.peek()}
                                    on_dismiss={move |id: u64| on_dismiss.call(id)}
                                />
                            }
                        }}
                    </ForEach>
                </List>
            </Frame>
        </Floating>
    }
}

#[component]
fn ToastCard(toast: Toast, duration: Option<Duration>, on_dismiss: Callback<u64>) -> NodeId {
    let theme = use_theme();
    let Toast {
        id,
        message,
        danger,
    } = toast;
    let expire = on_dismiss.clone();
    let expiry = create_timer(move || {
        expire.call(id);
        None
    });
    if let Some(duration) = duration {
        expiry.start(duration);
    }
    let (glyph, tint) = match danger {
        true => (ICON_ERROR, theme.danger.clone()),
        false => (ICON_INFO, theme.accent.clone()),
    };
    view! {
        <Frame
            @test_id={format!("toast.{id}")}
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Icon glyph={glyph.to_owned()} color={tint} />
                <Text
                    @sizing=ItemSize::Percent(100.0)
                    string={message}
                    font_size=FONT_BODY
                    color={theme.text.clone()}
                    wrap=true
                />
                <IconButton
                    @test_id={format!("toast.{id}.dismiss")}
                    glyph={ICON_CLOSE.to_owned()}
                    label="Dismiss"
                    size=IconButtonSize::Compact
                    press_focus=false
                    on_click={move || on_dismiss.call(id)}
                />
            </List>
        </Frame>
    }
}
