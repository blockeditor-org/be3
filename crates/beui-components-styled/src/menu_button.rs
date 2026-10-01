use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::action_row::ActionRowFace;
use crate::button::{ButtonFace, ButtonVariant};
use crate::context_menu::{menu_panel, menu_row};
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
    let disabled = create_memo(move || disabled.get());
    let trailing = create_memo(move || match arrow.get() {
        true => ICON_ARROW_DROP_DOWN.to_owned(),
        false => String::new(),
    });
    let face = disabled.clone();
    let label_text = create_memo(clone!(label -> move || label.get()));
    let named = create_memo(move || !icon_only.get());
    let face_label = create_memo(clone!(label_text named -> move || match named.get() {
        true => label_text.get(),
        false => String::new(),
    }));
    let accessibility = create_memo(clone!(label_text -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(label_text.get());
        node
    }));
    view! {
        <unstyled::MenuButton
            items
            disabled
            accessibility
            row={menu_row()}
            panel={menu_panel()}
            sheet={menu_sheet()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle {
                    open,
                    hovered,
                    active,
                    focused,
                } = handle;
                let quiet = create_memo(clone!(named -> move || named.get() || open.get()));
                let button = unstyled::ButtonHandle {
                    hovered,
                    active,
                    focused,
                };
                view! {
                    <Tooltip label={label_text.clone()} disabled={quiet}>
                        <ButtonFace
                            handle={button}
                            variant
                            label={face_label.clone()}
                            glyph
                            trailing_glyph={trailing.clone()}
                            disabled={face.clone()}
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
    let disabled = create_memo(move || disabled.get());
    let face = disabled.clone();
    let label_text = create_memo(clone!(label -> move || label.get()));
    let accessibility = create_memo(clone!(label_text -> move || {
        let mut node = Node::new(Role::Button);
        node.set_label(label_text.get());
        node
    }));
    let glyph = create_memo(move || glyph.get());
    view! {
        <unstyled::MenuButton
            items
            disabled
            accessibility
            row={menu_row()}
            panel={menu_panel()}
            sheet={menu_sheet()}
            trigger={move |handle: MenuButtonHandle| {
                let MenuButtonHandle {
                    open,
                    hovered,
                    active,
                    focused,
                } = handle;
                let button = unstyled::ButtonHandle {
                    hovered,
                    active,
                    focused,
                };
                view! {
                    <Tooltip label={label_text.clone()} disabled={open}>
                        <IconButtonFace
                            handle={button}
                            variant
                            size
                            glyph={glyph.clone()}
                            disabled={face.clone()}
                        />
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
    let button = unstyled::ButtonHandle {
        hovered,
        active,
        focused,
    };
    view! {
        <ActionRowFace handle={button} label glyph detail=String::new() disabled danger=false />
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
