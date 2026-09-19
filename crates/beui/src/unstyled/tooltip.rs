use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_macros::{component, view};

use crate::base::overlay::{Overlay, Placement};
use crate::input::PointerPress;
use crate::node::NodeId;
use crate::reactive::{
    Child, ClickCatcher, List, NodeRef, Prop, ReadSignal, Render, clone, create_memo,
    create_signal, each_frame, with_document,
};

pub const TOOLTIP_DELAY: Duration = Duration::from_millis(450);

pub struct TooltipHandle {
    pub label: Prop<String>,
    pub shown: ReadSignal<bool>,
}

#[component]
pub fn Tooltip(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TOOLTIP_DELAY)] delay: Duration,
    children: Child,
    content: Render<TooltipHandle>,
) -> NodeId {
    let anchor = NodeRef::new();
    let (shown, set_shown) = create_signal(false);
    let dwelling: Rc<Cell<Option<Instant>>> = Rc::default();
    let wanted = create_memo(clone!(label disabled -> move || {
        !disabled.get() && !label.get().is_empty()
    }));
    let open = create_memo(clone!(shown wanted -> move || shown.get() && wanted.get()));

    each_frame(clone!(dwelling set_shown -> move || {
        let Some(since) = dwelling.get() else {
            return;
        };
        let waited = since.elapsed();
        if waited >= delay {
            set_shown.set(true);
            return;
        }
        with_document(|document| document.request_repaint_after(delay - waited));
    }));

    let hover = clone!(dwelling set_shown -> move |hovered: bool| {
        dwelling.set(hovered.then(Instant::now));
        if !hovered {
            set_shown.set(false);
        }
    });
    let press = clone!(dwelling set_shown -> move |_: PointerPress| {
        dwelling.set(None);
        set_shown.set(false);
    });
    let bubble = content.call(TooltipHandle { label, shown });

    view! {
        <ClickCatcher on_hover_change={hover} on_press={press}>
            <List @node_ref=&anchor spacing=0.0>
                {children}
                <Overlay
                    anchor=&anchor
                    placement=Placement::BelowStart
                    modal=false
                    traps_focus=false
                    open={open}
                >
                    {bubble}
                </Overlay>
            </List>
        </ClickCatcher>
    }
}
