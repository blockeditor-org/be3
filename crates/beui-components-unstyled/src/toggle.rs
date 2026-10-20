use accesskit::{Node, Role, Toggled};
use beui_macros::{component, view};

use beui_core::input::CursorIcon;

use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
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
    let current = create_signal(checked.get_untracked());
    create_effect(move || current.set(checked.get()));
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let key_active = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let mixed = create_memo(move || mixed.get());

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    component_accessibility(create_memo(move || {
        let mut node = accessibility.get();
        if node.label().is_none() && !label.get().is_empty() {
            node.set_label(label.get());
        }
        node.set_toggled(match mixed.get() {
            true => Toggled::Mixed,
            false => Toggled::from(current.get()),
        });
        if disabled.get() {
            node.set_disabled();
        } else {
            node.clear_disabled();
        }
        node
    }));

    let content_node = content.map(|build| {
        build.call(ToggleHandle {
            checked: current.read_only(),
            mixed,
            hovered: hovered.read_only(),
            active: active.read_only(),
            focused: focused.read_only(),
            disabled,
            label,
            glyph,
            tooltip,
        })
    });

    let toggle_checked = move || {
        if disabled.get() {
            return;
        }
        let next = mixed.get() || !current.get();
        current.set(next);
        if let Some(action) = &action {
            action.run();
        }
        on_change.call(next);
    };
    let key_toggle = toggle_checked.clone();
    let tab_stop = Prop::dynamic(move || tab_stop.get() && !disabled.get());
    let cursor = create_memo(move || match disabled.get() {
        true => CursorIcon::Default,
        false => CursorIcon::PointingHand,
    });

    set_component_state(current.read_only());

    view! {
        <Interactive
            focusable=true
            tab_stop
            press_focus
            capture_presses
            on_focus_change={move |next: bool| focused.set(next)}
            on_activate_change={move |pressed: bool| key_active.set(pressed)}
            on_activate={key_toggle}
            cursor
            key_active
            on_click={toggle_checked}
            on_hover_change={move |next: bool| hovered.set(next)}
            on_active_change={move |next: bool| active.set(next)}
            children={content_node}
        />
    }
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> ReadSignal<bool> {
    *document.component_state::<ReadSignal<bool>>(toggle)
}
