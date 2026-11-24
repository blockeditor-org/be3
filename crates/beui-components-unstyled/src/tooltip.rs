use beui_view::types::LayerPolicy;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::anchored::SheetWhen;
use crate::anchored::{Anchored, AnchoredHandle, AnchoredSheetHandle, AnchoredStyle, Dropdown};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Key, KeyPhase, PointerPress};
use beui_view::types::{Placement, Pos2, Rect};

pub const TOOLTIP_DELAY: Duration = Duration::from_millis(450);

/// How long after the last tooltip closes another still opens at once, without its delay.
pub const TOOLTIP_COOL_DOWN: Duration = Duration::from_millis(300);

pub struct TooltipHandle {
    pub label: Prop<String>,
    pub shown: Memo<bool>,
}

/// The warm-up every tooltip in a document shares: while one is shown, or for
/// `TOOLTIP_COOL_DOWN` after the last one closed, the next opens at once, so a pointer
/// running along a toolbar waits the delay only once.
#[derive(Clone)]
struct Warmth(Rc<WarmthState>);

#[derive(Default)]
struct WarmthState {
    shown: Cell<usize>,
    closed_at: Cell<Option<Instant>>,
}

impl Warmth {
    fn get() -> Self {
        tree_local(|| Warmth(Rc::default()))
    }

    fn warm(&self) -> bool {
        self.0.shown.get() > 0
            || self
                .0
                .closed_at
                .get()
                .is_some_and(|closed| now().saturating_duration_since(closed) < TOOLTIP_COOL_DOWN)
    }

    fn shown(&self, shown: bool) {
        let count = &self.0.shown;
        match shown {
            true => count.set(count.get() + 1),
            false => {
                count.set(count.get().saturating_sub(1));
                self.0.closed_at.set(Some(now()));
            }
        }
    }
}

/// Shows `label` after the pointer rests on the child, or at once while the child holds the
/// keyboard's focus. The bubble is an inert overlay, so the child keeps answering the pointer
/// and Escape, and the dwell is a timer started on hover, so an unhovered tooltip costs no
/// frames. Escape hides a shown one until the pointer or the focus comes back. Tooltips share one warm-up: while one is shown or just closed, the next skips its
/// delay. The dwell, focus, a press and the pointer leaving write `open`, as an owner may; it
/// shows only while there is a label and it is not disabled, and its label describes the child
/// to assistive technology (where the child has no description of its own and another label).
#[component]
pub fn Tooltip(
    label: Prop<String>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = TOOLTIP_DELAY)] delay: Duration,
    #[prop(default = false)] open: Binding<bool>,
    #[prop(default = None)] at: Binding<Option<Pos2>>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
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
    let description = label.clone();
    bind_description(
        children,
        create_memo(move || match wanted.get() {
            true => description.get(),
            false => String::new(),
        }),
    );

    let warmth = Warmth::get();
    let counted = Rc::new(Cell::new(false));
    create_effect({
        let (warmth, counted) = (warmth.clone(), counted.clone());
        move || {
            let now = showing.get();
            if counted.replace(now) != now {
                warmth.shown(now);
            }
        }
    });
    on_cleanup({
        let warmth = warmth.clone();
        move || {
            if counted.replace(false) {
                warmth.shown(false);
            }
        }
    });

    let hovered = create_signal(false);
    let keyed = focus_ring(holds_focus(anchor));
    let dwell = create_timer(move || {
        open.set(true);
        None
    });
    let mut was_keyed = false;
    create_effect(move || {
        let now = keyed.get();
        if now == was_keyed {
            return;
        }
        was_keyed = now;
        untrack(|| match now {
            true => {
                dwell.stop();
                open.set(true);
            }
            false if !hovered.get() => open.set(false),
            false => {}
        });
    });
    // Escape hides a shown tooltip (WCAG 1.4.13), whether hover or focus showed it, and it
    // stays hidden until the pointer comes back or the focus leaves and returns. It hears
    // Escape only while shown, and only once what holds the focus has passed on it: on its own
    // control as the key bubbles up from the focus (before the window's layers dismiss
    // anything), or at the window once nothing took it. So a field, a menu or an embedded
    // program that answers Escape keeps it.
    let escape = move |event: KeyEvent| {
        let Some(press) = event.press() else {
            return false;
        };
        if press.key != Key::Escape || !showing.get_untracked() {
            return false;
        }
        if press.pressed {
            dwell.stop();
            open.set(false);
        }
        true
    };
    create_effect(move || {
        if !showing.get() {
            return;
        }
        untrack(|| {
            on_key(
                KeyAt::Control(anchor),
                KeyPhase::Bubble,
                KeyReveal::Nothing,
                escape,
            );
            on_key(KeyAt::Window, KeyPhase::Bubble, KeyReveal::Nothing, escape);
        });
    });
    let hover = move |now: bool| {
        hovered.set(now);
        match now {
            true if warmth.warm() => open.set(true),
            true => dwell.restart(delay),
            false => {
                dwell.stop();
                if !keyed.get_untracked() {
                    open.set(false);
                }
            }
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
