use beui_view::types::LayerPolicy;
use std::time::Duration;

use beui_macros::{component, view};

use crate::anchored::SheetWhen;
use crate::anchored::{Anchored, AnchoredHandle, AnchoredSheetHandle, AnchoredStyle, Dropdown};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::PointerPress;
use beui_view::types::{Placement, Pos2, Rect};

pub const TOOLTIP_DELAY: Duration = Duration::from_millis(450);

pub struct TooltipHandle {
    pub label: Prop<String>,
    pub shown: Memo<bool>,
}

/// Shows `label` after the pointer rests on the child. The bubble is an inert
/// overlay, so the child keeps answering the pointer and Escape, and the dwell is
/// a timer started on hover, so an unhovered tooltip costs no frames. The dwell
/// and a press or the pointer leaving write `open`, as an owner may; it shows only
/// while there is a label and it is not disabled.
#[component]
pub fn Tooltip(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TOOLTIP_DELAY)] delay: Duration,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = None)] at: Binding<Option<Pos2>>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    #[prop(default = false)] keep_alive: bool,
    children: Child,
    content: RenderFn<TooltipHandle, NodeId>,
) -> NodeId {
    let anchor = NodeRef::new();
    let wanted = create_memo({
        let label = label.clone();
        move || !disabled.get() && !label.with(String::is_empty)
    });
    let showing = create_memo(move || open.get() && wanted.get());
    let shown = Binding::new(showing, move |now: bool| open.set(now));

    let dwell = create_timer(move || {
        open.set(true);
        None
    });
    let hover = move |hovered: bool| match hovered {
        true => dwell.restart(delay),
        false => {
            dwell.stop();
            open.set(false);
        }
    };
    let press = move |_: PointerPress| {
        dwell.stop();
        open.set(false);
    };
    let style = AnchoredStyle {
        dropdown: RenderFn::new(|dropdown: Dropdown| dropdown.content),
        sheet: RenderFn::new(|handle: AnchoredSheetHandle| handle.content.call()),
    };

    view! {
        <Interactive on_hover_change={hover} on_press={press}>
            <List @node_ref=&anchor spacing=0.0>
                {children}
                <Anchored
                    style
                    anchor=anchor
                    at
                    placement
                    bounds
                    keep_alive
                    sheet=SheetWhen::Never
                    policy=LayerPolicy::Inert
                    traps_focus=false
                    open={shown}
                >|handle: AnchoredHandle| {
                    content.call(TooltipHandle { label: label.clone(), shown: handle.shown })
                }</Anchored>
            </List>
        </Interactive>
    }
}
