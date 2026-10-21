use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::input::{CursorIcon, KeyPress, PointerPress};

use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
    pub disabled: bool,
}

#[derive(Clone, Copy)]
pub struct ButtonHandle {
    pub state: Memo<ButtonState>,
    pub label: Memo<String>,
    pub glyph: Memo<String>,
    pub tooltip: Memo<String>,
}

#[component]
pub fn Button(
    #[prop(children)] content: Option<Render<ButtonHandle>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = String::new())] glyph: Prop<String>,
    #[prop(default = Role::Button)] role: Role,
    action: Option<Action>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    on_click: ClickCallback,
    on_click_at: Callback<PointerPress>,
    on_press: Callback<PointerPress>,
    on_key: Callback<KeyPress, bool>,
    on_text: Callback<String>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let focus_request = focused;
    let takes_text = !on_text.is_empty();
    let interaction = create_signal(InteractionState::default());
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let tooltip = action_tooltip(action.as_ref(), label.clone());
    let disabled = action_disabled(action.as_ref(), disabled);
    let disabled = create_memo(move || disabled.get());
    let label = create_memo(move || label.get());
    let glyph = create_memo(move || glyph.get());
    let tooltip = create_memo(move || tooltip.get());
    let state = create_memo(move || {
        let interaction = interaction.get();
        ButtonState {
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
        if disabled.get() {
            node.set_disabled();
        } else {
            node.clear_disabled();
        }
        node
    }));

    let content_node = content.map(|build| {
        build.call(ButtonHandle {
            state,
            label,
            glyph,
            tooltip,
        })
    });

    let click = move || {
        if disabled.get_untracked() {
            return;
        }
        if let Some(action) = &action {
            action.run();
        }
        on_click.call();
    };
    let key_click = click.clone();
    let click_at = move |press: PointerPress| {
        if disabled.get_untracked() {
            return;
        }
        on_click_at.call(press);
    };
    let tab_stop = Prop::dynamic(move || tab_stop.get() && !disabled.get());

    set_component_state(state);
    let on_state_change = match on_focus_change.is_empty() {
        true => Callback::from(interaction),
        false => {
            let focus = InteractionState::watch(|state| state.focused, on_focus_change);
            Callback::new(move |next: InteractionState| {
                interaction.set(next);
                focus.call(next);
            })
        }
    };

    view! {
        <Interactive
            focusable=true
            tab_stop
            press_focus
            focused={focus_request}
            on_key={on_key}
            on_text={on_text}
            takes_text
            on_activate={key_click}
            cursor=CursorIcon::PointingHand
            capture_presses
            on_click={click}
            on_click_at={click_at}
            on_press={on_press}
            on_state_change
            children={content_node}
        />
    }
}

pub fn button_state(document: &Document, button: NodeId) -> Memo<ButtonState> {
    *document.component_state::<Memo<ButtonState>>(button)
}
