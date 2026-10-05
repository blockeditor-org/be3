use beui_macros::{component, view};

use crate::action_row::ActionRowFace;
use crate::button::{ButtonFace, ButtonVariant};
use crate::context_menu::menu_style;
use crate::icon_button::{IconButtonFace, IconButtonSize};
use crate::sheet::ModalSheet;
use crate::tooltip::Tooltip;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    MenuButtonHandle, MenuItem, MenuRowHandle, MenuSheet, MenuSheetHandle,
};
use beui_core::icons::ICON_ARROW_DROP_DOWN;
use beui_core::node::NodeId;
use beui_view::reactive::{Callback, Children, Frame, Prop, RenderFn, clone, create_memo};

const SHEET_PADDING: f32 = 8.0;
const SHEET_STOPS: [f32; 2] = [0.5, 0.9];

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
            sheet={menu_sheet()}
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
            sheet={menu_sheet()}
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

pub fn menu_sheet() -> MenuSheet {
    MenuSheet {
        row: RenderFn::new(|handle| {
            view! {
                <MenuSheetRow handle />
            }
        }),
        sheet: RenderFn::new(|handle| {
            view! {
                <MenuSheetPanel handle />
            }
        }),
    }
}

#[component]
fn MenuSheetRow(handle: MenuRowHandle) -> NodeId {
    let MenuRowHandle {
        label,
        glyph,
        disabled,
        hovered,
        active,
        focused,
        ..
    } = handle;
    view! {
        <ActionRowFace
            hovered
            active
            focused
            label
            glyph
            detail=String::new()
            disabled
            danger=false
        />
    }
}

#[component]
fn MenuSheetPanel(handle: MenuSheetHandle) -> NodeId {
    let MenuSheetHandle {
        open,
        on_close,
        menu,
    } = handle;
    view! {
        <ModalSheet
            open={open}
            rest={SHEET_STOPS[0]}
            stops={SHEET_STOPS.to_vec()}
            on_close={move || on_close.call()}
        >
            <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>{menu}</Frame>
        </ModalSheet>
    }
}
