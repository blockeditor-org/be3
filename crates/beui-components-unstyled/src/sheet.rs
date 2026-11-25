//! A panel rising from the bottom of a narrow screen, as one vertical `Scroll`.
//!
//! The scroll's viewport is as tall as the sheet's highest stop, and its content is the space
//! above the sheet (as tall as the viewport) and then the sheet itself. Its one offset is both
//! how far the sheet is raised and, past its highest stop, how far its content has scrolled:
//! pulling up raises the sheet until it is open and then scrolls what it holds, and a pull from
//! the content's top lowers it. The stops are the scroll's snap stops, with 0 (lowered out of
//! sight) among them: a release that settles there closes it. The grip is a sticky item, so it
//! stays put while the content scrolls under it, and the scroll's bar starts at the highest
//! stop, so it reads as the content's own. Taking the grip while the content is scrolled
//! lowers the sheet at once: how far the content was scrolled is held as the content's own
//! shift until the sheet is back at its top with the grip let go, when it becomes the scroll
//! again. Pulled past the end of what it holds, the sheet's surface carries on down to the
//! bottom edge, so no gap opens under it. It rises as it opens, and settles, on the scroll's
//! spring.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::gestures;
use crate::scroll::{Scroll, ScrollApi, ScrollStyle, fade_edges};
use accesskit::Role;

use super::back_slide::BACK_DRAG_SHARE;
use super::modal::{Modal, ModalHandle, ModalMotion, ModalPlacement};
use beui_view::components::overlay::use_overlay_presence;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::Direction;
use beui_view::types::Extent;
use beui_view::types::ScrollPosition;
use beui_view::types::{
    BackGesture, CursorIcon, DragGesture, PointerInput, PointerPress, PointerStage, Vec2, Velocity,
    pos2, vec2,
};
use beui_view::types::{Key, KeyPress};

/// How near a stop the sheet counts as at it, and as lowered out of sight at 0.
const SETTLED: f32 = 0.5;
/// How soon after one sheet starts leaving another opening takes its place, and the other way.
const HANDOFF: Duration = Duration::from_millis(100);

pub struct SheetGripHandle {
    /// Whether the handle has the keyboard focus, for the grip's focus ring.
    pub focused: ReadSignal<bool>,
}

/// What a sheet leaving or entering offers the next, so a sheet opening just as another closes
/// takes its place at its height rather than both sliding.
#[derive(Clone, Copy)]
enum Offer {
    Leaving {
        at: Instant,
        from: f32,
        vanish: Callback<()>,
    },
    Entering {
        at: Instant,
        take: Callback<f32>,
    },
}

impl Offer {
    fn at(self) -> Instant {
        match self {
            Offer::Leaving { at, .. } | Offer::Entering { at, .. } => at,
        }
    }
}

#[derive(Clone, Default)]
struct Offers(Rc<Cell<Option<Offer>>>);

fn offers() -> Offers {
    tree_local(Offers::default)
}

/// The offer made within `HANDOFF` of now, taken.
fn take_offer() -> Option<Offer> {
    offers()
        .0
        .take()
        .filter(|offer| now().saturating_duration_since(offer.at()) < HANDOFF)
}

fn offer(offer: Offer) {
    offers().0.set(Some(offer));
}

/// Which place a sheet is settling on, by name rather than offset, so it follows the stops as
/// what the sheet holds is measured.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Aim {
    Rest,
    Stop(usize),
    Away,
}

/// Where a sheet stands, as offsets of its scroll: the highest it rises (the scroll's
/// viewport), its stops, lowest first, and the one it rests at when it opens.
#[derive(Clone, PartialEq, Debug, Default)]
struct Stops {
    top: f32,
    stops: Vec<f32>,
    rest: f32,
}

impl Stops {
    /// A sheet of `height` (its grip and what it holds) on a screen `extent` tall, with stops
    /// at `shares` of it, never taller than it holds; or, `fit`, one stop at its height, never
    /// above `rest` (unless it `grows` and is taller, when it rises to its highest share), past
    /// which what it holds scrolls.
    fn of(height: f32, extent: f32, shares: &[f32], rest: f32, fit: bool, grows: bool) -> Self {
        let highest = shares.iter().copied().fold(0.0, f32::max);
        if fit {
            let limit = match grows && height > rest * extent {
                true => highest.max(rest) * extent,
                false => rest * extent,
            };
            let top = height.min(limit).max(0.0);
            return Self {
                top,
                stops: vec![top],
                rest: top,
            };
        }
        let top = height.min(highest * extent).max(0.0);
        let mut stops: Vec<f32> = shares
            .iter()
            .map(|share| (share * extent).min(top))
            .collect();
        stops.sort_by(f32::total_cmp);
        stops.dedup_by(|a, b| (*a - *b).abs() < SETTLED);
        Self {
            top,
            stops,
            rest: (rest * extent).min(top),
        }
    }

