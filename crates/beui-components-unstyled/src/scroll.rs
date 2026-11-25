use beui_view::types::LayerPolicy;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use accesskit::{Action, Node, Role};

use super::fling::Fling;
use super::rubber_band::{MINIMUM_VELOCITY, Spring, bounce, rubber_band, spring_back};
use super::watched::Watched;
use crate::drag_board::drag_board;
use crate::gestures::{self, AUTOSCROLL_DEAD_ZONE};
use beui_view::components::overlay::Overlay;
use beui_view::drag::EdgeScroller;
use beui_view::escape::beui_html::{
    Event as HtmlEvent, Node as HtmlNode, attr, elements as html, on,
};
use beui_view::escape::{Escape, html_escape};
use beui_view::primitives::*;
use beui_view::reactive::KeyAt;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Claim;
use beui_view::types::KeyPhase;
use beui_view::types::Outline;
use beui_view::types::Placement;
use beui_view::types::Pos2;
use beui_view::types::Sides;
use beui_view::types::{
    AutoscrollGesture, CursorIcon, DragGesture, Key, KeyPress, PointerPress, ScrollGesture, Vec2,
    WHEEL_LATCH_TIMEOUT, WheelInput,
};
use beui_view::types::{Color, Direction, Justify, ScrollPosition};
use beui_view::{CORE, Core};

const STEP: f32 = 40.0;
const EDGE_SLACK: f32 = 0.5;
/// How near a snap stop, and how slow, a scroll settling on it simply stops there.
const SNAP_SLACK: f32 = 0.5;
const END_SLACK: f32 = 1.0;
const AUTOSCROLL_GAIN: f32 = 6.0;
const AUTOSCROLL_ACCELERATION: f32 = 0.02;

/// How long the bar stays fully shown after the offset changes or the pointer
/// leaves it, before fading out over [`SCROLLBAR_FADE_OUT`] on a timer that stops
/// once hidden.
pub const SCROLLBAR_LINGER: Duration = Duration::from_millis(1000);
pub const SCROLLBAR_FADE_OUT: Duration = Duration::from_millis(250);

pub struct ScrollHandle {
    pub position: Memo<ScrollPosition>,
    pub direction: Prop<Direction>,
    pub scroll_to: Callback<f32>,
    pub visibility: ReadSignal<f32>,
    pub on_engage: Callback<bool>,
}

/// How a scroll's own parts look: the spring its content springs back on when
/// pulled past an end, its bar, how far its edges fade, the marker an
/// autoscroll (a middle-button press) shows where it started, and the browser's
/// bar on the HTML core. A scroll inside a component (a text area, a list)
/// takes the same look as a plain one, marker and all.
#[derive(Clone, Copy)]
pub struct ScrollStyle {
    rubber: Spring,
    bar: Option<(f32, RenderFn<ScrollHandle, NodeId>)>,
    marker: Option<RenderFn<Memo<Direction>, NodeId>>,
    fade: f32,
    browser: Option<(Prop<Color>, Prop<Color>)>,
}

impl ScrollStyle {
    /// A scroll whose overscroll springs back on `rubber`, with no bar, fade or
    /// marker.
    pub fn new(rubber: Spring) -> Self {
        Self {
            rubber,
            bar: None,
            marker: None,
            fade: 0.0,
            browser: None,
        }
    }

    /// The bar drawn along the scroll, `width` across.
    pub fn bar(self, width: f32, bar: RenderFn<ScrollHandle, NodeId>) -> Self {
        Self {
            bar: Some((width, bar)),
            ..self
        }
    }

    /// What an autoscroll shows at the point it started from, given the
    /// scroll's axis; nothing without one. Natively only: the browser draws
    /// its own.
    pub fn marker(self, marker: RenderFn<Memo<Direction>, NodeId>) -> Self {
        Self {
            marker: Some(marker),
            ..self
        }
    }

    /// The thumb and track colours of the browser's own bar, which every scroll
    /// on the HTML core shows in place of `bar`; thin, as `scrollbar-color`.
    /// Natively they do nothing.
    pub fn browser_colors<M1, M2>(
        self,
        thumb: impl IntoPropValue<Prop<Color>, M1>,
        track: impl IntoPropValue<Prop<Color>, M2>,
    ) -> Self {
        Self {
            browser: Some((thumb.into_prop_value(), track.into_prop_value())),
            ..self
        }
    }

    pub fn fading(self, fade: f32) -> Self {
        Self { fade, ..self }
    }

    pub fn fade(&self) -> f32 {
        self.fade
    }

    fn width(&self) -> f32 {
        self.bar.as_ref().map_or(0.0, |(width, _)| *width)
    }

    fn over(&self, handle: ScrollHandle) -> Children<ListItem> {
        match &self.bar {
            None => Children::default(),
            Some((width, bar)) => {
                let (width, bar) = (*width, bar.call(handle));
                Children::from(view! {
                    <ListItem length=width>{bar}</ListItem>
                })
            }
        }
    }
}

struct Reveal {
    held: Memo<bool>,
    visibility: Signal<f32>,
    poked: Cell<Instant>,
    timer: Cell<Option<Timer>>,
    animation: Cell<Option<Animation>>,
}

impl Reveal {
    fn show(&self) {
        self.fade_to(1.0);
    }

    fn fade_to(&self, visibility: f32) {
        self.visibility.set(visibility);
    }

    fn poke(&self) {
        self.poked.set(beui_view::reactive::now());
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        self.show();
        if let Some(timer) = self.timer.get() {
            timer.restart(SCROLLBAR_LINGER);
        }
    }

