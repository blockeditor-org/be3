use accesskit::{Node, Role};
use beui_macros::{component, view};
use beui_view::types::icons::Glyph;

use beui_view::types::{CursorIcon, KeyPress, PointerPress};

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::{CORE, Core};

#[derive(Clone, Copy)]
pub struct ButtonHandle {
    pub hovered: ReadSignal<bool>,
    pub active: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
    pub label: Memo<String>,
    pub glyph: Memo<Option<Glyph>>,
    pub icon_only: Memo<bool>,
    pub tooltip: Memo<String>,
}

#[derive(Clone, Copy)]
struct State {
    active: ReadSignal<bool>,
    focused: ReadSignal<bool>,
}

#[component]
pub fn Button(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ButtonApi>,
    #[prop(children)] content: Option<Render<ButtonHandle>>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = None)] glyph: Prop<Option<Glyph>>,
    #[prop(default = Role::Button)] role: Role,
    action: Option<Action>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] capture_presses: Prop<bool>,
    #[prop(default = true)] tab_stop: Prop<bool>,
    #[prop(default = true)] press_focus: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] icon_only: Prop<bool>,
    #[prop(default = None)] href: Prop<Option<String>>,
    #[prop(default = false)] submit: bool,
    expanded: Option<Prop<bool>>,
    on_click: ClickCallback,
    on_click_at: Callback<PointerPress>,
    on_press: Callback<PointerPress>,
    on_key: Callback<KeyPress, bool>,
    on_text: Callback<String>,
    on_focus_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let focus_request = focused;
    let hovered = create_signal(false);
    let active = create_signal(false);
    let focused = create_signal(false);
    let key_active = create_signal(false);
    let label = action_label(action.as_ref(), label);
    let glyph = action_glyph(action.as_ref(), glyph);
    let tooltip = action_tooltip(action.as_ref(), label.clone());
    let disabled = action_disabled(action.as_ref(), disabled);
    let disabled = create_memo(move || disabled.get());
    let label = create_memo(move || label.get());
    let glyph = create_memo(move || glyph.get());
    let icon_only = create_memo(move || icon_only.get());
    let expanded = expanded.map(|expanded| create_memo(move || expanded.get()));
    let tooltip = create_memo(move || {
        let quiet = !icon_only.get() || expanded.is_some_and(|expanded| expanded.get());
        match quiet {
            true => String::new(),
            false => tooltip.get(),
        }
    });
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    component_accessibility(create_memo(move || {
        let mut node = accessibility.get();
        if node.label().is_none() && !label.get().is_empty() {
            node.set_label(label.get());
        }
        if let Some(expanded) = expanded {
            node.set_expanded(expanded.get());
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

    let form = crate::form::form_submit().filter(|_| submit && CORE != Core::Html);
    let click = move || {
        if disabled.get_untracked() {
            return;
        }
        if let Some(action) = &action {
            action.run();
        }
        on_click.call();
        if let Some(form) = form {
            form.call();
        }
    };
    let key_click = click.clone();
    let click_at = move |press: PointerPress| {
        if disabled.get_untracked() {
            return;
        }
        on_click_at.call(press);
    };
    let tab_stop = Prop::dynamic(move || tab_stop.get() && !disabled.get());

    api.fill(ButtonApi(State {
        active: active.read_only(),
        focused: focused.read_only(),
    }));

    let linked = untrack(|| href.get().is_some());
    let element = match (CORE, linked, submit) {
        (Core::Html, true, _) => Some((
            "a",
            vec![(
                "href",
                Prop::dynamic(move || href.get().filter(|_| !disabled.get())),
            )],
        )),
        (Core::Html, false, true) => Some((
            "button",
            vec![
                ("type", Prop::Static(Some("submit".to_owned()))),
                (
                    "disabled",
                    Prop::dynamic(move || disabled.get().then(String::new)),
                ),
            ],
        )),
        _ => None,
    };
    let browser_activates = element.is_some();
    let content_node = match element {
        Some((tag, attributes)) => Some(view! {
            <Element tag attributes styles={crate::form::cell_styles()} children={content_node} />
        }),
        None => content_node,
    };
    let (key_active_change, key_click): (Callback<bool>, ClickCallback) = match browser_activates {
        true => (Callback::empty(), ClickCallback::empty()),
        false => (key_active.into(), ClickCallback::new(key_click)),
    };

    view! {
        <Interactive
            focusable=true
            tab_stop
            press_focus
            focused={focus_request}
            on_key={on_key}
            on_text={on_text}
            on_focus_change={move |has_focus: bool| {
                focused.set(has_focus);
                on_focus_change.call(has_focus);
            }}
            on_activate_change={key_active_change}
            on_activate={key_click}
            cursor=CursorIcon::PointingHand
            capture_presses
            key_active
            on_click={click}
            on_click_at={click_at}
            on_press={on_press}
            on_hover_change={hovered}
            on_active_change={active}
            children={content_node}
        />
    }
}

#[derive(Clone, Copy)]
pub struct ButtonApi(State);

impl ButtonApi {
    pub fn active(&self) -> ReadSignal<bool> {
        self.0.active
    }

    pub fn focused(&self) -> ReadSignal<bool> {
        self.0.focused
    }
}
