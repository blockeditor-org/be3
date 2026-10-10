use beui_macros::{component, view};

use crate::button::{Button, ButtonVariant};
use crate::icon_button::{IconButton, IconButtonSize};
use crate::text::Icon;
use crate::theme::{BORDER_WIDTH, FONT_BODY, FONT_SMALL, RADIUS, use_theme};
use beui_components_unstyled::{Picture, Pressable};
use beui_core::base::Justify;
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::icons::{ICON_CLOSE, ICON_ERROR, ICON_INFO};
use beui_core::image::Image;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, List, Prop, Show, Text, create_memo,
    create_timer,
};
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
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
) -> NodeId {
    let listed = toasts.clone();
    let open = create_memo(move || !listed.get().is_empty());
    view! {
        <Overlay
            anchor
            placement=Placement::InsideBottomEnd
            mode=OverlayMode::Floating
            traps_focus=false
            open={open}
        >
            <Frame
                padding_horizontal=MARGIN
                padding_vertical=MARGIN
                width=Some(WIDTH + 2.0 * MARGIN)
            >
                <List spacing=SPACING>
                    <ForEach keys={toasts}>
                        {move |toast: Toast| {
                            let on_dismiss = on_dismiss.clone();
                            let on_action = on_action.clone();
                            let on_activate = on_activate.clone();
                            let duration = match toast.sticky {
                                true => None,
                                false => duration.peek(),
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
                        }}
                    </ForEach>
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
    let theme = use_theme();
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
    let pictured = image.is_some();
    let titled = !title.is_empty();
    let said = !message.is_empty();
    let acted = !actions.is_empty();
    let (message_size, message_color) = match titled {
        true => (FONT_SMALL, theme.text_muted.clone()),
        false => (FONT_BODY, theme.text.clone()),
    };
    let title_color = theme.text.clone();
    let content = view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
            <Show condition={pictured}>
                <ToastPicture image={image.clone()} />
            </Show>
            <Show condition={!pictured}>
                <Icon glyph={glyph.to_owned()} color={tint.clone()} />
            </Show>
            <List @sizing=ItemSize::Percent(100.0) spacing=TEXT_SPACING>
                <Show condition={titled}>
                    <Text
                        string={title.clone()}
                        font_size=FONT_BODY
                        color={title_color.clone()}
                        bold=true
                        wrap=true
                    />
                </Show>
                <Show condition={said}>
                    <Text
                        string={message.clone()}
                        font_size={message_size}
                        color={message_color.clone()}
                        wrap=true
                    />
                </Show>
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
            color={theme.surface_raised.clone()}
            outline={theme.border.clone()}
            outline_width=BORDER_WIDTH
            outline_visible=true
            radius=RADIUS
            padding_horizontal=PADDING_HORIZONTAL
            padding_vertical=PADDING_VERTICAL
        >
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Start spacing=SPACING>
                    <Frame @sizing=ItemSize::Percent(100.0)>{body}</Frame>
                    <IconButton
                        @test_id={format!("toast.{id}.dismiss")}
                        glyph={ICON_CLOSE.to_owned()}
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
                            let acting = on_action.clone();
                            move |chosen: (u64, String)| acting.call(chosen)
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
            <ForEach keys={actions}>
                {move |action: ToastAction| {
                    let on_action = on_action.clone();
                    let key = action.key.clone();
                    view! {
                        <Button
                            @test_id={format!("toast.{id}.action.{}", action.key)}
                            label={action.label}
                            variant=ButtonVariant::Secondary
                            on_click={move || on_action.call((id, key.clone()))}
                        />
                    }
                }}
            </ForEach>
        </List>
    }
}
