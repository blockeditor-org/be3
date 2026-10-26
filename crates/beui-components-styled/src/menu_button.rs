use crate::frame::{ColorRole, Frame, Radius};
use beui_core::icons::Glyph;
use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonVariant};
use crate::context_menu::menu_style;
use crate::focus_ring::FocusRing;
use crate::icon_button::{IconButtonFace, IconButtonSize};
use crate::text::{Icon, Text, TextSize};
use crate::theme::use_theme;
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuButtonHandle, MenuItem, Picture};
use beui_core::font::TextAlign;
use beui_core::icons::ICON_CHEVRON_DOWN;
use beui_core::image::Image;
use beui_core::input::ScrollGesture;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

const PICTURE_SIZE: f32 = TextSize::Base.icon_px();
const PICTURE_PADDING: f32 = 6.0;
const PICTURE_FOCUS_RING_OFFSET: f32 = 2.0;

#[component]
pub fn MenuButton(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] arrow: Prop<bool>,
    #[prop(default = false)] open: Binding<bool>,
    items: Children<MenuItem>,
    #[prop(default = Callback::default())] on_select: Callback<Vec<usize>>,
) -> NodeId {
    let trailing = create_memo(move || match arrow.get() {
        true => Some(ICON_CHEVRON_DOWN),
        false => None,
    });
    let icon_only = create_memo(move || icon_only.get());
    view! {
        <unstyled::MenuButton
            items
            open
            label
            glyph
            disabled
            menu={menu_style()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle { open, button } = handle;
                let quiet = create_memo(move || !icon_only.get() || open.get());
                view! {
                    <Tooltip label={button.label} disabled={quiet}>
                        <ButtonFace
                            handle={button}
                            variant
                            trailing_glyph={trailing}
                            icon_only={icon_only}
                        />
                    </Tooltip>
                }
            }}
            on_select={on_select}
        />
    }
}

#[component]
pub fn IconMenuButton(
    label: Prop<String>,
    glyph: Prop<Option<Glyph>>,
    #[prop(default = ButtonVariant::Ghost)] variant: ButtonVariant,
    #[prop(default = IconButtonSize::Regular)] size: IconButtonSize,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] open: Binding<bool>,
    items: Children<MenuItem>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    view! {
        <unstyled::MenuButton
            items
            open
            label
            glyph
            disabled
            menu={menu_style()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle { open, button } = handle;
                view! {
                    <Tooltip label={button.label} disabled={open}>
                        <IconButtonFace handle={button} variant size />
                    </Tooltip>
                }
            }}
            on_select={on_select}
        />
    }
}

#[component]
pub fn PictureMenuButton(
    label: Prop<String>,
    image: Prop<Option<Image>>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = true)] secondary_opens: Prop<bool>,
    items: Children<MenuItem>,
    on_click: ClickCallback,
    on_secondary_click: ClickCallback,
    on_middle_click: ClickCallback,
    on_scroll: Callback<ScrollGesture>,
    #[prop(default = false)] open: Binding<bool>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let image = create_memo(move || image.get());
    let pictured = create_memo(move || image.with(Option::is_some));
    let plain = create_memo(move || !pictured.get());
    let trigger = move |handle: MenuButtonHandle| {
        let MenuButtonHandle { open, button } = handle;
        let theme = use_theme();
        let unstyled::ButtonHandle {
            hovered,
            active,
            focused,
            disabled,
            glyph,
            label,
            ..
        } = button;
        let pressed = create_memo(move || active.get() || open.get());
        let fill = create_memo(move || {
            ButtonVariant::Ghost.fill(&theme, disabled.get(), hovered.get(), pressed.get())
        });
        let color = create_memo(move || ButtonVariant::Ghost.label(disabled.get()));
        view! {
            <Tooltip label disabled={open}>
                <FocusRing
                    focused
                    radius={Radius::custom(Radius::Md.px() + 2.0)}
                    offset=PICTURE_FOCUS_RING_OFFSET
                >
                    <Frame fill={ColorRole::custom(fill)} radius=Radius::Md padding=PICTURE_PADDING>
                        <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
                            <List spacing=0.0 align=Align::Center>
                                <Show condition={pictured}>
                                    <SizedPicture image={image} size=PICTURE_SIZE />
                                </Show>
                                <Show condition={plain}>
                                    <Text color={color} align=TextAlign::Center>
                                        <Icon glyph={glyph} />
                                    </Text>
                                </Show>
                            </List>
                        </Frame>
                    </Frame>
                </FocusRing>
            </Tooltip>
        }
    };
    view! {
        <unstyled::MenuButton
            items
            open
            label
            glyph
            disabled
            click_opens
            secondary_opens
            menu={menu_style()}
            trigger
            on_click={on_click}
            on_secondary_click={on_secondary_click}
            on_middle_click={on_middle_click}
            on_scroll={on_scroll}
            on_select={on_select}
        />
    }
}

#[component]
fn SizedPicture(image: Memo<Option<Image>>, size: f32) -> NodeId {
    view! {
        <Frame width=size height=size>
            <Picture image />
        </Frame>
    }
}
