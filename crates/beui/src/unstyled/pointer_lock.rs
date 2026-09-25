use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::geometry::Vec2;
use crate::input::{CursorIcon, Key, KeyPress};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, Focusable, Prop, ReadSignal, Render, clone, component_accessibility,
    create_effect, create_signal, on_cleanup, try_with_document, untrack, with_document,
};

pub struct PointerLockHandle {
    pub locked: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[component]
pub fn PointerLock(
    locked: Prop<bool>,
    #[prop(default = CursorIcon::Default)] cursor: Prop<CursorIcon>,
    on_change: Callback<bool>,
    on_motion: Callback<Vec2>,
    on_key: Callback<KeyPress, bool>,
    accessibility: Option<Prop<Node>>,
    #[prop(children)] content: Option<Render<PointerLockHandle>>,
) -> NodeId {
    let (locked_read, set_locked) = create_signal(locked.peek());
    create_effect(clone!(set_locked -> move || set_locked.set(locked.get())));
    let (hovered, set_hovered) = create_signal(false);
    let (focused, set_focused) = create_signal(false);

    let attached = with_document(|document| document.watch_context());
    create_effect(clone!(locked_read -> move || {
        attached.get();
        publish(locked_read.get());
    }));
    on_cleanup(|| publish(false));

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::Button)));
    component_accessibility(accessibility);

    let change = clone!(locked_read set_locked -> move |next: bool| {
        if untrack(|| locked_read.get()) == next {
            return;
        }
        set_locked.set(next);
        on_change.call(next);
    });

    let engage = clone!(change -> move || change(true));
    let release = clone!(change -> move || change(false));
    let blurred = clone!(release -> move |focused: bool| {
        set_focused.set(focused);
        if !focused {
            release();
        }
    });
    let key = clone!(locked_read release -> move |press: KeyPress| {
        if untrack(|| locked_read.get()) && press.key == Key::Escape {
            if press.pressed {
                release();
            }
            return true;
        }
        on_key.call(press)
    });

    let content_node = content.map(|build| {
        build.call(PointerLockHandle {
            locked: locked_read.clone(),
            hovered: hovered.clone(),
            focused: focused.clone(),
        })
    });

    view! {
        <Focusable
            on_focus_change={blurred}
            on_activate={engage.clone()}
            on_key={key}
            on_motion={move |motion: Vec2| on_motion.call(motion)}
        >
            <ClickCatcher
                cursor={cursor}
                on_click={engage}
                on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
                children={content_node}
            />
        </Focusable>
    }
}

fn publish(locked: bool) {
    try_with_document(|document| {
        if let Some(context) = document.context() {
            context.set_pointer_locked(locked);
        }
    });
}