    fn step(&self) -> Option<Duration> {
        if self.held.get_untracked() {
            self.show();
            return None;
        }
        let elapsed = beui_view::reactive::now().duration_since(self.poked.get());
        if elapsed < SCROLLBAR_LINGER {
            self.show();
            return Some(SCROLLBAR_LINGER - elapsed);
        }
        let fading = (elapsed - SCROLLBAR_LINGER).as_secs_f32() / SCROLLBAR_FADE_OUT.as_secs_f32();
        let visibility = (1.0 - fading).max(0.0);
        self.fade_to(visibility);
        (visibility > 0.0).then_some(Duration::ZERO)
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
struct Watch {
    coasting: bool,
    moving: bool,
}

struct Momentum {
    rubber: Spring,
    stretch: f32,
    drag: Option<f32>,
    dragging: bool,
    velocity: f32,
    fling: Option<Fling>,
    autoscroll: f32,
    /// The offsets a fling and a release settle on; none settles anywhere.
    snaps: Vec<f32>,
    /// The snap stop being settled on.
    snap: Option<f32>,
    /// A stop a coast chose on its own, to tell `on_snap`.
    reached: Option<f32>,
    /// How low a coast goes: a wheel past the last stop scrolls only what lies past it.
    floor: Option<f32>,
    /// Whether the stop it settles on was asked for (`ScrollApi::settle_to`) rather than
    /// chosen by a release: that motion takes no press, so what is under it can be pressed.
    asked: bool,
}

impl Momentum {
    fn new(rubber: Spring) -> Self {
        Self {
            rubber,
            stretch: 0.0,
            drag: None,
            dragging: false,
            velocity: 0.0,
            fling: None,
            autoscroll: 0.0,
            snaps: Vec::new(),
            snap: None,
            reached: None,
            floor: None,
            asked: false,
        }
    }

    fn moving(&self) -> bool {
        self.stretch != 0.0
            || self.velocity != 0.0
            || self.fling.is_some()
            || self.snap.is_some()
            || self.autoscroll != 0.0
    }

    fn flinging(&self) -> bool {
        self.fling.is_some() && self.stretch == 0.0 && self.drag.is_none()
    }

    /// What effects watch of it: whether it coasts, and whether it moves at all.
    fn watched(&self) -> Watch {
        Watch {
            coasting: self.coasting(),
            moving: self.moving(),
        }
    }

    fn coasting(&self) -> bool {
        (self.fling.is_some() || (self.snap.is_some() && !self.asked) || self.stretch != 0.0)
            && self.drag.is_none()
    }

    /// The snap stop nearest where a coast from `offset` at the current velocity would come to
    /// rest, within the scroll's reach. Past the last stop there is none: it coasts freely, and
    /// one coming back below the last stop chooses there, from the speed it has left (a fling
    /// through a sheet's content that reaches its top goes on to lower the sheet).
    fn aim(&self, offset: f32, max_offset: f32) -> Option<f32> {
        let carried = Fling::new(self.velocity).map_or(0.0, |fling| fling.distance());
        let landing = offset + carried;
        let last = self.snaps.last()?.clamp(0.0, max_offset);
        if offset > last + SNAP_SLACK {
            return None;
        }
        if landing > last + SNAP_SLACK {
            // Past the last stop it moves freely (a sheet's content, past its top stop), but a
            // coast from short of it stops there: a flick up raises a sheet, not its content.
            return (offset < last - SNAP_SLACK).then_some(last);
        }
        self.snaps
            .iter()
            .map(|stop| stop.clamp(0.0, max_offset))
            .min_by(|a, b| (a - landing).abs().total_cmp(&(b - landing).abs()))
    }

    /// The next stop from `offset` the way `forwards` says, while `offset` is within the stops'
    /// reach (at or short of the last).
    fn next_stop(&self, offset: f32, forwards: bool, max_offset: f32) -> Option<f32> {
        let mut stops = self.snaps.iter().map(|stop| stop.clamp(0.0, max_offset));
        let last = self.snaps.last()?.clamp(0.0, max_offset);
        if offset > last + SNAP_SLACK {
            return None;
        }
        match forwards {
            true => stops.find(|stop| *stop > offset + SNAP_SLACK),
            false => stops.rev().find(|stop| *stop < offset - SNAP_SLACK),
        }
    }

    fn stop_fling(&mut self) {
        if self.flinging() {
            self.fling = None;
            self.velocity = 0.0;
        }
    }

    fn rest(&mut self) {
        self.stretch = 0.0;
        self.drag = None;
        self.velocity = 0.0;
        self.fling = None;
        self.snap = None;
        self.asked = false;
        self.floor = None;
        self.autoscroll = 0.0;
    }

    fn grab(&mut self, position: &ScrollPosition) {
        self.drag = Some(position.offset + self.stretch);
        self.velocity = 0.0;
        self.fling = None;
        self.snap = None;
        self.asked = false;
    }

    fn drag(&mut self, position: &mut ScrollPosition, delta: f32, banding: bool) {
        let raw = self.drag.unwrap_or(position.offset) - delta;
        position.offset = raw.clamp(0.0, position.max_offset());
        self.drag = Some(if banding { raw } else { position.offset });
        self.stretch = match banding {
            true => raw - position.offset,
            false => 0.0,
        };
        self.velocity = 0.0;
    }

    fn release(&mut self, velocity: f32, position: &ScrollPosition) {
        self.drag = None;
        self.asked = false;
        self.velocity = velocity;
        if self.stretch == 0.0 {
            self.coast(position);
        }
    }

    /// Coasts on from `position`: towards the snap stop the coast would land nearest, given
    /// stops, or else in a fling.
    fn coast(&mut self, position: &ScrollPosition) {
        if let Some(stop) = self.aim(position.offset, position.max_offset()) {
            self.fling = None;
            self.snap = Some(stop);
            return;
        }
        self.fling = (self.velocity.abs() >= MINIMUM_VELOCITY)
            .then(|| Fling::new(self.velocity))
            .flatten();
        self.velocity = self.fling.map_or(0.0, |fling| fling.velocity());
    }

    fn animate(&mut self, position: &mut ScrollPosition, elapsed: f32, banding: bool) {
        if elapsed <= 0.0 {
            return;
        }
        if self.autoscroll != 0.0 {
            let raw = position.offset + self.autoscroll * elapsed;
            position.offset = raw.clamp(0.0, position.max_offset());
            if raw != position.offset {
                self.autoscroll = 0.0;
            }
            return;
        }
        if self.stretch != 0.0 {
            let back = bounce(&mut self.stretch, &mut self.velocity, elapsed, self.rubber);
            if self.stretch == 0.0 {
                self.velocity = back;
                self.coast(position);
                if self.snap.is_some() {
                    self.reached = self.snap;
                }
            }
            return;
        }
        if let Some(stop) = self.snap {
            // Within what it can reach now: content still being measured moves it.
            let stop = stop.clamp(0.0, position.max_offset());
            let mut distance = position.offset - stop;
            spring_back(&mut distance, &mut self.velocity, elapsed, self.rubber);
            if distance.abs() < SNAP_SLACK && self.velocity.abs() < MINIMUM_VELOCITY {
                distance = 0.0;
                self.velocity = 0.0;
            }
            position.offset = stop + distance;
            if distance == 0.0 && self.velocity == 0.0 {
                self.snap = None;
            }
            return;
        }
        let Some(fling) = &mut self.fling else {
            self.velocity = 0.0;
            return;
        };
        let from = position.offset;
        let raw = position.offset + fling.advance(elapsed);
        self.velocity = fling.velocity();
        if fling.done() {
            self.fling = None;
        }
        // A free coast coming back below the last stop settles on a stop from there, or, held
        // to its floor (a wheel's), stops at the last.
        if let Some(last) = self
            .snaps
            .last()
            .map(|last| last.clamp(0.0, position.max_offset()))
            && from > last
            && raw <= last
        {
            position.offset = last;
            self.fling = None;
            if self.floor.is_some() {
                self.velocity = 0.0;
                return;
            }
            self.snap = self.aim(last, position.max_offset());
            if self.snap.is_some() {
                self.reached = self.snap;
            }
            return;
        }
        position.offset = raw.clamp(0.0, position.max_offset());
        if raw != position.offset {
            self.fling = None;
            match banding {
                true => self.stretch = raw - position.offset,
                false => self.velocity = 0.0,
            }
        }
    }
}

const EDGE_GAIN: f32 = 3.0;

/// A browser scroll's content box, as long as what it holds along the scroll and at least
/// as long as the scroll, clips what is painted past it: a lifted or gliding item's
/// translation would otherwise add to what the browser lets the scroll reach.
fn content_clip(direction: Direction) -> String {
    let (length, least) = match direction {
        Direction::Vertical => ("height", "min-height"),
        Direction::Horizontal => ("width", "min-width"),
    };
    format!(
        "display: grid; grid-template: minmax(0, 1fr) / minmax(0, 1fr); \
         {length}: max-content; {least}: 100%; overflow: clip"
    )
}

fn edge_scroll(
    node: NodeRef,
    direction: Prop<Direction>,
    position: impl Fn() -> Option<ScrollPosition> + 'static,
    scroll: impl Fn(Option<f32>) + 'static,
) {
    let board = drag_board();
    let edge = board.register_scroller(EdgeScroller {
        id: 0,
        node,
        movable: Rc::new(move || {
            let along = position().map_or([false; 2], |position| {
                [
                    position.offset > 0.0,
                    position.offset < position.max_offset(),
                ]
            });
            match direction.get_untracked() {
                Direction::Horizontal => [along, [false; 2]],
                Direction::Vertical => [[false; 2], along],
            }
        }),
        scroll: Rc::new(move |depth: Option<Vec2>| {
            scroll(
                depth
                    .map(|depth| direction.get_untracked().main(depth))
                    .map(|depth| (AUTOSCROLL_DEAD_ZONE + depth.abs() * EDGE_GAIN) * depth.signum()),
            );
        }),
    });
    on_cleanup(move || board.unregister_scroller(edge));
}

fn chaining(
    motion: Stored<Motion>,
    wheel: Callback<WheelInput, bool>,
) -> Callback<WheelInput, bool> {
    Callback::new(move |input: WheelInput| {
        if let WheelInput::Scroll(gesture) = &input
            && motion.value().wheel_chains(gesture)
        {
            return false;
        }
        wheel.call(input)
    })
}

fn crosswise(
    direction: Prop<Direction>,
    wheel: Callback<WheelInput, bool>,
) -> Callback<WheelInput, bool> {
    Callback::new(move |input: WheelInput| match input {
        WheelInput::Scroll(gesture)
            if gesture.unclaimed
                && direction.get_untracked() == Direction::Horizontal
                && gesture.delta.x == 0.0
                && gesture.fling.x == 0.0 =>
        {
            wheel.call(WheelInput::Scroll(ScrollGesture {
                delta: Vec2::new(gesture.delta.y, 0.0),
                fling: Vec2::new(gesture.fling.y, 0.0),
                ..gesture
            }))
        }
        input => wheel.call(input),
    })
}

fn autoscroll_speed(distance: f32) -> f32 {
    let beyond = (distance.abs() - AUTOSCROLL_DEAD_ZONE).max(0.0);
    distance.signum() * (beyond * AUTOSCROLL_GAIN + beyond * beyond * AUTOSCROLL_ACCELERATION)
}

fn rubber_banding() -> bool {
    motion().animates()
}

struct Motion {
    node: Option<NodeId>,
    reported: ReadSignal<Option<ScrollPosition>>,
    direction: Prop<Direction>,
    snaps: Prop<Vec<f32>>,
    keep_past_last_stop: bool,
    on_snap: Callback<f32>,
    /// A stop a release chose, told to `on_snap` once the momentum is let go.
    chosen: Cell<Option<f32>>,
    momentum: Watched<Momentum, Watch>,
    animation: Cell<Option<Animation>>,
    autoscrolling: Cell<Option<Timer>>,
    autoscrolled: Cell<Instant>,
    wheeled: Cell<Option<Instant>>,
    /// The offset it last drove the scroll to, without its stretch.
    driven: Cell<f32>,
}

impl Motion {
    fn axis(&self) -> Direction {
        self.direction.get_untracked()
    }

    fn placed(&self) -> Option<ScrollPosition> {
        let reported = self.reported.get_untracked()?;
        let node = self.node?;
        let offset = node.offset()?;
        let position = ScrollPosition { offset, ..reported };
        Some(ScrollPosition {
            offset: position.clamped(),
            ..position
        })
    }

    fn publish(&self, momentum: &Momentum, position: &ScrollPosition, offset: f32) {
        let Some(node) = self.node else {
            return;
        };
        self.driven.set(offset);
        node.drive_offset(offset + rubber_band(momentum.stretch, position.viewport));
    }

    fn settle(&self, position: ScrollPosition) -> bool {
        let Some(node) = self.node else {
            return false;
        };
        let Ok(momentum) = self.momentum.try_borrow() else {
            return false;
        };
        let stray = position.overscroll() != 0.0 && momentum.drag.is_none() && !momentum.moving();
        if stray {
            node.drive_offset(position.clamped());
        }
        stray
    }

    /// A press stops a fling where it is, and a release's settle on a stop goes to the stop
    /// nearest where it is, without the speed it had.
    fn stop_fling(&self) {
        let mut momentum = self.momentum.borrow_mut();
        momentum.stop_fling();
        // A settle asked for (a sheet rising as it opens) takes no press, so it goes on.
        if momentum.snap.is_some()
            && !momentum.asked
            && momentum.drag.is_none()
            && momentum.stretch == 0.0
            && let Some(position) = self.placed()
        {
            momentum.velocity = 0.0;
            momentum.snap = momentum.aim(position.offset, position.max_offset());
        }
    }

    fn chains(&self, along: f32) -> bool {
        if along == 0.0 {
            return false;
        }
        let Some(position) = self.placed() else {
            return false;
        };
        match along < 0.0 {
            true => position.offset >= position.max_offset() - EDGE_SLACK,
            false => position.offset <= EDGE_SLACK,
        }
    }

    fn wheel_chains(&self, gesture: &ScrollGesture) -> bool {
        let along = self.axis().main(gesture.delta);
        if along == 0.0 {
            return false;
        }
        let latched = self
            .wheeled
            .get()
            .is_some_and(|last| now().saturating_duration_since(last) < WHEEL_LATCH_TIMEOUT);
        if !latched && !gesture.unclaimed && self.chains(along) {
            return true;
        }
        self.wheeled.set(Some(now()));
        false
    }

    fn wheel(&self, gesture: ScrollGesture) {
        self.wheel_moves(gesture);
        self.report();
    }

    fn wheel_moves(&self, gesture: ScrollGesture) {
        let wheel = self.axis().main(gesture.delta);
        let fling = self.axis().main(gesture.fling);
        let Some(mut position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        if !gesture.held && momentum.drag.is_none() && wheel == 0.0 && fling == 0.0 {
            return;
        }
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        if gesture.held && rubber_banding() {
            if momentum.drag.is_none() {
                momentum.grab(&position);
            }
            momentum.drag(&mut position, wheel, true);
            self.publish(&momentum, &position, position.offset);
            return;
        }
        if momentum.drag.is_some() && !momentum.dragging {
            self.release(&mut momentum, -fling, &position);
            self.publish(&momentum, &position, position.offset);
            self.animate(&mut momentum);
            return;
        }
        if wheel == 0.0 && fling == 0.0 {
            return;
        }
        momentum.rest();
        let mut offset = (position.offset - wheel).clamp(0.0, position.max_offset());
        self.take_snaps(&mut momentum);
        // Up to the last stop a wheel steps between stops; past it, it scrolls freely and comes
        // back no further than the last stop. Kept past the last stop, it never goes back below
        // it, and short of it only steps forwards.
        let last = momentum
            .snaps
            .last()
            .map(|last| last.clamp(0.0, position.max_offset()));
        if let Some(last) = last {
            let forwards = wheel < 0.0;
            let at_last = (position.offset - last).abs() <= SNAP_SLACK;
            let steps = match position.offset < last - SNAP_SLACK {
                true => forwards || !self.keep_past_last_stop,
                false => at_last && !forwards && !self.keep_past_last_stop,
            };
            if steps {
                if let Some(stop) =
                    momentum.next_stop(position.offset, forwards, position.max_offset())
                {
                    momentum.snap = Some(stop);
                    self.chosen.set(Some(stop));
                    self.animate(&mut momentum);
                }
                return;
            }
            if position.offset < last - SNAP_SLACK {
                return;
            }
            offset = offset.max(last);
        }
        self.publish(&momentum, &position, offset);
        if fling != 0.0 && rubber_banding() {
            let moved = ScrollPosition { offset, ..position };
            self.release(&mut momentum, -fling, &moved);
            momentum.floor = last;
            self.animate(&mut momentum);
        }
    }

    fn drag(&self, gesture: DragGesture) {
        self.drag_moves(gesture);
        self.report();
    }

    fn drag_moves(&self, gesture: DragGesture) {
        if gesture.started
            && let Some(animation) = self.animation.get()
        {
            animation.stop();
        }
        let Some(mut position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        momentum.dragging = !gesture.ended && !gesture.cancelled;
        if momentum.drag.is_none() {
            momentum.grab(&position);
        }
        let dragged = self.axis().main(gesture.delta);
        if dragged != 0.0 {
            momentum.drag(&mut position, dragged, rubber_banding());
        }
        if gesture.ended {
            // Without motion the velocity still chooses a stop, but carries nothing on.
            let velocity = -self.axis().main(gesture.velocity);
            self.release(&mut momentum, velocity, &position);
        } else if gesture.cancelled {
            self.release(&mut momentum, 0.0, &position);
        }
        self.publish(&momentum, &position, position.offset);
        if gesture.ended || gesture.cancelled {
            self.animate(&mut momentum);
        }
    }

    fn autoscroll(&self, gesture: AutoscrollGesture) {
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        let mut momentum = self.momentum.borrow_mut();
        if gesture.ended {
            momentum.autoscroll = 0.0;
            return;
        }
        if momentum.stretch != 0.0
            && let Some(position) = self.placed()
        {
            momentum.rest();
            self.publish(&momentum, &position, position.offset);
        }
        momentum.drag = None;
        momentum.velocity = 0.0;
        momentum.fling = None;
        momentum.autoscroll = autoscroll_speed(self.axis().main(gesture.pos - gesture.origin));
        drop(momentum);
        self.autoscrolled.set(now());
        self.autoscrolling.get().unwrap().start(Duration::ZERO);
    }

    /// Lets go at `velocity` from `position`, settling on a snap stop if the scroll has any.
    fn release(&self, momentum: &mut Momentum, velocity: f32, position: &ScrollPosition) {
        self.take_snaps(momentum);
        momentum.release(velocity, position);
        if !rubber_banding() && momentum.snap.is_none() {
            momentum.fling = None;
            momentum.velocity = 0.0;
        }
        self.chosen.set(momentum.snap);
    }

    fn take_snaps(&self, momentum: &mut Momentum) {
        let mut snaps = self.snaps.get_untracked();
        snaps.sort_by(f32::total_cmp);
        momentum.snaps = snaps;
    }

    /// Tells `on_snap` the stop a release chose, with the momentum let go.
    fn report(&self) {
        if let Some(stop) = self.chosen.take() {
            self.on_snap.call(stop);
        }
    }

    /// Settles on `offset` as on a snap stop, starting at `velocity`: on the overscroll spring,
    /// or at once without motion.
    fn settle_to(&self, offset: f32, velocity: f32) {
        if self.placed().is_none() {
            return;
        }
        let mut momentum = self.momentum.borrow_mut();
        momentum.rest();
        // Clamped as it moves, to what it can reach then.
        momentum.snap = Some(offset.max(0.0));
        momentum.velocity = velocity;
        momentum.asked = true;
        self.animate(&mut momentum);
    }

    fn animate(&self, momentum: &mut Momentum) {
        if !rubber_banding() {
            // Without motion a scroll settling on a stop goes straight there.
            if let Some(stop) = momentum.snap.take()
                && let Some(position) = self.placed()
            {
                momentum.velocity = 0.0;
                self.publish(momentum, &position, stop.clamp(0.0, position.max_offset()));
            }
            return;
        }
        let Some(animation) = self.animation.get() else {
            return;
        };
        if momentum.moving() {
            animation.start();
        }
    }

    fn shift(&self, by: f32) {
        let Some(position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        if let Some(hold) = &mut momentum.drag {
            *hold += by;
        }
        let offset = (position.offset + by).clamp(0.0, position.max_offset());
        self.publish(&momentum, &position, offset);
    }

    fn scroll_to(&self, offset: f32) {
        let Some(position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        momentum.rest();
        self.publish(
            &momentum,
            &position,
            offset.clamp(0.0, position.max_offset()),
        );
    }

    fn to_end(&self) {
        let Some(node) = self.node else {
            return;
        };
        if let Some(animation) = self.animation.get() {
            animation.stop();
        }
        self.momentum.borrow_mut().rest();
        node.offset_to_end();
    }

    fn key(&self, press: KeyPress) -> bool {
        if press.modifiers.ctrl || press.modifiers.alt {
            return false;
        }
        let Some(position) = self.placed().filter(|position| position.max_offset() > 0.0) else {
            return false;
        };
        let vertical = self.axis() == Direction::Vertical;
        let (forwards, backwards) = match vertical {
            false => (Key::ArrowRight, Key::ArrowLeft),
            true => (Key::ArrowDown, Key::ArrowUp),
        };
        let offset = match press.key {
            key if key == forwards => position.offset + STEP,
            key if key == backwards => position.offset - STEP,
            Key::PageDown | Key::Space if vertical && !press.modifiers.shift => {
                position.offset + position.viewport
            }
            Key::PageUp | Key::Space if vertical => position.offset - position.viewport,
            Key::Home => 0.0,
            Key::End => position.max_offset(),
            _ => return false,
        }
        .clamp(0.0, position.max_offset());
        let offset = self.keyed(&position, offset);
        if offset == position.offset {
            return false;
        }
        if press.pressed {
            let mut momentum = self.momentum.borrow_mut();
            momentum.rest();
            self.publish(&momentum, &position, offset);
        }
        true
    }

    /// Where a key that would go to `offset` goes, given stops: past the last stop it moves
    /// freely, coming back no further than the last stop; up to it, it steps to the next stop
    /// that way. Kept past the last stop, a key never takes it back below it.
    fn keyed(&self, position: &ScrollPosition, offset: f32) -> f32 {
        let mut momentum = self.momentum.borrow_mut();
        self.take_snaps(&mut momentum);
        let max_offset = position.max_offset();
        let Some(last) = momentum
            .snaps
            .last()
            .map(|last| last.clamp(0.0, max_offset))
        else {
            return offset;
        };
        let forwards = offset > position.offset;
        let at_last = (position.offset - last).abs() <= SNAP_SLACK;
        let past = position.offset > last + SNAP_SLACK;
        if past || (at_last && (forwards || self.keep_past_last_stop)) {
            return offset.max(last);
        }
        momentum
            .next_stop(position.offset, forwards, max_offset)
            .unwrap_or(position.offset)
    }

    /// Moved from outside (a reveal), it stops. A settle asked for goes on, as it does through
    /// a press, while the move took it towards its stop and not past it: what a scroll holds
    /// past its stops shows at least as well there (a sheet rising as it opens with focus on
    /// its first row).
    fn steer(&self) {
        let Ok(mut momentum) = self.momentum.try_borrow_mut() else {
            return;
        };
        let from = self.driven.get();
        let goes_on = momentum.asked
            && momentum.drag.is_none()
            && momentum.stretch == 0.0
            && momentum
                .snap
                .zip(self.placed())
                .is_some_and(|(stop, position)| {
                    let stop = stop.clamp(0.0, position.max_offset());
                    let (to, from) = (stop - position.offset, stop - from);
                    to * from > 0.0 && to.abs() <= from.abs()
                });
        if !goes_on {
            momentum.rest();
        }
    }

    fn finish(&self) {
        let Some(position) = self.placed() else {
            return;
        };
        let mut momentum = self.momentum.borrow_mut();
        let autoscroll = momentum.autoscroll;
        let settled = momentum.snap.map_or(position.offset, |stop| {
            stop.clamp(0.0, position.max_offset())
        });
        momentum.rest();
        momentum.autoscroll = autoscroll;
        self.publish(&momentum, &position, settled);
    }

    fn step(&self, elapsed: f32) -> Option<Duration> {
        let node = self.node?;
        if !node.exists() {
            return None;
        }
        let mut momentum = self.momentum.borrow_mut();
        if std::mem::take(&mut momentum.dragging) {
            return Some(Duration::ZERO);
        }
        momentum.drag = None;
        if !momentum.moving() {
            return None;
        }
        let mut position = self.placed()?;
        momentum.animate(&mut position, elapsed, rubber_banding());
        self.publish(&momentum, &position, position.offset);
        let moving = momentum.moving();
        let reached = momentum.reached.take();
        drop(momentum);
        if let Some(stop) = reached {
            self.on_snap.call(stop);
        }
        moving.then_some(Duration::ZERO)
    }
}

#[component]
fn Scrolling(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ScrollApi>,
    direction: Prop<Direction>,
    snaps: Prop<Vec<f32>>,
    keep_past_last_stop: bool,
    on_snap: Callback<f32>,
    bar_from: Prop<f32>,
    fit: bool,
    bleed: Prop<Sides>,
    focus_ring: Prop<Option<Outline>>,
    style: ScrollStyle,
    on_change: Callback<ScrollPosition>,
    #[prop(children)] content: Render<Callback<ScrollPosition>, NodeId>,
) -> NodeId {
    let reported = create_signal(None::<ScrollPosition>);
    let settles = Rc::new(Cell::new(None::<Stored<Motion>>));
    let node = content.call(Callback::new({
        let settles = Rc::clone(&settles);
        move |position: ScrollPosition| {
            if settles
                .get()
                .is_some_and(|motion| motion.value().settle(position))
            {
                return;
            }
            reported.set(Some(position));
            on_change.call(position);
        }
    }));
    let motion = store(Motion {
        node: node.first_offset_within(),
        reported: reported.read_only(),
        direction,
        snaps,
        on_snap,
        chosen: Cell::new(None),
        momentum: Watched::new(Momentum::new(style.rubber), Momentum::watched),
        animation: Cell::default(),
        autoscrolling: Cell::default(),
        autoscrolled: Cell::new(now()),
        keep_past_last_stop,
        wheeled: Cell::new(None),
        driven: Cell::new(0.0),
    });
    settles.set(Some(motion));
    let animation = create_animation(
        move |frame| motion.value().step(frame.delta.as_secs_f32()).is_some(),
        move || motion.value().finish(),
    );
    let autoscrolling = create_timer(move || {
        let state = motion.value();
        if state.momentum.borrow().autoscroll == 0.0 {
            return None;
        }
        let elapsed = now().saturating_duration_since(state.autoscrolled.replace(now()));
        state.step(elapsed.as_secs_f32())
    });
    motion.with(|motion| motion.autoscrolling.set(Some(autoscrolling)));
    motion.with(|motion| motion.animation.set(Some(animation)));
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    create_effect(move || {
        if !animates.get() {
            motion.value().finish();
        }
    });
    if let Some(offset) = motion.with(|motion| motion.node) {
        offset.on_steer(move |()| motion.value().steer());
    }
    api.fill(ScrollApi(motion, Callback::empty(), Callback::empty()));

    let position = create_memo(move || reported.get().unwrap_or_default());
    let overflows = create_memo(move || position.get().max_offset() > 0.0);
    // What the bar shows: the offsets past `bar_from`, as a scroll of what lies past it.
    let barred = create_memo(move || {
        let from = bar_from.get().max(0.0);
        let position = position.get();
        if from == 0.0 {
            return position;
        }
        ScrollPosition {
            offset: (position.offset - from).max(0.0),
            content: (position.content - from).max(0.0),
            ..position
        }
    });
    component_accessibility(create_memo(move || {
        scroll_view(reported.get(), direction.get())
    }));

    let near = create_signal(false);
    let engaged = create_signal(false);
    let reveal = store(Reveal {
        held: create_memo(move || near.get() || engaged.get()),
        visibility: create_signal(0.0),
        poked: Cell::new(beui_view::reactive::now()),
        timer: Cell::default(),
        animation: Cell::default(),
    });
    let animation = create_animation(
        move |_| reveal.value().step().is_some(),
        move || {
            if !reveal.value().held.get_untracked() {
                reveal.value().fade_to(0.0);
            }
        },
    );
    reveal.with(|reveal| reveal.animation.set(Some(animation)));
    let fading = create_timer(move || {
        if !reveal.value().held.get_untracked() {
            animation.start();
            if animation.running() && reveal.value().step().is_none() {
                animation.finish();
            }
        }
        None
    });
    reveal.with(|reveal| reveal.timer.set(Some(fading)));
    let moved = Cell::new(None::<f32>);
    create_effect(move || {
        if reported.with(Option::is_none) {
            return;
        }
        let at = barred.get().offset;
        // At or short of `bar_from` there is no bar: what lies past it is not being scrolled
        // (a sheet's grip lowering it).
        let short = position.get().offset < bar_from.get().max(0.0) + EDGE_SLACK;
        if short && bar_from.get() > 0.0 {
            moved.set(Some(at));
            untrack(|| reveal.value().fade_to(0.0));
            return;
        }
        if moved.replace(Some(at)).is_some_and(|was| was != at) {
            reveal.value().poke();
        }
    });
    create_effect(move || {
        let held = reveal.with(|reveal| reveal.held.get());
        match held {
            true => {
                reveal.value().show();
                fading.stop();
                animation.stop();
            }
            false => {
                if reveal.with(|reveal| reveal.visibility.get_untracked()) > 0.0 {
                    reveal.value().poke();
                }
            }
        }
    });
    let bar_width = style.width();
    let bar = ScrollHandle {
        position: barred,
        direction,
        scroll_to: Callback::new(move |offset: f32| {
            motion
                .value()
                .scroll_to(offset + bar_from.get_untracked().max(0.0))
        }),
        visibility: reveal.with(|reveal| reveal.visibility.read_only()),
        on_engage: Callback::new(move |now: bool| {
            engaged.set(now);
        }),
    };

    let focused = create_signal(false);
    let across = across(&direction);
    let axis = create_memo(move || Some(direction.get()));
    let catcher = NodeRef::new();
    let origin = create_signal(None::<Pos2>);
    let marked = create_memo(move || origin.with(Option::is_some));
    let anchor = create_memo(move || OverlayAnchor::Point(origin.get().unwrap_or(Pos2::ZERO)));
    let marker_axis = create_memo(move || direction.get());
    let marker = match style.marker {
        Some(marker) => marker.call(marker_axis),
        None => view! {
            <List spacing=0.0 />
        },
    };
    on_key(
        KeyAt::Node(catcher),
        KeyPhase::Bubble,
        KeyReveal::Nothing,
        move |event| {
            event
                .press()
                .is_some_and(|press| navigates(press.key) && motion.value().key(press))
        },
    );
    let autoscroll = gestures::autoscroll(move |gesture: AutoscrollGesture| {
        let marking = (!gesture.ended).then_some(gesture.origin);
        origin.set(marking);
        motion.value().autoscroll(gesture);
    });
    edge_scroll(
        catcher,
        direction,
        move || Some(position.get_untracked()),
        move |reach: Option<f32>| {
            let pos = Pos2::ZERO + direction.get_untracked().axes(reach.unwrap_or(0.0), 0.0);
            motion.value().autoscroll(AutoscrollGesture {
                origin: Pos2::ZERO,
                pos,
                ended: reach.is_none(),
            });
        },
    );
    let pointer = gestures::first_of([
        autoscroll.pointer,
        gestures::chaining_scroll_drag(
            axis,
            move |movement: Vec2| {
                motion
                    .value()
                    .chains(direction.get_untracked().main(movement))
            },
            move |gesture: DragGesture| motion.value().drag(gesture),
        ),
    ]);
    let wheel = crosswise(
        direction,
        gestures::first_of([
            autoscroll.wheel,
            chaining(
                motion,
                gestures::scroll_wheel(axis, move |gesture: ScrollGesture| {
                    motion.value().wheel(gesture)
                }),
            ),
        ]),
    );
    let cursor = create_memo(move || {
        autoscroll.active.get().then(|| match direction.get() {
            Direction::Vertical => CursorIcon::ResizeVertical,
            Direction::Horizontal => CursorIcon::ResizeHorizontal,
        })
    });
    let watched = motion.with(|motion| motion.momentum.flag());
    let coasting = create_memo(move || match watched.get().coasting {
        true => vec![Claim::always(true)],
        false => Vec::new(),
    });
    let (grow, shrink) = content_grow_and_shrink(fit);
    view! {
        <Frame padding={move || -bleed.get()}>
            <Layers>
                <Interactive
                    focusable=true
                    tab_stop={overflows}
                    yields_tab_stop=true
                    on_focus_change={focused}
                    on_key={move |press: KeyPress| motion.value().key(press)}
                    cursor={cursor}
                    grabs_pointer={autoscroll.active}
                    @node_ref=&catcher
                    claims={coasting}
                    on_press={move |_: PointerPress| motion.value().stop_fling()}
                    on_hover_move={move |press: PointerPress| {
                        let at_edge = bar_width > 0.0
                            && barred.get_untracked().max_offset() > 0.0
                            && near_edge(direction.get_untracked(), press, bar_width);
                        near.set(at_edge);
                    }}
                    on_hover_change={move |inside: bool| {
                        if !inside && near.get_untracked() {
                            near.set(false);
                        }
                    }}
                    on_pointer={pointer}
                    on_wheel={wheel}
                >
                    <Frame outline={ring_when(focused, focus_ring)}>
                        <List direction={marker_axis} spacing=0.0>
                            <ListItem grow shrink>{node}</ListItem>
                            <KeepAlive>
                                <Overlay
                                    anchor
                                    placement=Placement::Around
                                    policy=LayerPolicy::Inert
                                    traps_focus=false
                                    open={marked}
                                >
                                    <Kept>{marker}</Kept>
                                </Overlay>
                            </KeepAlive>
                        </List>
                    </Frame>
                </Interactive>
                <List direction={across} justify=Justify::End spacing=0.0>{style.over(bar)}</List>
            </Layers>
        </Frame>
    }
}

/// Scrolls its children along `direction` with wheel, touch drag, arrow keys,
/// momentum and rubber banding, over a base `Offset`.
///
/// A gesture that starts with the scroll already at its edge that way is turned
/// down so an enclosing scroll takes it; the choice is made once per gesture, so
/// a fling into the band never moves the outer scroll mid-gesture. Unclaimed input
/// is taken even at the edge, so the outermost scroll still bands.
///
/// `fit`/`max_length` make it as long as its content up to a bound; a `Frame` with
/// a maximum around a scroll does not, since such a frame fills up to its maximum.
///
/// On the HTML core one without `snaps` is a real `overflow: auto` element
/// (`BrowserScroll`): the browser scrolls and chains it, with none of the momentum,
/// band or drawn bar, and `ScrollApi::animating` is always false. One with `snaps`
/// (a sheet) runs beui's own motion there as natively.
///
/// Keys scroll it only while it can scroll that way, so one at its end leaves
/// the key to the scroll around it: the arrows along its axis move 40 points
/// and Home/End to the ends, and a vertical one also takes Page Up/Down and
/// Space/Shift+Space by a page. Keys the focused control inside leaves unused
/// bubble here. It is a Tab stop only while it overflows and nothing inside it
/// is one.
#[component]
pub fn Scroll(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ScrollApi>,
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    /// Offsets along `direction` that a fling and a release settle on, the nearest to where
    /// they would have come to rest; none lets them rest anywhere. Past the last stop it moves
    /// freely, but a coast from short of it stops there. A wheel or a key steps between stops,
    /// scrolls freely past the last one and, coming back, stops at it before stepping on. A
    /// scroll with stops runs beui's own motion on the HTML core too, not the browser's.
    #[prop(default = Vec::new())]
    snaps: Prop<Vec<f32>>,
    /// Once at or past the last stop, a wheel or a key never takes it back below it, and short
    /// of it a wheel only steps forwards (a sheet: they scroll what it holds, never lower it).
    #[prop(default = false)]
    keep_past_last_stop: bool,
    /// Told the stop a release, a fling or a wheel's step chose, as it starts settling there.
    on_snap: Callback<f32>,
    /// The offset its bar starts at: the bar shows only the offsets past it, as the scroll of
    /// what lies past it, and nothing while the scroll is short of it (a sheet's bar, past its
    /// highest stop). Not for the browser's own scroll (the HTML core's, without stops), which
    /// draws its own bar.
    #[prop(default = 0.0)]
    bar_from: Prop<f32>,
    #[prop(default = false)] fit: bool,
    #[prop(default = false)] follow_end: Prop<bool>,
    #[prop(default = Sides::ZERO)] bleed: Prop<Sides>,
    #[prop(default = None)] focus_ring: Prop<Option<Outline>>,
    style: ScrollStyle,
    on_change: Callback<ScrollPosition>,
    children: Children<ListItem>,
) -> NodeId {
    let rows = NodeRef::new();
    // A scroll with stops (a sheet) runs beui's own motion on both cores, so its springs, its
    // flings into a stop and a drag that starts beside it behave the same; any other is the
    // browser's own scroll on the HTML core.
    let snapping = !matches!(&snaps, Prop::Static(stops) if stops.is_empty());
    if CORE == Core::Html && !snapping {
        let scroll = view! {
            <BrowserScroll
                api
                offset
                direction
                fit
                follow_end
                bleed
                focus_ring
                style
                on_change={on_change}
                rows
            >
                {children}
            </BrowserScroll>
        };
        reveal_row(reveal, rows);
        return scroll;
    }
    let fade = style.fade();
    let faded = create_signal(None::<ScrollPosition>);
    let scroll = view! {
        <Scrolling
            api
            direction
            snaps
            keep_past_last_stop
            on_snap={on_snap}
            bar_from
            fit
            bleed
            focus_ring
            style
            on_change={on_change}
        >|report: Callback<ScrollPosition>|
            <Frame fade_edges={move || fade_edges(direction.get(), faded.get(), fade)}>
                <Offset
                    offset
                    direction={direction}
                    fit
                    follow_end
                    on_change={move |position: ScrollPosition| {
                        faded.set(Some(position));
                        report.call(position);
                    }}
                >
                    <Frame padding={bleed}>
                        <List @node_ref=&rows direction={direction} spacing=0.0>{children}</List>
                    </Frame>
                </Offset>
            </Frame>
        </Scrolling>
    };
    reveal_row(reveal, rows);
    scroll
}

fn reveal_row(reveal: Prop<Option<usize>>, rows: NodeRef) {
    create_effect(move || {
        let Some(index) = reveal.get() else {
            return;
        };
        let rows = rows.get();
        if let Some(row) = rows.children().get(index).copied() {
            row.reveal();
        }
    });
}

#[component]
fn BrowserScroll(
    api: ComponentRef<ScrollApi>,
    offset: Prop<f32>,
    direction: Prop<Direction>,
    fit: bool,
    follow_end: Prop<bool>,
    bleed: Prop<Sides>,
    focus_ring: Prop<Option<Outline>>,
    style: ScrollStyle,
    on_change: Callback<ScrollPosition>,
    rows: NodeRef,
    children: Children<ListItem>,
) -> NodeId {
    let reported = create_signal(None::<ScrollPosition>);
    let fade = style.fade();
    let edges = create_memo(move || fade_edges(direction.get(), reported.get(), fade));
    let content = view! {
        <Frame padding={bleed}>
            <List @node_ref=&rows direction={direction} spacing=0.0>{children}</List>
        </Frame>
    };
    let scroller = store(RefCell::new(None::<HtmlNode>));
    let pinned = StoredCell::new(true);
    let placed = StoredCell::new(false);
    let horizontal = move || direction.get_untracked() == Direction::Horizontal;
    let names = move || match horizontal() {
        true => ("scrollLeft", "scrollWidth", "clientWidth"),
        false => ("scrollTop", "scrollHeight", "clientHeight"),
    };
    let scroll_to = move |value: f32| {
        scroller.with(|held| {
            if let Some(element) = held.borrow().as_ref() {
                element.set_number(names().0, f64::from(value));
            }
        });
    };
    let refresh = move || {
        let (offset_name, content_name, viewport_name) = names();
        let position = scroller.with(|held| {
            let held = held.borrow();
            let element = held.as_ref()?;
            Some(ScrollPosition {
                offset: element.number(offset_name)? as f32,
                content: element.number(content_name)? as f32,
                viewport: element.number(viewport_name)? as f32,
            })
        });
        let Some(position) = position else {
            return;
        };
        if reported.get_untracked() != Some(position) {
            reported.set(Some(position));
            on_change.call(position);
        }
    };
    let scrolled = move || {
        refresh();
        if let Some(position) = reported.get_untracked() {
            pinned.set(position.offset >= position.max_offset() - END_SLACK);
        }
    };
    let bare = style.bar.is_none();
    let focused = create_signal(false);
    let node = html_escape(move |escape| {
        escape.style("outline", "none".to_owned());
        escape.style("overflow-anchor", "auto".to_owned());
        if bare {
            escape.style("scrollbar-width", "none".to_owned());
        } else if let Some((thumb, track)) = style.browser {
            escape.style("scrollbar-width", "thin".to_owned());
            escape.style(
                "scrollbar-color",
                Prop::dynamic(move || {
                    Some(format!(
                        "{} {}",
                        Escape::color(thumb.get()),
                        Escape::color(track.get())
                    ))
                }),
            );
        }
        escape.style("display", "grid".to_owned());
        let track = "minmax(100%, max-content)";
        for (property, vertical, sideways) in [
            ("overflow-y", "auto", "hidden"),
            ("overflow-x", "hidden", "auto"),
            ("grid-template-columns", "minmax(0, 1fr)", "none"),
            ("grid-template-rows", "none", "minmax(0, 1fr)"),
            ("grid-auto-flow", "row", "column"),
            ("grid-auto-rows", track, "auto"),
            ("grid-auto-columns", "auto", track),
        ] {
            escape.style(
                property,
                Prop::dynamic(move || {
                    Some(
                        match direction.get() {
                            Direction::Vertical => vertical,
                            Direction::Horizontal => sideways,
                        }
                        .to_owned(),
                    )
                }),
            );
        }
        let content = escape.embed(content);
        let element = view! {
            <html::Div
                on_focus={move |_: HtmlEvent| focused.set(true)}
                on_blur={move |_: HtmlEvent| focused.set(false)}
                @on={("scroll", move |_: HtmlEvent| scrolled())}
            >
                <html::Div
                    @attr={("style", Prop::dynamic(move || Some(content_clip(direction.get()))))}
                >
                    {content}
                </html::Div>
            </html::Div>
        };
        scroller.with(|held| *held.borrow_mut() = Some(element.clone()));
        element
    });
    // An offset asked for (`ScrollApi::scroll_to`) the content is not yet long enough to reach:
    // the browser clamped it, so it is asked again as the content grows.
    let wanted = StoredCell::new(None::<f32>);
    let shown = measure(node);
    let laid = measure(content);
    create_effect(move || {
        shown.track();
        laid.track();
        if shown.get_untracked().is_none() {
            return;
        }
        if !placed.replace(true) {
            scroll_to(offset.get_untracked());
        }
        if let Some(target) = wanted.get() {
            scroll_to(target);
            refresh();
            if reported
                .get_untracked()
                .is_some_and(|position| (position.offset - target).abs() < 1.0)
            {
                wanted.set(None);
            }
        }
        if follow_end.get_untracked() && pinned.get() {
            scroll_to(f32::MAX);
        }
        refresh();
    });
    create_effect(move || {
        let offset = offset.get();
        if placed.get() {
            scroll_to(offset);
            refresh();
        }
    });
    let viewport = NodeRef::new();
    let speed = StoredCell::new(0.0_f32);
    let edge_offset = StoredCell::new(None::<f32>);
    let ticked = StoredCell::new(now());
    let edging = create_timer(move || {
        let speed = speed.get();
        let position = reported.get_untracked()?;
        if speed == 0.0 {
            return None;
        }
        let elapsed = now().saturating_duration_since(ticked.replace(now()));
        let from = edge_offset.get().unwrap_or(position.offset);
        let to = (from + speed * elapsed.as_secs_f32()).clamp(0.0, position.max_offset());
        edge_offset.set(Some(to));
        scroll_to(to);
        scrolled();
        drag_board().retrack();
        Some(Duration::ZERO)
    });
    edge_scroll(
        viewport,
        direction,
        move || reported.get_untracked(),
        move |reach: Option<f32>| {
            let now_speed = reach.map_or(0.0, autoscroll_speed);
            speed.set(now_speed);
            if now_speed == 0.0 {
                edging.stop();
                edge_offset.set(None);
            } else if !edging.running() {
                ticked.set(now());
                edge_offset.set(None);
                edging.start(Duration::ZERO);
            }
        },
    );
    let motion = store(Motion {
        node: None,
        reported: reported.read_only(),
        direction,
        snaps: Prop::Static(Vec::new()),
        on_snap: Callback::empty(),
        chosen: Cell::new(None),
        momentum: Watched::new(Momentum::new(style.rubber), Momentum::watched),
        animation: Cell::default(),
        autoscrolling: Cell::default(),
        autoscrolled: Cell::new(now()),
        keep_past_last_stop: false,
        wheeled: Cell::new(None),
        driven: Cell::new(0.0),
    });
    api.fill(ScrollApi(
        motion,
        Callback::new(move |()| {
            pinned.set(true);
            scroll_to(f32::MAX);
            refresh();
        }),
        Callback::new(move |offset: f32| {
            pinned.set(false);
            scroll_to(offset);
            refresh();
            let reached = reported
                .get_untracked()
                .is_some_and(|position| (position.offset - offset).abs() < 1.0);
            wanted.set((!reached).then_some(offset));
        }),
    ));
    component_accessibility(create_memo(move || {
        scroll_view(reported.get(), direction.get())
    }));
    let overflows = create_memo(move || {
        reported
            .get()
            .is_some_and(|position| position.max_offset() > 0.0)
    });
    let outset = create_memo(move || -bleed.get());
    let (grow, shrink) = content_grow_and_shrink(fit);
    view! {
        <Frame padding={outset}>
            <Frame outline={ring_when(focused, focus_ring)}>
                <List direction={direction} spacing=0.0>
                    <ListItem grow shrink>
                        <Frame fade_edges={edges}>
                            <Interactive
                                focusable=true
                                tab_stop={overflows}
                                yields_tab_stop=true
                                @node_ref=&viewport
                            >
                                {node}
                            </Interactive>
                        </Frame>
                    </ListItem>
                </List>
            </Frame>
        </Frame>
    }
}

/// A scroll's content fills it, or with `fit` takes its own length and shrinks to the
/// scroll's.
fn content_grow_and_shrink(fit: bool) -> (f32, f32) {
    match fit {
        true => (0.0, 1.0),
        false => (1.0, 0.0),
    }
}

pub(crate) fn fade_edges(
    direction: Direction,
    position: Option<ScrollPosition>,
    fade: f32,
) -> Sides {
    let Some(position) = position.filter(|_| fade > 0.0) else {
        return Sides::default();
    };
    let reach = fade.min(position.viewport / 2.0);
    let before = position.offset.clamp(0.0, reach);
    let after = (position.max_offset() - position.offset).clamp(0.0, reach);
    match direction {
        Direction::Horizontal => Sides {
            left: before,
            top: 0.0,
            right: after,
            bottom: 0.0,
        },
        Direction::Vertical => Sides {
            left: 0.0,
            top: before,
            right: 0.0,
            bottom: after,
        },
    }
}

fn scroll_view(position: Option<ScrollPosition>, direction: Direction) -> Node {
    let mut node = Node::new(Role::ScrollView);
    if let Some(position) = position {
        let (offset, max) = (position.clamped().into(), position.max_offset().into());
        match direction {
            Direction::Horizontal => {
                node.set_scroll_x(offset);
                node.set_scroll_x_min(0.0);
                node.set_scroll_x_max(max);
                node.add_action(Action::ScrollLeft);
                node.add_action(Action::ScrollRight);
            }
            Direction::Vertical => {
                node.set_scroll_y(offset);
                node.set_scroll_y_min(0.0);
                node.set_scroll_y_max(max);
                node.add_action(Action::ScrollUp);
                node.add_action(Action::ScrollDown);
            }
        }
        node.add_action(Action::Focus);
        node.add_action(Action::SetScrollOffset);
    }
    node
}

fn across(direction: &Prop<Direction>) -> Memo<Direction> {
    let direction = *direction;
    create_memo(move || match direction.get() {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    })
}

fn near_edge(direction: Direction, press: PointerPress, width: f32) -> bool {
    let across = match direction {
        Direction::Horizontal => Direction::Vertical,
        Direction::Vertical => Direction::Horizontal,
    };
    !press.touch && across.main(press.size) - across.main(press.local.to_vec2()) <= width
}

fn navigates(key: Key) -> bool {
    matches!(
        key,
        Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
    )
}

/// A scroll's handle. On the HTML core a scroll without stops is the browser's own, so it
/// carries the browser scroll's own ways to its end and to an offset.
#[derive(Clone, Copy)]
pub struct ScrollApi(Stored<Motion>, Callback<()>, Callback<f32>);

impl ScrollApi {
    /// Whether it is moving on its own (settling, coasting, springing back), read
    /// reactively.
    pub fn animating(&self) -> bool {
        self.0.with(|motion| motion.momentum.flag().get().moving)
    }

    /// How fast it is moving on its own, in offsets a second; 0 for the browser's own scroll.
    pub fn velocity(&self) -> f32 {
        self.0
            .try_value()
            .and_then(|motion| motion.momentum.try_borrow().ok().map(|momentum| momentum.velocity))
            .unwrap_or(0.0)
    }

    pub fn scroll_to_end(&self) {
        if !self.1.is_empty() {
            self.1.call(());
            return;
        }
        self.0.with(|motion| motion.to_end());
    }

    /// Moves by `by` at once, taking a drag's hold with it so the drag goes on from where it
    /// is (as if what it holds had moved by as much).
    pub fn shift(&self, by: f32) {
        if !self.2.is_empty() {
            let at = self
                .0
                .try_value()
                .and_then(|motion| motion.reported.get_untracked())
                .map_or(0.0, |position| position.offset);
            self.2.call(at + by);
            return;
        }
        if let Some(motion) = self.0.try_value() {
            motion.shift(by);
        }
    }

    /// Moves to `offset` at once, stopping whatever motion it had.
    pub fn scroll_to(&self, offset: f32) {
        if !self.2.is_empty() {
            self.2.call(offset);
            return;
        }
        if let Some(motion) = self.0.try_value() {
            motion.scroll_to(offset);
        }
    }

    /// Moves to `offset` as it settles on a snap stop, starting at `velocity` (offsets a
    /// second, what a gesture let it go at): on the overscroll spring, or at once without
    /// motion; the browser's own scroll (on the HTML core, without stops) goes at once.
    pub fn settle_to(&self, offset: f32, velocity: f32) {
        if !self.2.is_empty() {
            self.2.call(offset);
            return;
        }
        if let Some(motion) = self.0.try_value() {
            motion.settle_to(offset, velocity);
        }
    }

    /// Drags it as a drag on it would, for a drag that starts beside it (the space above a
    /// sheet).
    pub fn drag(&self, gesture: DragGesture) {
        if let Some(motion) = self.0.try_value() {
            motion.drag(gesture);
        }
    }
}

fn ring_when(focused: impl IntoProp<bool>, ring: Prop<Option<Outline>>) -> Memo<Option<Outline>> {
    let shown = focus_ring(focused);
    create_memo(move || ring.get().filter(|_| shown.get()))
}
