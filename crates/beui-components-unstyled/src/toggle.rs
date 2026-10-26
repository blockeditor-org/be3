use accesskit::{Node, Role, Toggled};
use beui_view::types::icons::Glyph;
use beui_macros::{component, view};

use beui_view::types::CursorIcon;

use beui_view::reactive::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct ToggleHandle {
    pub checked: Binding<bool>,
    pub mixed: Memo<bool>,
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
    pub label: Memo<String>,
    pub glyph: Memo<Option<Glyph>>,
    pub icon_only: Memo<bool>,
    pub tooltip: Memo<String>,
}

#[component]
pub fn Toggle(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ToggleApi>,
    checked: Binding<bool>,
    #[prop(default = false)] mixed: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = Role::CheckBox)] role: Role,
    action: Option<Action>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] icon_only: Prop<bool>,
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
    let icon_only = create_memo(move || icon_only.get());
    let tooltip = create_memo(move || match icon_only.get() {
        true => tooltip.get(),
        false => String::new(),
    });
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
            checked,
            mixed,
            hovered: hovered.read_only(),
            active: active.read_only(),
            focused: focused.read_only(),
            disabled,
            label,
            glyph,
            icon_only,
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

    api.fill(ToggleApi(checked));

    view! {
        <Interactive
            focusable=true
            tab_stop
            press_focus
            capture_presses
            on_focus_change={focused}
            on_activate_change={key_active}
            on_activate={key_toggle}
            cursor
            key_active
            on_click={toggle_checked}
            on_hover_change={hovered}
            on_active_change={active}
            children={content_node}
        />
    }
}

#[derive(Clone, Copy)]
pub struct ToggleApi(Binding<bool>);

impl ToggleApi {
    pub fn checked(&self) -> Binding<bool> {
        self.0
    }
}