    /// The offset `aim` stands for, which moves with the stops as the sheet is measured.
    fn offset(&self, aim: Aim) -> f32 {
        match aim {
            Aim::Rest => self.rest,
            Aim::Stop(index) => self.stops.get(index).copied().unwrap_or(self.top),
            Aim::Away => 0.0,
        }
    }

    /// The stop at `offset`, as an aim.
    fn aim_at(&self, offset: f32) -> Option<Aim> {
        if offset <= SETTLED {
            return Some(Aim::Away);
        }
        self.stops
            .iter()
            .position(|stop| (stop - offset).abs() < SETTLED)
            .map(Aim::Stop)
    }

    fn lowest(&self) -> f32 {
        self.stops.first().copied().unwrap_or(self.top)
    }

    /// The next stop from `at` up or down; the lowest is as far down as it goes.
    fn next(&self, at: f32, up: bool) -> Option<f32> {
        match up {
            true => self.stops.iter().copied().find(|stop| *stop > at + SETTLED),
            false => self
                .stops
                .iter()
                .rev()
                .copied()
                .find(|stop| *stop < at - SETTLED),
        }
    }
}

/// A panel rising from the bottom of a narrow screen: a vertical `Scroll` whose one offset
/// raises it and, past its highest stop, scrolls what it holds (the module's doc). It rests on
/// `stops` (shares of `extent`), never taller than it holds, and opens at `rest`; or, `fit`,
/// at its own height up to `rest` (or, `grows`, up to its highest stop when it is taller): past
/// that what it holds scrolls. A release that settles below its lowest stop, a tap above it, Escape and a
/// back gesture ask `on_close`; it goes only when `open` turns false, sliding down out of sight,
/// and then `on_gone`. `on_presence` hears how far it is up, from 0 to 1 at its lowest stop. A
/// sheet opening just as another closes in the same document starts at the other's height, so
/// the two read as one sheet changing content.
#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    #[prop(default = false)] grows: bool,
    #[prop(default = 0.0)] fade: f32,
    style: ScrollStyle,
    #[prop(default = ComponentRef::new())] scroll: ComponentRef<ScrollApi>,
    grip: Render<SheetGripHandle, NodeId>,
    panel: RenderFn<NodeId, NodeId>,
    on_presence: Callback<f32>,
    on_gone: Callback<()>,
    on_close: Callback<()>,
    children: NodeId,
) -> NodeId {
    let shares: Rc<[f32]> = Rc::from(stops);
    let extent = create_memo(move || extent.get().max(1.0));
    let height = create_signal(0.0_f32);
    let placed = {
        let shares = Rc::clone(&shares);
        create_memo(move || Stops::of(height.get(), extent.get(), &shares, rest, fit, grows))
    };
    let snaps = create_memo(move || {
        let mut snaps = vec![0.0];
        snaps.extend(placed.with(|placed| placed.stops.clone()));
        snaps
    });
    let top = create_memo(move || placed.with(|placed| placed.top));
    let position = create_signal(None::<ScrollPosition>);
    let raised = create_memo(move || position.get().map_or(0.0, |position| position.offset));
    let start = create_signal(0.0_f32);
    let gone = create_signal(!open.get_untracked());
    let leaving = create_signal(false);
    // Waiting to rise to its rest once laid out, from where it starts.
    let entering = create_signal(open.get_untracked());
    // The place it is settling on, and that place's offset when it set out for it.
    let aim = StoredCell::new(None::<Aim>);
    let aimed = StoredCell::new(0.0_f32);
    // How far what it holds was scrolled past its top when its grip was taken, held as that
    // content's own shift while the grip lowers the sheet, so it stays where it was scrolled.
    let kept = create_signal(0.0_f32);
    let gripped = create_signal(false);
    let moving_aside = StoredCell::new(false);
    let api = move || scroll.try_get();
    let laid_out = move || position.with_untracked(Option::is_some) && top.get_untracked() > 0.0;
    // Settles on `to` from `thrown`, the speed a gesture let it go at, or else from the speed
    // it is moving at, so a new aim bends its path rather than stopping it.
    let settle_on = move |to: Aim, thrown: Option<f32>| {
        let offset = placed.with_untracked(|placed| placed.offset(to));
        aim.set(Some(to));
        aimed.set(offset);
        if let Some(api) = api() {
            api.settle_to(offset, thrown.unwrap_or_else(|| api.velocity()));
        }
    };
    // Its stops moved (what it holds measured anew) while it rests on, or heads for, a place:
    // it follows that place.
    create_effect(move || {
        let goal = placed.with(|placed| aim.get().map(|aim| placed.offset(aim)));
        untrack(|| {
            let Some(goal) = goal else {
                return;
            };
            let was = aimed.get();
            if (goal - was).abs() < SETTLED || leaving.get() || gone.get() {
                return;
            }
            let moving = api().is_some_and(|api| api.animating());
            let there = (raised.get() - was).abs() < SETTLED;
            if !(moving || there) {
                return;
            }
            aimed.set(goal);
            if let Some(api) = api() {
                match moving {
                    true => api.settle_to(goal, 0.0),
                    false => api.scroll_to(goal),
                }
            }
        });
    });

    let vanish = Callback::new(move |()| {
        if !gone.is_alive() || gone.get_untracked() {
            return;
        }
        leaving.set(false);
        entering.set(false);
        gone.set(true);
        on_presence.call(0.0);
        on_gone.call(());
    });
    let take = Callback::new(move |from: f32| {
        start.set(from);
        if laid_out()
            && let Some(api) = api()
        {
            api.scroll_to(from);
            entering.set(false);
            settle_on(Aim::Rest, Some(0.0));
        }
    });
    let enter = move || {
        gone.set(false);
        leaving.set(false);
        kept.set(0.0);
        match take_offer() {
            Some(Offer::Leaving { from, vanish, .. }) => {
                vanish.call(());
                take.call(from);
            }
            _ => offer(Offer::Entering { at: now(), take }),
        }
        match laid_out() {
            true => settle_on(Aim::Rest, Some(0.0)),
            false => entering.set(true),
        }
    };
    if open.get_untracked() {
        untrack(enter);
    }
    create_effect(move || {
        if entering.get() && top.get() > 0.0 && position.with(Option::is_some) {
            untrack(|| {
                entering.set(false);
                settle_on(Aim::Rest, Some(0.0));
            });
        }
    });
    let shown = open.clone();
    create_effect_on(
        move || shown.get(),
        move |&now_open, was| {
            let Some(&was_open) = was else {
                return;
            };
            if now_open == was_open {
                return;
            }
            match now_open {
                true => enter(),
                false => {
                    entering.set(false);
                    leaving.set(true);
                    let from = raised.get_untracked();
                    if let Some(Offer::Entering { take, .. }) = take_offer()
                        && from > SETTLED
                    {
                        take.call(from);
                        vanish.call(());
                        return;
                    }
                    offer(Offer::Leaving {
                        at: now(),
                        from,
                        vanish,
                    });
                    match from > SETTLED && laid_out() {
                        true => settle_on(Aim::Away, None),
                        false => vanish.call(()),
                    }
                }
            }
        },
    );
    // Lowered out of sight while leaving, it is gone.
    create_effect(move || {
        if leaving.get() && raised.get() <= SETTLED {
            untrack(|| vanish.call(()));
        }
    });
    let lowest = create_memo(move || placed.with(Stops::lowest));
    create_effect(move || {
        if gone.get() || entering.get() {
            return;
        }
        let lowest = lowest.get();
        let presence = match lowest > 0.0 {
            true => (raised.get() / lowest).clamp(0.0, 1.0),
            false => 1.0,
        };
        untrack(|| on_presence.call(presence));
    });
    // A release that settles out of sight asks to close; kept open, it rises back.
    let close = on_close;
    let snapped = Callback::new(move |stop: f32| {
        aim.set(placed.with_untracked(|placed| placed.aim_at(stop)));
        aimed.set(stop);
        if stop > SETTLED || leaving.get_untracked() {
            return;
        }
        // Kept open, it rises back from the speed it was going.
        close.call(());
        if !leaving.get_untracked() {
            settle_on(Aim::Rest, None);
        }
    });
    // Where a back gesture took it from, and how fast it lowers it, on the frame clock, so
    // the settle once it is let go starts at that speed.
    let backed = StoredCell::new(None::<f32>);
    let tracked = store(RefCell::new(None::<Velocity>));
    let released = move || {
        tracked.with(|tracked| {
            tracked
                .borrow_mut()
                .take()
                .map_or(0.0, |tracked| tracked.get().y)
        })
    };
    on_back(open.clone(), move |gesture: BackGesture| match gesture {
        BackGesture::Started { .. } => {
            let from = raised.get_untracked();
            backed.set(Some(from));
            tracked.with(|tracked| {
                *tracked.borrow_mut() = Some(Velocity::new(now(), pos2(0.0, from)));
            });
        }
        BackGesture::Progressed(progress) => {
            if let (Some(from), Some(api)) = (backed.get(), api()) {
                let to = from * (1.0 - progress * BACK_DRAG_SHARE);
                api.scroll_to(to);
                tracked.with(|tracked| {
                    if let Some(tracked) = tracked.borrow_mut().as_mut() {
                        tracked.sample(now(), pos2(0.0, to));
                    }
                });
            }
        }
        BackGesture::Cancelled => {
            if let Some(from) = backed.take() {
                let thrown = released();
                match placed.with_untracked(|placed| placed.aim_at(from)) {
                    Some(to) => settle_on(to, Some(thrown)),
                    None => {
                        aimed.set(from);
                        if let Some(api) = api() {
                            api.settle_to(from, thrown);
                        }
                    }
                }
            }
        }
        BackGesture::Invoked => {
            backed.set(None);
            // It sets out back to its rest at the speed the gesture let it go at, and its
            // owner closing it, now or once this handler is done, turns it away from that
            // speed.
            settle_on(Aim::Rest, Some(released()));
            close.call(());
        }
    });

    let focused = create_signal(false);
    let grip = grip.call(SheetGripHandle {
        focused: focused.read_only(),
    });
    let step = Callback::new(move |up: bool| {
        let from = match api().is_some_and(|api| api.animating()) {
            true => aimed.get(),
            false => raised.get_untracked(),
        };
        let next = placed.with_untracked(|placed| {
            let stop = placed.next(from, up)?;
            placed.aim_at(stop)
        });
        if let Some(to) = next {
            settle_on(to, None);
        }
    });
    // Taking the grip moves the scroll past its top into `kept`, so the grip lowers the sheet
    // at once rather than first scrolling what it holds back.
    let grip_taken = Callback::new(move |taken: bool| {
        gripped.set(taken);
        let past = raised.get_untracked() - top.get_untracked();
        if taken
            && past > SETTLED
            && let Some(api) = api()
        {
            kept.update(|kept| *kept += past);
            moving_aside.set(true);
            api.scroll_to(top.get_untracked());
        }
    });
    // What was kept becomes the scroll again, looking just as it did, once the sheet is back
    // at its top with the grip let go, or as soon as anything scrolls it past its top (a drag
    // going on), so dragging, the wheel and the keys go on from where it shows and the
    // content never runs past its end.
    create_effect(move || {
        let (raised, top) = (raised.get(), top.get());
        // The scroll has yet to report the move to the top that set `kept` aside.
        if moving_aside.get() {
            match raised > top + SETTLED {
                true => return,
                false => moving_aside.set(false),
            }
        }
        let back = kept.get() > 0.0
            && (raised > top + SETTLED
                || (raised >= top - SETTLED
                    && !gripped.get()
                    && !api().is_some_and(|api| api.animating())));
        if back {
            untrack(|| {
                let by = kept.get();
                kept.set(0.0);
                if let Some(api) = api() {
                    api.shift(by);
                }
            });
        }
    });
    let drag = Callback::new(move |gesture: DragGesture| {
        if let Some(api) = api() {
            api.drag(gesture);
        }
    });
    let edges = create_memo(move || {
        let mut edges = fade_edges(Direction::Vertical, position.get(), fade);
        // The grip sticks at the top, so only the bottom fades.
        edges.top = 0.0;
        edges
    });
    let sheet = panel.call(view! {
        <List spacing=0.0>
            <ListItem sticky={move || height.get() > top.get() + SETTLED}>
                <SheetHandle placed raised extent focused step drag grip_taken>{grip}</SheetHandle>
            </ListItem>
            <Offset fit=true offset={move || kept.get()}>{children}</Offset>
        </List>
    });
    let measured = measure(sheet);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            height.set(measured.size.y);
        }
    });
    let length = create_memo(move || match gone.get() {
        true => 0.0,
        false => top.get(),
    });
    // Pulled past the end of what it holds, the sheet's surface carries on down to the bottom
    // edge rather than leaving a gap the page shows through.
    let open_below = create_memo(move || {
        position.get().map_or(0.0, |position| {
            (position.viewport - (position.content - position.offset)).max(0.0)
        })
    });
    let filling = create_memo(move || open_below.get() > 0.0);
    view! {
        <Frame height={move || Extent::px(length.get())} fade_edges={edges}>
            <Layers>
                <Show value={filling}>|| {
                    let filler = panel.call(view! {
                        <Frame />
                    });
                    view! {
                        <List spacing=0.0>
                            <ListItem grow=1.0>
                                <Frame />
                            </ListItem>
                            <ListItem length={move || Some(open_below.get())}>{filler}</ListItem>
                        </List>
                    }
                }</Show>
                <Scroll
                    api=scroll
                    offset={start}
                    snaps={snaps}
                    keep_past_last_stop=true
                    on_snap={snapped}
                    bar_from={top}
                    style={style.fading(0.0)}
                    on_change={move |at: ScrollPosition| position.set(Some(at))}
                >
                    <ListItem length={move || Some(top.get())}>
                        <Interactive on_click={move || close.call(())}>
                            <Frame />
                        </Interactive>
                    </ListItem>
                    {sheet}
                </Scroll>
            </Layers>
        </Frame>
    }
}

