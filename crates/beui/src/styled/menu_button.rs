use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::icons::ICON_ARROW_DROP_DOWN;
use crate::node::NodeId;
use crate::reactive::{Callback, Children, Prop, clone, create_memo};
use crate::styled::button::{ButtonFace, ButtonVariant};
use crate::styled::context_menu::{menu_panel, menu_row};
use crate::styled::tooltip::Tooltip;
use crate::unstyled;
use crate::unstyled::{MenuButtonHandle, MenuItem};

#[component]
pub fn MenuButton(
    label: Prop<String>,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] arrow: bool,
    items: Children<MenuItem>,
    on_select: Callback<Vec<usize>>,
) -> NodeId {
    let disabled = create_memo(move || disabled.get());
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
                let trailing = match arrow {
                    true => ICON_ARROW_DROP_DOWN.to_owned(),
                    false => String::new(),
                };
                view! {
                    <Tooltip label={label_text.clone()} disabled={quiet}>
                        <ButtonFace
                            handle={button}
                            variant
                            label={face_label.clone()}
                            glyph
                            trailing_glyph={trailing}
                            disabled={face.clone()}
                        />
                    </Tooltip>
                }
            }}
            on_select={move |path: Vec<usize>| on_select.call(path)}
        />
    }
}
