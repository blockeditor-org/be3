use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_macros::{component, view};

use crate::button::ButtonSize;
use crate::button::{Button, ButtonVariant};
use crate::text::{Icon, Text, TextColor, TextSize, Weight};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    Edge, Picture, Pressable, TOAST_DURATION, Toast, ToastAction, ToastHandle,
};
use beui_core::base::Justify;
use beui_core::base::overlay::OverlayAnchor;
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_CIRCLE_ALERT, ICON_INFO, ICON_X};
use beui_core::image::Image;
use beui_core::node::NodeId;
use crate::list::List;
use crate::tokens::{Padding, Size, Space, use_space};
use beui_view::reactive::*;
use std::time::Duration;

const MARGIN: Space = Space::Xl;
const WIDTH: Size = Size::Dialog;
const SPACING: Space = Space::Md;
const PADDING: Padding = Padding::symmetric(Space::Lg, Space::Md);
const PICTURE_SIZE: Size = Size::Lg;
const TEXT_SPACING: Space = Space::Xs2;

#[component]
pub fn Toasts(
    anchor: Prop<OverlayAnchor>,
    toasts: Prop<Vec<Toast>>,
    #[prop(default = Some(TOAST_DURATION))] duration: Prop<Option<Duration>>,
    #[prop(default = Edge::BottomEnd)] edge: Prop<Edge>,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
) -> NodeId {
    view! {
        <unstyled::Toasts
            anchor
            toasts
            duration
            edge
            width={WIDTH.px()}
            margin={use_space(MARGIN)}
            spacing={use_space(SPACING)}
            on_dismiss
            on_action
            on_activate
        >|handle: ToastHandle|
            <ToastCard handle />
        </unstyled::Toasts>
    }
}

#[component]
fn ToastCard(handle: ToastHandle) -> NodeId {
    let ToastHandle {
        toast,
        dismiss,
        activate,
        act,
    } = handle;
    let Toast {
        id,
        title,
        message,
        danger,
        image,
        actions,
        activates,
        sticky: _,
    } = toast;
    let (glyph, tint) = match danger {
        true => (ICON_CIRCLE_ALERT, TextColor::Danger),
        false => (ICON_INFO, TextColor::Accent),
    };
    let pictured = image.is_some();
    let titled = !title.is_empty();
    let said = !message.is_empty();
    let acted = !actions.is_empty();
    let (message_size, message_color) = match titled {
        true => (TextSize::Sm, TextColor::Muted),
        false => (TextSize::Base, TextColor::Default),
    };
    let title_color = TextColor::Default;
    let content = view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Show condition={pictured}>
                <ToastPicture image={image.clone()} />
            </Show>
            <Show condition={!pictured}>
                <Text color={tint} align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
            </Show>
            <List @sizing=ItemSize::Percent(100.0) spacing=TEXT_SPACING>
                <Show condition={titled}>|| {
                    let title = title.clone(); view! {
                        <Text color={title_color} weight=Weight::Bold>{title}</Text>
                    }
                }</Show>
                <Show condition={said}>|| {
                    let message = message.clone(); view! {
                        <Text size={message_size} color={message_color}>{message}</Text>
                    }
                }</Show>
            </List>
        </List>
    };
    let body = match activates {
        true => view! {
            <Pressable @test_id={format!("toast.{id}.activate")} on_click={activate}>
                {content}
            </Pressable>
        },
        false => content,
    };
    view! {
        <Frame
            @test_id={format!("toast.{id}")}
            fill=ColorRole::SurfaceRaised
            border={Border::DEFAULT}
            radius=Radius::Md
            padding=PADDING
        >
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                    <Frame @sizing=ItemSize::Percent(100.0)>{body}</Frame>
                    <Button
                        icon_only=true
                        @test_id={format!("toast.{id}.dismiss")}
                        glyph=ICON_X
                        label="Dismiss"
                        size=ButtonSize::Small
                        press_focus=false
                        on_click={dismiss}
                    />
                </List>
                <Show condition={acted}>
                    <ToastActions id actions={actions.clone()} act />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ToastPicture(image: Option<Image>) -> NodeId {
    view! {
        <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
            <Picture image />
        </Frame>
    }
}

#[component]
fn ToastActions(id: u64, actions: Vec<ToastAction>, act: Callback<String>) -> NodeId {
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            justify=Justify::End
            spacing=SPACING
        >
            <ForEach keys={actions}>|action: ToastAction| {
                let key = action.key.clone();
                view! {
                    <Button
                        @test_id={format!("toast.{id}.action.{}", action.key)}
                        label={action.label}
                        variant=ButtonVariant::Secondary
                        on_click={move || act.call(key.clone())}
                    />
                }
            }</ForEach>
        </List>
    }
}