#[component]
pub fn ModalSheet(
    open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    #[prop(default = false)] grows: bool,
    #[prop(default = 0.0)] fade: f32,
    style: ScrollStyle,
    scrim: Prop<Color>,
    grip: RenderFn<SheetGripHandle, NodeId>,
    panel: RenderFn<NodeId, NodeId>,
    /// A swipe away, Escape and back, and a press outside unless `on_outside` takes it.
    on_close: Callback<()>,
    /// A press outside the sheet, on its scrim, in place of `on_close`: a blocking dialog
    /// calls attention to itself there rather than closing.
    on_outside: Option<Callback<()>>,
    #[prop(children)] content: BuildFn<NodeId>,
) -> NodeId {
    let shown = open.clone();
    let outside = on_outside.unwrap_or(on_close);
    view! {
        <Modal
            open
            scrim
            placement=ModalPlacement::Sheet
            motion=ModalMotion::Reported
            on_dismiss={on_close}
        >|modal: ModalHandle|
            <ModalSheetBody
                open={shown.clone()}
                rest
                stops={stops.clone()}
                fit
                grows
                fade
                style
                grip={move |handle: SheetGripHandle| grip.call(handle)}
                panel
                on_presence={modal.presence}
                on_close={on_close}
                on_outside={outside}
            >
                {content.call()}
            </ModalSheetBody>
        </Modal>
    }
}

