use crate::frame::{Border, ColorRole, Frame, Radius};
use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::button::ButtonSize;
use crate::button::{Button, ButtonVariant};
use crate::list::List;
use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor, TextSize, Weight};
use crate::tokens::{Padding, Size, Space, use_space};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    Edge, Picture, Pressable, TOAST_DURATION, Toast, ToastAction, ToastHandle,
};
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::Justify;
use beui_view::types::TextAlign;
use beui_view::types::icons::{ICON_CIRCLE_ALERT, ICON_INFO, ICON_X};
use beui_view::types::{Align, Direction};
use beui_view::types::{Overflow, WrapAt};
use std::time::Duration;

const MARGIN: Space = Space::Xl;
const WIDTH: Size = Size::Dialog;
const SPACING: Space = Space::Md;
const PADDING: Padding = Padding::symmetric(Space::Lg, Space::Md);
const PICTURE_SIZE: Size = Size::Lg;
const TEXT_SPACING: Space = Space::Xs2;
const SOURCE_LINES: u32 = 1;
const TITLE_LINES: u32 = 2;
const MESSAGE_LINES: u32 = 4;

fn clamped(lines: u32) -> Overflow {
    Overflow::Wrap {
        at: WrapAt::Words,
        max_lines: Some(lines),
    }
}

#[component]
pub fn Toasts(
    anchor: Prop<OverlayAnchor>,
    toasts: Prop<Vec<Toast>>,
    #[prop(default = Some(TOAST_DURATION))] duration: Prop<Option<Duration>>,
    #[prop(default = Edge::BottomEnd)] edge: Prop<Edge>,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    on_more: ClickCallback,
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
            scrollbar={scrollbar_style()}
            on_dismiss
            on_action
            on_activate
            more={move |hidden: Memo<usize>| view! {
                <ToastsHidden hidden on_more />
            }}
        >|handle: ToastHandle|
            <ToastCard handle />
        </unstyled::Toasts>
    }
}

#[component]
fn ToastsHidden(hidden: Memo<usize>, on_more: ClickCallback) -> NodeId {
    let label = create_memo(move || format!("{} more", hidden.get()));
    let card = view! {
        <Frame
            @test_id="toasts.more"
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Md
            padding=PADDING
        >
            <Text size=TextSize::Sm color=TextColor::Muted align=TextAlign::Center>{label}</Text>
        </Frame>
    };
    match on_more.is_empty() {
        true => card,
        false => view! {
            <Pressable @test_id="toasts.more.open" on_click={on_more}>{card}</Pressable>
        },
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
        source,
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
    let sourced = !source.is_empty();
    let titled = !title.is_empty();
    let said = !message.is_empty();
    let acted = !actions.is_empty();
    let (message_size, message_color) = match titled {
        true => (TextSize::Sm, TextColor::Muted),
        false => (TextSize::Base, TextColor::Default),
    };
    let title_color = TextColor::Default;
    let mut named = Node::new(Role::Button);
    match titled {
        true => {
            named.set_label(title.clone());
            if said {
                named.set_description(message.clone());
            }
        }
        false => named.set_label(message.clone()),
    }
    let content = view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Show condition={pictured}>
                <ToastPicture image={image.clone()} />
            </Show>
            <Show condition={!pictured}>
                <Frame width=PICTURE_SIZE>
                    <Text color={tint} align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Frame>
            </Show>
            <ListItem grow=1.0>
                <List spacing=TEXT_SPACING>
                    <Show condition={sourced}>|| {
                        let source = source.clone(); view! {
                            <Text
                                size=TextSize::Xs
                                color=TextColor::Muted
                                overflow={clamped(SOURCE_LINES)}
                            >
                                {source}
                            </Text>
                        }
                    }</Show>
                    <Show condition={titled}>|| {
                        let title = title.clone(); view! {
                            <Text
                                color={title_color}
                                weight=Weight::Bold
                                overflow={clamped(TITLE_LINES)}
                            >
                                {title}
                            </Text>
                        }
                    }</Show>
                    <Show condition={said}>|| {
                        let message = message.clone(); view! {
                            <Text
                                size={message_size}
                                color={message_color}
                                overflow={clamped(MESSAGE_LINES)}
                            >
                                {message}
                            </Text>
                        }
                    }</Show>
                </List>
            </ListItem>
        </List>
    };
    let body = match activates {
        true => view! {
            <Pressable
                @test_id={format!("toast.{id}.activate")}
                accessibility={named}
                on_click={activate}
            >
                {content}
            </Pressable>
        },
        false => content,
    };
    view! {
        <Frame
            @test_id={format!("toast.{id}")}
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Md
            padding=PADDING
        >
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                    <ListItem grow=1.0>
                        <Frame>{body}</Frame>
                    </ListItem>
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
                        press_focus=false
                        on_click={move || act.call(key.clone())}
                    />
                }
            }</ForEach>
        </List>
    }
}
