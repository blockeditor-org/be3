use beui_macros::{component, view};

use crate::button::{ButtonFace, ButtonVariant};
use crate::context_menu::menu_style;
use crate::focus_ring::FocusRing;
use crate::icon_button::{IconButtonFace, IconButtonSize};
use crate::text::IconSized;
use crate::theme::{ICON_SIZE, RADIUS, use_theme};
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{MenuButtonHandle, MenuItem, Picture};
use beui_core::icons::ICON_ARROW_DROP_DOWN;
use beui_core::image::Image;
use beui_core::input::ScrollGesture;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Align, Callback, Children, ClickCallback, Frame, List, Memo, Prop, Show, clone, create_memo,
};

const PICTURE_PADDING: f32 = 6.0;
const PICTURE_FOCUS_RING_OFFSET: f32 = 2.0;

#[component]
pub fn MenuButton(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] arrow: Prop<bool>,
    items: Children<MenuItem>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let trailing = create_memo(move || match arrow.get() {
        true => ICON_ARROW_DROP_DOWN.to_owned(),
        false => String::new(),
    });
    let icon_only = create_memo(move || icon_only.get());
    view! {
        <unstyled::MenuButton
            items
            label
            glyph
            disabled
            menu={menu_style()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle { open, button } = handle;
                let quiet = create_memo(clone!(icon_only -> move || !icon_only.get() || open.get()));
                view! {
                    <Tooltip label={button.label.clone()} disabled={quiet}>
                        <ButtonFace
                            handle={button}
                            variant
                            trailing_glyph={trailing.clone()}
                            icon_only={icon_only.clone()}
                        />
                    </Tooltip>
                }
            }}
            on_select={move |path: Vec<usize>| on_select.call(path)}
        />
    }
}

#[component]
pub fn IconMenuButton(
    label: Prop<String>,
    glyph: Prop<String>,
    #[prop(default = ButtonVariant::Ghost)] variant: ButtonVariant,
    #[prop(default = IconButtonSize::Regular)] size: IconButtonSize,
    #[prop(default = false)] disabled: Prop<bool>,
    items: Children<MenuItem>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    view! {
        <unstyled::MenuButton
            items
            label
            glyph
            disabled
            menu={menu_style()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle { open, button } = handle;
                view! {
                    <Tooltip label={button.label.clone()} disabled={open}>
                        <IconButtonFace handle={button} variant size />
                    </Tooltip>
                }
            }}
            on_select={move |path: Vec<usize>| on_select.call(path)}
        />
    }
}

#[component]
pub fn PictureMenuButton(
    label: Prop<String>,
    image: Prop<Option<Image>>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = ICON_SIZE)] size: f32,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = true)] secondary_opens: Prop<bool>,
    items: Children<MenuItem>,
    on_click: ClickCallback,
    on_secondary_click: ClickCallback,
    on_middle_click: ClickCallback,
    on_scroll: Callback<ScrollGesture>,
    on_open_change: Callback<bool>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let image = create_memo(move || image.get());
    let pictured = create_memo(clone!(image -> move || image.with(Option::is_some)));
    let plain = create_memo(clone!(pictured -> move || !pictured.get()));
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
        let pressed = create_memo(clone!(open -> move || active.get() || open.get()));
        let fill = create_memo(clone!(theme disabled -> move || {
            ButtonVariant::Ghost.fill(&theme, disabled.get(), hovered.get(), pressed.get())
        }));
        let color =
            create_memo(clone!(theme -> move || ButtonVariant::Ghost.label(&theme, disabled.get())));
        let (image, pictured, plain) = (image.clone(), pictured.clone(), plain.clone());
        view! {
            <Tooltip label disabled={open}>
                <FocusRing focused radius={RADIUS + 2} offset=PICTURE_FOCUS_RING_OFFSET>
                    <Frame
                        color={fill}
                        radius=RADIUS
                        padding_horizontal=PICTURE_PADDING
                        padding_vertical=PICTURE_PADDING
                    >
                        <Frame width=size height=size>
                            <List spacing=0.0 align=Align::Center>
                                <Show condition={pictured}>
                                    <SizedPicture image={image.clone()} size />
                                </Show>
                                <Show condition={plain}>
                                    <IconSized glyph={glyph.clone()} font_size=size color={color.clone()} />
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
            label
            glyph
            disabled
            click_opens
            secondary_opens
            menu={menu_style()}
            trigger
            on_click={move || on_click.call()}
            on_secondary_click={move || on_secondary_click.call()}
            on_middle_click={move || on_middle_click.call()}
            on_scroll={move |gesture: ScrollGesture| on_scroll.call(gesture)}
            on_open_change={move |open: bool| on_open_change.call(open)}
            on_select={move |path: Vec<usize>| on_select.call(path)}
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
