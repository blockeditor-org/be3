use accesskit::{Node, Role, Toggled};
use beui_macros::{component, view};

use beui_core::input::CursorIcon;

use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Action, Callback, Interactive, IntoProp, Memo, Prop, ReadSignal, Render, action_disabled,
    action_glyph, action_label, action_pressed, action_tooltip, clone, component_accessibility,
    create_effect, create_memo, create_signal, set_component_state, untrack,
};

pub struct ToggleHandle {
    pub checked: ReadSignal<bool>,
    pub mixed: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
    pub label: Memo<String>,
    pub glyph: Memo<String>,
    pub tooltip: Memo<String>,
}

#[component]
pub fn Toggle(
    checked: Prop<bool>,
    #[prop(default = false)] mixed: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = Role::CheckBox)] role: Role,
    action: Option<Action>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(children)] content: Option<Render<ToggleHandle>>,
    on_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let tooltip = action_tooltip(action.as_ref(), label.clone());
    let checked = action_pressed(action.as_ref(), checked);
    let disabled = action_disabled(action.as_ref(), disabled);
    let label = create_memo(move || label.get());
    let glyph = create_memo(move || glyph.get());
    let tooltip = create_memo(move || tooltip.get());
    let (checked_read, set_checked) = create_signal(checked.peek());
    create_effect(clone!(set_checked -> move || set_checked.set(checked.get())));
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let mixed = create_memo(move || mixed.get());

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    component_accessibility(create_memo(
        clone!(checked_read mixed disabled label -> move || {
            let mut node = accessibility.get();
            if node.label().is_none() && !label.get().is_empty() {
                node.set_label(label.get());
            }
            node.set_toggled(match mixed.get() {
                true => Toggled::Mixed,
                false => Toggled::from(checked_read.get()),
            });
            if disabled.get() {
                node.set_disabled();
            } else {
                node.clear_disabled();
            }
            node
        }),
    ));

    let content_node = content.map(|build| {
        build.call(ToggleHandle {
            checked: checked_read.clone(),
            mixed: mixed.clone(),
            hovered: hovered.clone(),
            active: active.clone(),
            focused: focused.clone(),
            disabled: disabled.clone(),
            label,
            glyph,
            tooltip,
        })
    });

    let toggle_checked = {
        let checked = checked_read.clone();
        let disabled = disabled.clone();
        move || {
            if untrack(|| disabled.get()) {
                return;
            }
            let next = untrack(|| mixed.get() || !checked.get());
            set_checked.set(next);
            if let Some(action) = &action {
                action.run();
            }
            on_change.call(next);
        }
    };
    let key_toggle = toggle_checked.clone();
    let tab_stop = disabled.clone().into_prop().map(|disabled: bool| !disabled);
    let cursor = create_memo(clone!(disabled -> move || match disabled.get() {
        true => CursorIcon::Default,
        false => CursorIcon::PointingHand,
    }));

    set_component_state(checked_read.clone());

    view! {
        <Interactive
            focusable=true
            tab_stop
            on_focus_change={move |focused: bool| set_focused.set(focused)}
            on_activate_change={move |pressed: bool| set_key_active.set(pressed)}
            on_activate={key_toggle}
            cursor
            key_active
            on_click={toggle_checked}
            on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
            on_active_change={move |active: bool| set_active.set(active)}
            children={content_node}
        />
    }
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    document.component_state::<ReadSignal<bool>>(toggle).clone()
}
