use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::input::{CursorIcon, KeyPress, PointerPress};

use beui_core::document::Document;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Action, Callback, Child, ClickCallback, Interactive, Memo, Prop, ReadSignal, Render,
    action_disabled, action_glyph, action_label, action_tooltip, clone, component_accessibility,
    create_memo, create_signal, set_component_state, untrack,
};

#[derive(Clone)]
pub struct ButtonHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
    pub label: Memo<String>,
    pub glyph: Memo<String>,
    pub tooltip: Memo<String>,
}

struct State {
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
}

#[component]
pub fn Button(
    children: Option<Child>,
    content: Option<Render<ButtonHandle>>,
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
    let (hovered, set_hovered) = create_signal(false);
    let (active, set_active) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let (key_active, set_key_active) = create_signal(false);
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let tooltip = action_tooltip(action.as_ref(), label.clone());
    let disabled = action_disabled(action.as_ref(), disabled);
    let disabled = create_memo(move || disabled.get());
    let label = create_memo(move || label.get());
    let glyph = create_memo(move || glyph.get());
    let tooltip = create_memo(move || tooltip.get());
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    component_accessibility(create_memo(clone!(disabled label -> move || {
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
    })));

    let content_node = match content {
        Some(build) => Some(build.call(ButtonHandle {
            hovered,
            active: active.clone(),
            focused: focused.clone(),
            disabled: disabled.clone(),
            label,
            glyph,
            tooltip,
        })),
        None => children,
    };

    let click = {
        let disabled = disabled.clone();
        move || {
            if untrack(|| disabled.get()) {
                return;
            }
            if let Some(action) = &action {
                action.run();
            }
            on_click.call();
        }
    };
    let key_click = click.clone();
    let click_at = {
        let disabled = disabled.clone();
        move |press: PointerPress| {
            if untrack(|| disabled.get()) {
                return;
            }
            on_click_at.call(press);
        }
    };
    let tab_stop = tab_stop.map(move |tab_stop| tab_stop && !disabled.get());

    set_component_state(State {
        active: active.clone(),
        focused: focused.clone(),
    });

    view! {
        <Interactive
            focusable=true
            tab_stop
            press_focus
            focused={focus_request}
            on_key={move |press| on_key.call(press)}
            on_text={move |text| on_text.call(text)}
            takes_text
            on_focus_change={move |has_focus: bool| {
                set_focused.set(has_focus);
                on_focus_change.call(has_focus);
            }}
            on_activate_change={move |pressed: bool| set_key_active.set(pressed)}
            on_activate={key_click}
            cursor=CursorIcon::PointingHand
            capture_presses
            key_active
            on_click={click}
            on_click_at={click_at}
            on_press={move |press| on_press.call(press)}
            on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
            on_active_change={move |active: bool| set_active.set(active)}
            children={content_node}
        />
    }
}

pub fn button_active(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).active.clone()
}

pub fn button_focused(document: &Document, button: NodeId) -> ReadSignal<bool> {
    document.component_state::<State>(button).focused.clone()
}
