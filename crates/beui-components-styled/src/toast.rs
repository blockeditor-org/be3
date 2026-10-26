use crate::frame::{Border, ColorRole, Frame, Radius};
use beui_core::layer::{LayerLevel, LayerPolicy};
use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::icon_button::{IconButton, IconButtonSize};
use crate::text::{Icon, Text, TextColor, TextSize, Weight};
use beui_components_unstyled::{Edge, Picture, Pressable};
use beui_core::base::Justify;
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::font::TextAlign;
use beui_core::icons::{ICON_CIRCLE_ALERT, ICON_INFO, ICON_X};
use beui_core::image::Image;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;
use std::hash::{Hash, Hasher};
use std::time::Duration;

pub const TOAST_DURATION: Duration = Duration::from_secs(8);
const MARGIN: f32 = 16.0;
const WIDTH: f32 = 360.0;
const SPACING: f32 = 8.0;
const PADDING_HORIZONTAL: f32 = 12.0;
const PADDING_VERTICAL: f32 = 10.0;
const PICTURE_SIZE: f32 = 32.0;
const TEXT_SPACING: f32 = 2.0;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ToastAction {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub danger: bool,
    pub image: Option<Image>,
    pub actions: Vec<ToastAction>,
    pub activates: bool,
    pub sticky: bool,
}

impl Hash for Toast {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.title.hash(state);
        self.message.hash(state);
        self.danger.hash(state);
        self.image.as_ref().map(Image::id).hash(state);
        self.actions.hash(state);
        self.activates.hash(state);
        self.sticky.hash(state);
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
) -> NodeId {
    let toasts = create_memo(move || toasts.get());
    let open = create_memo(move || toasts.with(|toasts| !toasts.is_empty()));
    let placement = edge.map(|edge| match edge {
        Edge::Top => Placement::InsideTop,
        Edge::TopEnd => Placement::InsideTopEnd,
        Edge::Bottom => Placement::InsideBottom,
        Edge::BottomEnd => Placement::InsideBottomEnd,
    });
    view! {
        <Overlay
            anchor
            placement={placement}
            policy=LayerPolicy::Pass
            level=LayerLevel::Top
            traps_focus=false
            open={open}
        >
            <Frame padding=MARGIN width={WIDTH + 2.0 * MARGIN}>
                <List spacing=SPACING>
                    <ForEach keys={toasts}>|toast: Toast| {
                        let duration = match toast.sticky || toast.danger {
                            true => None,
                            false => duration.get_untracked(),
                        };
                        view! {
                            <ToastCard
                                toast
                                duration
                                on_dismiss={move |id: u64| on_dismiss.call(id)}
                                on_action={move |chosen: (u64, String)| on_action.call(chosen)}
                                on_activate={move |id: u64| on_activate.call(id)}
                            />
                        }
                    }</ForEach>
                </List>
            </Frame>
        </Overlay>
    }
}

#[component]
fn ToastCard(
    toast: Toast,
    duration: Option<Duration>,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
) -> NodeId {
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
    let expiry = create_timer(move || {
        on_dismiss.call(id);
        None
    });
    if let Some(duration) = duration {
        expiry.start(duration);
    }
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
            <Pressable
                @test_id={format!("toast.{id}.activate")}
                on_click={move || on_activate.call(id)}
            >
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
            padding={Sides::symmetric(PADDING_HORIZONTAL, PADDING_VERTICAL)}
        >
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                    <Frame @sizing=ItemSize::Percent(100.0)>{body}</Frame>
                    <IconButton
                        @test_id={format!("toast.{id}.dismiss")}
                        glyph=ICON_X
                        label="Dismiss"
                        size=IconButtonSize::Compact
                        press_focus=false
                        on_click={move || on_dismiss.call(id)}
                    />
                </List>
                <Show condition={acted}>
                    <ToastActions
                        id
                        actions={actions.clone()}
                        on_action={{
                            move |chosen: (u64, String)| on_action.call(chosen)
                        }}
                    />
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
fn ToastActions(id: u64, actions: Vec<ToastAction>, on_action: Callback<(u64, String)>) -> NodeId {
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
                        on_click={move || on_action.call((id, key.clone()))}
                    />
                }
            }</ForEach>
        </List>
    }
}
