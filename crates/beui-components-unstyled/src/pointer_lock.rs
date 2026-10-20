use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::geometry::Vec2;
use beui_core::input::{CursorIcon, Key, KeyPress};
use beui_core::node::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct PointerLockHandle {
    pub locked: Binding<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[component]
pub fn PointerLock(
    locked: Binding<bool>,
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    on_motion: Callback<Vec2>,
    on_key: Callback<KeyPress, bool>,
    accessibility: Option<Prop<Node>>,
    #[prop(children)] content: Option<Render<PointerLockHandle>>,
) -> NodeId {
    let current = locked;
    let hovered = create_signal(false);
    let focused = create_signal(false);

    let attached = with_document(|document| document.watch_context());
    create_effect(move || {
        attached.get();
        publish(current.get());
    });
    on_cleanup(|| publish(false));

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    component_accessibility(accessibility);

    let change = move |next: bool| current.set(next);

    let engage = move || change(true);
    let release = move || change(false);
    let blurred = move |next: bool| {
        focused.set(next);
        if !next {
            release();
        }
    };
    let key = move |press: KeyPress| {
        if current.get_untracked() && press.key == Key::Escape {
            if press.pressed {
                release();
            }
            return true;
        }
        on_key.call(press)
    };

    let content_node = content.map(|build| {
        build.call(PointerLockHandle {
            locked: current,
            hovered: hovered.read_only(),
            focused: focused.read_only(),
        })
    });

    view! {
        <Interactive
            focusable=true
            on_focus_change={blurred}
            on_activate={engage}
            on_key={key}
            on_motion={move |motion: Vec2| on_motion.call(motion)}
            cursor={cursor}
            on_click={engage}
            on_hover_change={move |next: bool| hovered.set(next)}
            children={content_node}
        />
    }
}

fn publish(locked: bool) {
    try_with_document(|document| {
        if let Some(context) = document.context() {
            context.set_pointer_locked(locked);
        }
    });
}
