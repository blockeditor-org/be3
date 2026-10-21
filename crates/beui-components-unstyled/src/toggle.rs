use accesskit::{Node, Role, Toggled};
use beui_macros::{component, view};

use beui_core::input::CursorIcon;

use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleState {
    pub checked: bool,
    pub mixed: bool,
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub disabled: bool,
}

pub struct ToggleHandle {
    pub state: Memo<ToggleState>,
    pub label: Memo<String>,
    pub glyph: Memo<String>,
    pub tooltip: Memo<String>,
}

#[component]
pub fn Toggle(
    checked: Binding<bool>,
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
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let tooltip = action_tooltip(action.as_ref(), label.clone());
    let checked = match action.as_ref().and_then(Action::checked) {
        Some(pressed) => Binding::new(pressed, Callback::empty()),
        None => checked,
    };
    let disabled = action_disabled(action.as_ref(), disabled);
    let label = create_memo(move || label.get());
    let glyph = create_memo(move || glyph.get());
    let tooltip = create_memo(move || tooltip.get());
    let interaction = create_signal(InteractionState::default());
    let disabled = create_memo(move || disabled.get());
    let mixed = create_memo(move || mixed.get());
    let state = create_memo(move || {
        let interaction = interaction.get();
        ToggleState {
            checked: checked.get(),
            mixed: mixed.get(),
            hovered: interaction.hovered,
            active: interaction.active,
            focused: interaction.focused,
            disabled: disabled.get(),
        }
    });

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    component_accessibility(create_memo(move || {
        let mut node = accessibility.get();
        if node.label().is_none() && !label.get().is_empty() {
            node.set_label(label.get());
        }
        node.set_toggled(match mixed.get() {
            true => Toggled::Mixed,
            false => Toggled::from(checked.get()),
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
            state,
            label,
            glyph,
            tooltip,
        })
    });

    let toggle_checked = move || {
        if disabled.get() {
            return;
        }
        let next = mixed.get() || !checked.get();
        checked.set(next);
        if let Some(action) = &action {
            action.run();
        }
    };
    let key_toggle = toggle_checked.clone();
    let tab_stop = Prop::dynamic(move || tab_stop.get() && !disabled.get());
    let cursor = create_memo(move || match disabled.get() {
        true => CursorIcon::Default,
        false => CursorIcon::PointingHand,
    });

    set_component_state(checked);

    view! {
        <Interactive
            focusable=true
            tab_stop
            press_focus
            capture_presses
            on_activate={key_toggle}
            cursor
            on_click={toggle_checked}
            on_state_change={interaction}
            children={content_node}
        />
    }
}

pub fn toggle_checked(document: &Document, toggle: NodeId) -> Binding<bool> {
    *document.component_state::<Binding<bool>>(toggle)
}