#[component]
fn ModalSheetBody(
    open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    fit: bool,
    grows: bool,
    fade: f32,
    style: ScrollStyle,
    grip: Render<SheetGripHandle, NodeId>,
    panel: RenderFn<NodeId, NodeId>,
    on_presence: Callback<f32>,
    on_close: Callback<()>,
    on_outside: Callback<()>,
    children: NodeId,
) -> NodeId {
    let gone = create_signal(false);
    let reopened = open.clone();
    create_effect(move || {
        if reopened.get() {
            gone.set(false);
        }
    });
    if let Some(presence) = use_overlay_presence() {
        presence.linger_while(create_memo(move || !gone.get()));
    }
    let scroll = ComponentRef::<ScrollApi>::new();
    let extent = create_signal(0.0);
    let highest = match fit && !grows {
        true => rest,
        false => stops.iter().copied().fold(rest, f32::max),
    };
    let built = view! {
        <List spacing=0.0>
            <ListItem grow=1.0 min={move || extent.get() * (1.0 - highest)}>
                <Interactive
                    @test_id={"sheet.outside"}
                    on_click={on_outside}
                    on_pointer={gestures::scroll_drag(Some(Direction::Vertical), move |gesture: DragGesture| {
                        if let Some(scroll) = scroll.try_get() { scroll.drag(gesture); }
                    })}
                >
                    <Frame />
                </Interactive>
            </ListItem>
            <ListItem shrink=1.0>
                <Sheet
                    extent
                    open
                    rest
                    stops
                    fit
                    grows
                    fade
                    style
                    scroll
                    grip
                    panel
                    on_presence={move |presence: f32| on_presence.call(presence)}
                    on_gone={move || gone.set(true)}
                    on_close={on_close}
                >
                    {children}
                </Sheet>
            </ListItem>
        </List>
    };
    let measured = measure(built);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            extent.set(measured.size.y);
        }
    });
    built
}

