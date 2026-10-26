use accesskit::{HasPopup, Node, Role};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use crate as unstyled;
use crate::ButtonHandle;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;

#[derive(Clone, Copy)]
pub struct PopupTriggerHandle {
    pub open: Binding<bool>,
    pub button: ButtonHandle,
}

#[component]
pub fn PopupTrigger(
    open: Binding<bool>,
    popup: HasPopup,
    content: Render<PopupTriggerHandle>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = true)] click_opens: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = Node::new(Role::Button))] accessibility: Prop<Node>,
    on_click: ClickCallback,
    on_focus_change: Callback<bool>,
) -> NodeId {
    let accessibility = create_memo(move || {
        let mut node = accessibility.get();
        node.set_has_popup(popup);
        node
    });
    view! {
        <unstyled::Button
            label
            glyph
            icon_only
            disabled
            focused
            tab_stop
            capture_presses
            expanded={open}
            accessibility={accessibility}
            on_focus_change={on_focus_change}
            on_click={move || match click_opens.get_untracked() {
                true => open.update(|open| *open = !*open),
                false => on_click.call(),
            }}
            content={Render::new(move |button: ButtonHandle| {
                content.call(PopupTriggerHandle { open, button })
            })}
        />
    }
}
