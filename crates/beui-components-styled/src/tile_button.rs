use accesskit::{Node, Role, Toggled};
use beui_components_unstyled as unstyled;
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Align2;

use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::tokens::{Size, Stroke};
use crate::tooltip::Tooltip;

#[component]
pub fn TileButton(
    label: Prop<String>,
    #[prop(default = false)] selected: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] outlined: Prop<bool>,
    #[prop(default = false)] radio: bool,
    #[prop(default = String::new())] description: Prop<String>,
    #[prop(default = Size::Xl)] size: Size,
    on_click: ClickCallback,
    children: Child,
) -> NodeId {
    let selected = create_memo(move || selected.get());
    let accessibility = create_memo(move || {
        let mut node = Node::new(match radio {
            true => Role::RadioButton,
            false => Role::Button,
        });
        if radio {
            node.set_toggled(Toggled::from(selected.get()));
        }
        let description = description.get();
        if !description.is_empty() {
            node.set_description(description);
        }
        node
    });
    view! {
        <unstyled::Button
            label
            icon_only=true
            disabled
            accessibility={accessibility}
            on_click={on_click}
        >|handle: unstyled::ButtonHandle| {
            let keyboard = focus_ring(handle.focused);
            let fill = create_memo(move || match (selected.get(), handle.hovered.get()) {
                (true, _) => ColorRole::AccentSoft,
                (false, true) => ColorRole::Hover,
                (false, false) => ColorRole::None,
            });
            let border = create_memo(move || {
                if selected.get() || keyboard.get() {
                    Some(Border::new(ColorRole::Accent).width(Stroke::Focus))
                } else {
                    outlined.get().then_some(Border::DEFAULT)
                }
            });
            view! {
                <Tooltip label={handle.tooltip}>
                    <Frame
                        width=size
                        height=size
                        radius=Radius::Md
                        fill={fill}
                        border={border}
                        align={Align2::CENTER}
                    >
                        {children}
                    </Frame>
                </Tooltip>
            }
        }</unstyled::Button>
    }
}