/// The sheet's handle: focused (by Tab, never by a press, so a drag on a phone keeps the field
/// being edited), Up and Down settle it on the next stop that way. Both keys are taken, pressed
/// and let go, even where no stop is that way, so they never scroll what holds the sheet. A
/// screen reader finds it as a slider named "Sheet height" whose value is the share of the
/// extent it stands at, read as the stop it rests at ("Stop 2 of 3"), which its Increment and
/// Decrement step (a phone's screen reader swiping up and down). A Tab stop, and a slider, only
/// when the sheet has more than one stop. A finger's drag on it drags the sheet's scroll, as
/// anywhere on the sheet; a mouse's drags it too, as a finger's would.
#[component]
fn SheetHandle(
    placed: Memo<Stops>,
    raised: Memo<f32>,
    extent: Memo<f32>,
    focused: Signal<bool>,
    step: Callback<bool>,
    drag: Callback<DragGesture>,
    grip_taken: Callback<bool>,
    children: NodeId,
) -> NodeId {
    let steps = create_memo(move || placed.with(|placed| placed.stops.len() > 1));
    let accessibility =
        crate::labelled_node(Role::Slider, None, Prop::Static("Sheet height".to_owned()));
    component_accessibility(create_memo(move || {
        let mut node = accessibility.get();
        if !steps.get() {
            return node;
        }
        let (stops, top) = placed.with(|placed| (placed.stops.clone(), placed.top));
        let at = raised.get().min(top);
        let share = at / extent.get();
        node.set_numeric_value(f64::from(share * 100.0));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(100.0);
        // Read as the stop it rests at, or the share it was let go at between them.
        node.set_value(
            match stops.iter().position(|stop| (stop - at).abs() < SETTLED) {
                Some(stop) => format!("Stop {} of {}", stop + 1, stops.len()),
                None => format!("{}%", (share * 100.0).round()),
            },
        );
        node
    }));
    // Where a mouse's press on it last was, while it is held.
    let pressed = StoredCell::new(None::<f32>);
    view! {
        <Interactive
            @test_id={"sheet.handle"}
            focusable=true
            tab_stop={steps}
            press_focus=false
            on_step={move |delta: f32| step.call(delta > 0.0)}
            on_focus_change={focused}
            on_key={move |press: KeyPress| {
                let up = match press.key {
                    Key::ArrowUp => true,
                    Key::ArrowDown => false,
                    _ => return false,
                };
                if press.pressed {
                    step.call(up);
                }
                true
            }}
            cursor=CursorIcon::ResizeVertical
            on_pointer={move |input: PointerInput| {
                match input.stage {
                    PointerStage::Down => grip_taken.call(true),
                    PointerStage::Up | PointerStage::Cancel => grip_taken.call(false),
                    PointerStage::Move => {}
                }
                false
            }}
            on_press={move |press: PointerPress| {
                pressed.set(Some(press.pos.y));
                drag.call(DragGesture { started: true, ..still() });
            }}
            on_drag={move |press: PointerPress| {
                let Some(last) = pressed.get() else {
                    return;
                };
                pressed.set(Some(press.pos.y));
                drag.call(DragGesture { delta: vec2(0.0, press.pos.y - last), ..still() });
            }}
            on_cancel={move || {
                if pressed.take().is_some() {
                    drag.call(DragGesture { cancelled: true, ..still() });
                }
            }}
            on_active_change={move |active: bool| {
                if !active {
                    grip_taken.call(false);
                }
                if !active && pressed.take().is_some() {
                    drag.call(DragGesture { ended: true, ..still() });
                }
            }}
            children={Some(children)}
        />
    }
}

/// A drag gesture that moved nothing, to fill in from.
fn still() -> DragGesture {
    DragGesture {
        started: false,
        ended: false,
        cancelled: false,
        delta: Vec2::ZERO,
        velocity: Vec2::ZERO,
    }
}
