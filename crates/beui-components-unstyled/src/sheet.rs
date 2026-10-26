use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::gestures;
use beui_macros::{component, view};

use super::back_slide::BACK_DRAG_SHARE;
use super::fling::Fling;
use super::modal::{Modal, ModalHandle, ModalMotion, ModalPlacement};
use super::rubber_band::{SCROLL_SPRING, WINDOW_SPRING, bounce, rubber_band, spring_back, unband};
use beui_view::components::overlay::use_overlay_presence;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;
use beui_view::types::Direction;
use beui_view::types::ItemSize;
use beui_view::types::ScrollPosition;
use beui_view::types::{BackGesture, CursorIcon, DragGesture, PointerPress, ScrollGesture};
use beui_view::types::{Pos2, Rect, vec2};

const CLOSE_SHARE: f32 = 0.4;
const CLOSE_DISTANCE: f32 = 96.0;
const PROJECTION: f32 = 0.2;
const RELEASED_STRETCH: f32 = 0.35;
const EPSILON: f32 = 0.5;
const ENTER_SECONDS: f32 = 0.3;
const LEAVE_SECONDS: f32 = 0.22;
const RETURN_SECONDS: f32 = 0.25;
const HIDDEN_SHIFT: f32 = 100_000.0;
const UNPLACED_FRAMES: u8 = 3;
const HANDOFF_SECONDS: f32 = 0.1;

pub struct SheetGripHandle {
    pub dragging: ReadSignal<bool>,
    pub on_drag: Callback<DragGesture>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Rest {
    Share(f32),
    Fitted,
    Free(f32),
}

struct Shape {
    extent: f32,
    top: f32,
    stops: Vec<(f32, Rest)>,
    viewport: f32,
    max_offset: f32,
}

enum Phase {
    Still,
    Snap {
        target: Rest,
        velocity: f32,
        closes: bool,
    },
    Fling(Fling),
    SheetBounce {
        velocity: f32,
    },
    Bounce {
        velocity: f32,
    },
}

#[derive(Clone, Copy)]
enum Slide {
    Shown,
    Entering {
        elapsed: f32,
        waited: u8,
    },
    Leaving {
        elapsed: f32,
        from: f32,
        velocity: f32,
        acceleration: f32,
    },
    Returning {
        elapsed: f32,
        from: f32,
    },
    Morphing {
        elapsed: f32,
        from: f32,
        waited: u8,
    },
    Gone,
}

#[derive(Clone, Copy)]
enum Handoff {
    Leaving(Stored<Motion>),
    Entering(Stored<Motion>),
}

#[derive(Clone, Default)]
struct Handoffs(Rc<Cell<Option<Handoff>>>);

fn handoffs() -> Handoffs {
    tree_local(Handoffs::default)
}

fn claim(
    me: Stored<Motion>,
    wanted: impl Fn(&Handoff) -> Option<Stored<Motion>>,
) -> Option<Rc<Motion>> {
    let offered = handoffs().0.take()?;
    let other = wanted(&offered).filter(|other| *other != me)?;
    other.try_value()
}

fn offer(handoff: Handoff) {
    handoffs().0.set(Some(handoff));
}

struct Held {
    travel: Option<f32>,
    handle: Option<f32>,
    dismiss_drag: bool,
    phase: Phase,
    slide: Slide,
    flung: f32,
    backing: bool,
}

struct Motion {
    extent: Memo<f32>,
    shares: Rc<[f32]>,
    resting: Rest,
    fit: bool,
    rest_at: Signal<Rest>,
    shifted: Cell<f32>,
    slot: NodeRef,
    chrome: Cell<f32>,
    dragging: Signal<bool>,
    content: NodeRef,
    held: RefCell<Held>,
    animation: Cell<Option<Animation>>,
    open: Prop<bool>,
    shift: Signal<f32>,
    gone: Signal<bool>,
    on_presence: Callback<f32>,
    on_gone: ClickCallback,
    on_close: ClickCallback,
}

fn banding() -> bool {
    motion().animates()
}

impl Motion {
    fn node(&self) -> Option<NodeId> {
        self.content.try_get()?.first_offset_within()
    }

    fn laid_out(&self) -> (Option<Rect>, Option<Rect>, ScrollPosition) {
        let node = self.node();
        let slot = self.slot.try_get();
        let sheet = slot.and_then(|slot| measure(slot).get_untracked());
        let body = node.and_then(|node| measure(node).get_untracked());
        let position = node
            .and_then(|node| node.scroll_position())
            .unwrap_or_default();
        (sheet, body, position)
    }

    fn height(&self) -> f32 {
        match self.rest_at.get_untracked() {
            Rest::Free(height) => height,
            Rest::Share(_) | Rest::Fitted => self.laid_out().0.map_or(0.0, |sheet| sheet.height()),
        }
    }

    fn shape(&self) -> Shape {
        let extent = self.extent.get_untracked();
        let (sheet, body, position) = self.laid_out();
        if let (Some(sheet), Some(body)) = (sheet, body) {
            self.chrome
                .set(body.top() - sheet.top() - self.shifted.get());
        }
        let chrome = self.chrome.get();
        let highest = self.shares.last().copied().unwrap_or(1.0);
        let top = (chrome + position.content).min(highest * extent);
        let stops = match self.fit {
            true => vec![(top, Rest::Fitted)],
            false => self
                .shares
                .iter()
                .map(|share| ((share * extent).min(top), Rest::Share(*share)))
                .collect(),
        };
        let viewport = (top - chrome).max(0.0);
        Shape {
            extent,
            top,
            stops,
            viewport,
            max_offset: (position.content - viewport).max(0.0),
        }
    }

    fn top_rest(&self) -> Rest {
        match self.fit {
            true => Rest::Fitted,
            false => Rest::Share(self.shares.last().copied().unwrap_or(1.0)),
        }
    }

    fn rest_height(&self, rest: Rest, shape: &Shape) -> f32 {
        match rest {
            Rest::Share(share) => (share * shape.extent).min(shape.top),
            Rest::Fitted => shape.top,
            Rest::Free(height) => height,
        }
    }

    fn content(&self) -> (f32, f32) {
        let Some(node) = self.node() else {
            return (0.0, 0.0);
        };
        let Some(raw) = node.offset() else {
            return (0.0, 0.0);
        };
        let offset = raw.clamp(0.0, self.shape().max_offset);
        (offset, raw - offset)
    }

    fn place_content(&self, offset: f32, overscroll: f32) {
        let Some(node) = self.node() else {
            return;
        };
        node.drive_offset(offset + overscroll);
    }

    fn settle_content(&self, position: ScrollPosition) -> bool {
        let held = self.held.borrow();
        if position.overscroll() == 0.0
            || held.travel.is_some()
            || !matches!(held.phase, Phase::Still)
        {
            return false;
        }
        drop(held);
        let Some(node) = self.node() else {
            return false;
        };
        node.drive_offset(position.clamped());
        true
    }

    fn place_height(&self, rest: Rest) {
        self.rest_at.set(rest);
    }

    fn travel(&self, shape: &Shape) -> f32 {
        let height = self.height();
        let (offset, overscroll) = self.content();
        if height > shape.top + EPSILON {
            return shape.top + unband(height - shape.top, shape.extent);
        }
        if height < shape.top - EPSILON {
            return height;
        }
        shape.top + offset + unband(overscroll, shape.viewport)
    }

    fn apply(&self, travel: f32, shape: &Shape) {
        let upper = shape.top + shape.max_offset;
        let banding = banding();
        let (height, offset, overscroll) = if travel <= shape.top {
            (travel.max(0.0), 0.0, 0.0)
        } else if travel <= upper || !banding {
            (shape.top, (travel - shape.top).min(shape.max_offset), 0.0)
        } else if shape.max_offset > 0.0 {
            (
                shape.top,
                shape.max_offset,
                rubber_band(travel - upper, shape.viewport),
            )
        } else {
            (
                shape.top + rubber_band(travel - upper, shape.extent),
                0.0,
                0.0,
            )
        };
        self.place_height(Rest::Free(height));
        self.place_content(offset, overscroll);
    }

    fn grab(&self) {
        self.settle_morph();
        let shape = self.shape();
        let displaced = {
            let held = self.held.borrow();
            matches!(held.phase, Phase::SheetBounce { .. })
                || matches!(held.slide, Slide::Returning { .. })
        };
        if displaced {
            self.held.borrow_mut().slide = Slide::Shown;
            if self.content().0 > 0.0 {
                self.return_back();
            } else {
                let height = self.height();
                self.place_height(Rest::Free((height - self.shifted.get()).max(0.0)));
                self.place_shift(0.0, height);
            }
        }
        let travel = self.travel(&shape);
        let mut held = self.held.borrow_mut();
        held.travel = Some(travel);
        held.phase = Phase::Still;
        self.dragging.set(true);
    }

    fn pull(&self, by: f32) {
        let shape = self.shape();
        let travel = {
            let mut held = self.held.borrow_mut();
            let Some(travel) = held.travel.as_mut() else {
                return;
            };
            *travel = (*travel - by).max(0.0);
            *travel
        };
        self.apply(travel, &shape);
    }

    fn release(&self, velocity: f32) {
        if self.held.borrow_mut().travel.take().is_none() {
            return;
        }
        self.dragging.set(false);
        let shape = self.shape();
        let height = self.height();
        let (offset, overscroll) = self.content();
        let phase = if overscroll != 0.0 {
            Phase::Bounce {
                velocity: velocity * RELEASED_STRETCH,
            }
        } else if height > shape.top + EPSILON {
            Phase::Snap {
                target: self.top_rest(),
                velocity: velocity * RELEASED_STRETCH,
                closes: false,
            }
        } else if offset > 0.0
            || (height >= shape.top - EPSILON && velocity > 0.0 && shape.max_offset > 0.0)
        {
            Fling::new(velocity)
                .filter(|_| banding())
                .map_or(Phase::Still, Phase::Fling)
        } else {
            self.snap_phase(height, velocity, &shape)
        };
        if let Phase::Snap {
            velocity,
            closes: true,
            ..
        } = phase
        {
            self.close(velocity);
            return;
        }
        self.start(phase);
    }

    fn close(&self, velocity: f32) {
        self.held.borrow_mut().flung = velocity;
        self.on_close.call();
        if self.open.get_untracked() {
            self.held.borrow_mut().flung = 0.0;
            self.start(Phase::Snap {
                target: if self.content().0 > 0.0 {
                    self.top_rest()
                } else {
                    self.resting
                },
                velocity: 0.0,
                closes: false,
            });
            self.return_back();
        }
    }

    fn back(&self, gesture: BackGesture) {
        let height = self.sheet_height().filter(|height| *height > 0.0);
        let backing = self.held.borrow().backing;
        match gesture {
            BackGesture::Started { .. } => {
                self.settle_morph();
                let mut held = self.held.borrow_mut();
                if height.is_some()
                    && held.travel.is_none()
                    && matches!(held.slide, Slide::Shown | Slide::Returning { .. })
                {
                    if matches!(held.phase, Phase::SheetBounce { .. }) {
                        held.phase = Phase::Still;
                    }
                    held.backing = true;
                    held.slide = Slide::Shown;
                }
            }
            BackGesture::Progressed(progress) => {
                if let Some(height) = height.filter(|_| backing) {
                    self.place_shift(progress * BACK_DRAG_SHARE * height, height);
                }
            }
            BackGesture::Cancelled => {
                self.held.borrow_mut().backing = false;
                if backing {
                    self.return_back();
                }
            }
            BackGesture::Invoked => {
                self.held.borrow_mut().backing = false;
                let velocity = match height.filter(|_| backing) {
                    Some(height) => (height - self.shifted.get()).max(0.0) / LEAVE_SECONDS,
                    None => 0.0,
                };
                self.close(velocity);
            }
        }
    }

    fn return_back(&self) {
        let from = self.shifted.get();
        let mut held = self.held.borrow_mut();
        if from <= 0.0 || !matches!(held.slide, Slide::Shown) {
            return;
        }
        held.slide = Slide::Returning { elapsed: 0.0, from };
        drop(held);
        self.kick();
    }

    fn place_shift(&self, shift: f32, height: f32) {
        self.shifted.set(shift);
        self.shift.set(shift);
        self.on_presence.call(1.0 - shift / height);
    }

    fn sheet_height(&self) -> Option<f32> {
        self.laid_out().0.map(|sheet| sheet.height())
    }

    fn enter(&self, me: Stored<Motion>) {
        self.gone.set(false);
        self.reset();
        let leaving = claim(me, |offered| match offered {
            Handoff::Leaving(other) => Some(*other),
            Handoff::Entering(_) => None,
        });
        if let Some(from) = leaving.and_then(|other| other.hand_off()) {
            self.morph(from);
            return;
        }
        offer(Handoff::Entering(me));
        {
            let mut held = self.held.borrow_mut();
            held.slide = Slide::Entering {
                elapsed: 0.0,
                waited: 0,
            };
        }
        self.shifted.set(HIDDEN_SHIFT);
        self.shift.set(HIDDEN_SHIFT);
        self.on_presence.call(0.0);
        self.kick();
    }

    fn leave(&self, me: Stored<Motion>) {
        let entering = claim(me, |offered| match offered {
            Handoff::Entering(other) => Some(*other),
            Handoff::Leaving(_) => None,
        });
        if let Some(other) = entering.filter(|other| other.just_entered())
            && let Some(from) = self.visible_height()
        {
            self.vanish();
            other.morph(from);
            return;
        }
        let Some(height) = self.sheet_height().filter(|height| *height > 0.0) else {
            self.held.borrow_mut().slide = Slide::Gone;
            self.on_presence.call(0.0);
            self.gone.set(true);
            self.on_gone.call();
            return;
        };
        {
            let mut held = self.held.borrow_mut();
            held.travel = None;
            held.handle = None;
            held.dismiss_drag = false;
            held.phase = Phase::Still;
            let from = self.shifted.get().min(height);
            let velocity = std::mem::take(&mut held.flung);
            let distance = height - from;
            let acceleration = (2.0 * (distance - velocity * LEAVE_SECONDS)
                / (LEAVE_SECONDS * LEAVE_SECONDS))
                .max(0.0);
            held.slide = Slide::Leaving {
                elapsed: 0.0,
                from,
                velocity,
                acceleration,
            };
        }
        self.dragging.set(false);
        offer(Handoff::Leaving(me));
        self.kick();
    }

    fn just_entered(&self) -> bool {
        matches!(self.held.borrow().slide, Slide::Entering { elapsed, .. } if elapsed < HANDOFF_SECONDS)
    }

    fn visible_height(&self) -> Option<f32> {
        self.sheet_height()
            .map(|height| (height - self.shifted.get()).max(0.0))
            .filter(|height| *height > 0.0)
    }

    fn hand_off(&self) -> Option<f32> {
        let leaving = matches!(
            self.held.borrow().slide,
            Slide::Leaving { elapsed, .. } if elapsed < HANDOFF_SECONDS
        );
        let from = self.visible_height().filter(|_| leaving)?;
        self.vanish();
        Some(from)
    }

    fn vanish(&self) {
        {
            let mut held = self.held.borrow_mut();
            held.travel = None;
            held.handle = None;
            held.dismiss_drag = false;
            held.phase = Phase::Still;
            held.flung = 0.0;
            held.slide = Slide::Gone;
        }
        self.dragging.set(false);
        self.on_presence.call(0.0);
        self.gone.set(true);
        self.on_gone.call();
    }

    fn morph(&self, from: f32) {
        self.held.borrow_mut().slide = Slide::Morphing {
            elapsed: 0.0,
            from,
            waited: 0,
        };
        self.shifted.set(0.0);
        self.shift.set(0.0);
        self.on_presence.call(1.0);
        self.place_height(Rest::Free(from));
        self.kick();
    }

    fn step_morph(&self, before: f32, from: f32, waited: u8, elapsed: f32) -> bool {
        let (sheet, body, _) = self.laid_out();
        if sheet.is_none() || body.is_none() {
            if waited < UNPLACED_FRAMES {
                self.held.borrow_mut().slide = Slide::Morphing {
                    elapsed: before,
                    from,
                    waited: waited + 1,
                };
                return true;
            }
            self.held.borrow_mut().slide = Slide::Shown;
            self.place_height(self.resting);
            return false;
        }
        let shape = self.shape();
        let goal = self.rest_height(self.resting, &shape);
        let elapsed = before + elapsed;
        let progress = tween(elapsed, ENTER_SECONDS, Easing::Linear);
        if progress >= 1.0 {
            self.held.borrow_mut().slide = Slide::Shown;
            self.place_height(self.resting);
            return false;
        }
        let eased = Easing::OutCubic.sample(progress);
        self.held.borrow_mut().slide = Slide::Morphing {
            elapsed,
            from,
            waited,
        };
        self.place_height(Rest::Free(from + (goal - from) * eased));
        true
    }

    fn settle_morph(&self) {
        let morphing = matches!(self.held.borrow().slide, Slide::Morphing { .. });
        if morphing {
            self.held.borrow_mut().slide = Slide::Shown;
        }
    }

    fn kick(&self) {
        let Some(animation) = self.animation.get() else {
            return;
        };
        animation.start();
    }

    fn step_slide(&self, elapsed: f32) -> bool {
        let slide = self.held.borrow().slide;
        if let Slide::Morphing {
            elapsed: before,
            from,
            waited,
        } = slide
        {
            return self.step_morph(before, from, waited, elapsed);
        }
        let height = match (slide, self.sheet_height().filter(|height| *height > 0.0)) {
            (Slide::Shown | Slide::Gone | Slide::Morphing { .. }, _) => return false,
            (_, Some(height)) => height,
            (Slide::Entering { elapsed, waited }, None) if waited < UNPLACED_FRAMES => {
                self.held.borrow_mut().slide = Slide::Entering {
                    elapsed,
                    waited: waited + 1,
                };
                return true;
            }
            (Slide::Entering { .. }, None) => {
                self.held.borrow_mut().slide = Slide::Shown;
                self.shifted.set(0.0);
                self.shift.set(0.0);
                self.on_presence.call(1.0);
                return false;
            }
            (Slide::Returning { .. }, None) => {
                self.held.borrow_mut().slide = Slide::Shown;
                self.shifted.set(0.0);
                self.shift.set(0.0);
                self.on_presence.call(1.0);
                return false;
            }
            (Slide::Leaving { .. }, None) => {
                self.held.borrow_mut().slide = Slide::Gone;
                self.on_presence.call(0.0);
                self.gone.set(true);
                self.on_gone.call();
                return false;
            }
        };
        let (shift, next) = match slide {
            Slide::Entering {
                elapsed: before,
                waited,
            } => {
                let elapsed = before + elapsed;
                let progress = tween(elapsed, ENTER_SECONDS, Easing::Linear);
                let eased = Easing::OutCubic.sample(progress);
                let next = match progress >= 1.0 {
                    true => Slide::Shown,
                    false => Slide::Entering { elapsed, waited },
                };
                ((1.0 - eased) * height, next)
            }
            Slide::Leaving {
                elapsed: before,
                from,
                velocity,
                acceleration,
            } => {
                let elapsed = before + elapsed;
                let shift = from + velocity * elapsed + 0.5 * acceleration * elapsed * elapsed;
                match shift >= height || (velocity <= 0.0 && acceleration <= 0.0) {
                    true => (height, Slide::Gone),
                    false => (
                        shift,
                        Slide::Leaving {
                            elapsed,
                            from,
                            velocity,
                            acceleration,
                        },
                    ),
                }
            }
            Slide::Returning {
                elapsed: before,
                from,
            } => {
                let elapsed = before + elapsed;
                let progress = tween(elapsed, RETURN_SECONDS, Easing::Linear);
                let eased = Easing::OutCubic.sample(progress);
                let next = match progress >= 1.0 {
                    true => Slide::Shown,
                    false => Slide::Returning { elapsed, from },
                };
                (from * (1.0 - eased), next)
            }
            Slide::Shown | Slide::Gone | Slide::Morphing { .. } => return false,
        };
        {
            let mut held = self.held.borrow_mut();
            held.slide = next;
        }
        self.place_shift(shift, height);
        match next {
            Slide::Gone => {
                self.gone.set(true);
                self.on_gone.call();
                false
            }
            Slide::Shown | Slide::Morphing { .. } => false,
            Slide::Entering { .. } | Slide::Leaving { .. } | Slide::Returning { .. } => true,
        }
    }

    fn snap_phase(&self, height: f32, velocity: f32, shape: &Shape) -> Phase {
        let projected = height + velocity * PROJECTION;
        let lowest = shape.stops.first().map_or(shape.top, |(stop, _)| *stop);
        let close_below = lowest - (lowest * CLOSE_SHARE).min(CLOSE_DISTANCE);
        if projected < close_below {
            return Phase::Snap {
                target: Rest::Free(height),
                velocity: (-velocity).max(0.0),
                closes: true,
            };
        }
        let target = shape
            .stops
            .iter()
            .min_by(|(a, _), (b, _)| (a - projected).abs().total_cmp(&(b - projected).abs()))
            .map_or(self.top_rest(), |(_, rest)| *rest);
        Phase::Snap {
            target,
            velocity,
            closes: false,
        }
    }

    fn start(&self, phase: Phase) {
        let mut held = self.held.borrow_mut();
        held.phase = phase;
        if matches!(held.phase, Phase::Still) {
            return;
        }
        let Some(animation) = self.animation.get() else {
            return;
        };
        drop(held);
        animation.start();
    }

    fn flinging(&self) -> bool {
        matches!(
            self.held.borrow().phase,
            Phase::Fling(_) | Phase::SheetBounce { .. }
        )
    }

    fn stop_fling(&self) {
        let mut held = self.held.borrow_mut();
        let bouncing = matches!(held.phase, Phase::SheetBounce { .. });
        if matches!(held.phase, Phase::Fling(_) | Phase::SheetBounce { .. }) {
            held.phase = Phase::Still;
        }
        drop(held);
        if bouncing {
            self.return_back();
        }
    }

    fn drag(&self, gesture: DragGesture) {
        if self.held.borrow().travel.is_none() {
            self.grab();
        }
        if gesture.delta.y != 0.0 {
            self.pull(gesture.delta.y);
        }
        if gesture.ended {
            self.release(-gesture.velocity.y);
        } else if gesture.cancelled {
            self.release(0.0);
        }
    }

    fn wheel(&self, gesture: ScrollGesture) {
        let (wheel, fling) = (gesture.delta.y, gesture.fling.y);
        if gesture.held && wheel == 0.0 && fling == 0.0 {
            self.stop_fling();
            return;
        }
        if self.held.borrow().travel.is_some() || (wheel == 0.0 && fling == 0.0) {
            return;
        }
        let shape = self.shape();
        let height = self.height();
        if height < shape.top - EPSILON {
            if wheel < 0.0 || fling < 0.0 {
                self.start(Phase::Snap {
                    target: self.top_rest(),
                    velocity: 0.0,
                    closes: false,
                });
            }
            return;
        }
        self.return_back();
        self.held.borrow_mut().phase = Phase::Still;
        let (offset, _) = self.content();
        self.place_content((offset - wheel).clamp(0.0, shape.max_offset), 0.0);
        if fling != 0.0 && banding() {
            self.start(Fling::new(-fling).map_or(Phase::Still, Phase::Fling));
        }
    }

    fn grab_handle(&self) {
        self.settle_morph();
        if self.content().0 > 0.0 {
            self.place_height(self.top_rest());
            let mut held = self.held.borrow_mut();
            held.phase = Phase::Still;
            held.slide = Slide::Shown;
            held.dismiss_drag = true;
            held.travel = Some(self.shifted.get());
            self.dragging.set(true);
        } else {
            self.grab();
            let mut held = self.held.borrow_mut();
            held.dismiss_drag = false;
            held.travel = Some(self.height());
        }
    }

    fn handle_press(&self, press: PointerPress) {
        self.grab_handle();
        self.held.borrow_mut().handle = Some(press.pos.y);
    }

    fn handle_drag(&self, press: PointerPress) {
        let Some(last) = self.held.borrow_mut().handle.replace(press.pos.y) else {
            return;
        };
        self.pull_handle(press.pos.y - last);
    }

    fn pull_handle(&self, delta: f32) {
        if self.held.borrow().dismiss_drag {
            let height = self.height();
            let shift = {
                let mut held = self.held.borrow_mut();
                let Some(travel) = held.travel.as_mut() else {
                    return;
                };
                *travel = (*travel + delta).clamp(0.0, height);
                *travel
            };
            self.place_shift(shift, height);
            return;
        }
        let shape = self.shape();
        let height = {
            let mut held = self.held.borrow_mut();
            let Some(height) = held.travel.as_mut() else {
                return;
            };
            *height = (*height - delta).max(0.0);
            *height
        };
        self.place_height(Rest::Free(if height > shape.top && banding() {
            shape.top + rubber_band(height - shape.top, shape.extent)
        } else {
            height.min(shape.top)
        }));
    }

    fn release_handle(&self, velocity: f32) {
        if self.held.borrow_mut().travel.take().is_none() {
            return;
        }
        self.dragging.set(false);
        if std::mem::take(&mut self.held.borrow_mut().dismiss_drag) {
            if self.shifted.get() - velocity * PROJECTION > self.height() * CLOSE_SHARE {
                self.close((-velocity).max(0.0));
            } else {
                self.start(Phase::SheetBounce {
                    velocity: -velocity,
                });
            }
            return;
        }
        let phase = self.snap_phase(self.height(), velocity, &self.shape());
        if let Phase::Snap {
            velocity,
            closes: true,
            ..
        } = phase
        {
            self.close(velocity);
        } else {
            self.start(phase);
        }
    }

    fn outside_drag(&self, gesture: DragGesture) {
        if self.held.borrow().travel.is_none() {
            self.grab_handle();
        }
        self.pull_handle(gesture.delta.y);
        if gesture.ended {
            self.release_handle(-gesture.velocity.y);
        } else if gesture.cancelled {
            self.cancel_handle();
        }
    }

    fn cancel_handle(&self) {
        if self.held.borrow().dismiss_drag {
            let mut held = self.held.borrow_mut();
            held.travel = None;
            held.dismiss_drag = false;
            drop(held);
            self.dragging.set(false);
            self.start(Phase::SheetBounce { velocity: 0.0 });
        } else {
            self.release_handle(0.0);
        }
    }

    fn handle_cancel(&self) {
        if self.held.borrow_mut().handle.take().is_some() {
            self.cancel_handle();
        }
    }

    fn handle_active(&self, active: bool) {
        if active || self.held.borrow_mut().handle.take().is_none() {
            return;
        }
        self.release_handle(0.0);
    }

    fn reset(&self) {
        let mut held = self.held.borrow_mut();
        held.travel = None;
        held.handle = None;
        held.dismiss_drag = false;
        held.phase = Phase::Still;
        drop(held);
        self.dragging.set(false);
        self.place_height(self.resting);
        self.place_content(0.0, 0.0);
    }

    fn finish(&self) {
        let (slide, phase) = {
            let mut held = self.held.borrow_mut();
            let slide = held.slide;
            held.slide = match slide {
                Slide::Leaving { .. } => Slide::Gone,
                Slide::Gone => Slide::Gone,
                _ => Slide::Shown,
            };
            (slide, std::mem::replace(&mut held.phase, Phase::Still))
        };
        match phase {
            Phase::Snap { target, .. } => self.place_height(target),
            Phase::Bounce { .. } | Phase::Fling(_) => {
                let (offset, _) = self.content();
                self.place_content(offset, 0.0);
            }
            Phase::SheetBounce { .. } => self.place_shift(0.0, self.height()),
            Phase::Still => {}
        }
        match slide {
            Slide::Leaving { .. } => {
                self.on_presence.call(0.0);
                self.gone.set(true);
                self.on_gone.call();
            }
            Slide::Entering { .. } | Slide::Returning { .. } => {
                self.shifted.set(0.0);
                self.shift.set(0.0);
                self.on_presence.call(1.0);
            }
            Slide::Morphing { .. } => self.place_height(self.resting),
            _ => {}
        }
    }

    fn step(&self, frame: AnimationFrame) -> Option<Duration> {
        let elapsed = frame.delta.as_secs_f32();
        let sliding = self.step_slide(elapsed);
        let moving = self.step_phase(elapsed);
        (sliding || moving).then_some(Duration::ZERO)
    }

    fn step_phase(&self, elapsed: f32) -> bool {
        let mut held = self.held.borrow_mut();
        if held.travel.is_some() {
            return false;
        }
        let shape = self.shape();
        let (offset, overscroll) = self.content();
        match &mut held.phase {
            Phase::Still => false,
            Phase::Snap {
                target,
                velocity,
                closes,
            } => {
                let (target, closes) = (*target, *closes);
                let goal = self.rest_height(target, &shape);
                let mut distance = self.height() - goal;
                spring_back(&mut distance, velocity, elapsed, WINDOW_SPRING);
                if closes && goal + distance <= 0.0 {
                    distance = 0.0;
                    *velocity = 0.0;
                }
                if distance != 0.0 || *velocity != 0.0 {
                    drop(held);
                    self.place_height(Rest::Free(goal + distance));
                    return true;
                }
                held.phase = Phase::Still;
                drop(held);
                if !closes {
                    self.place_height(target);
                }
                false
            }
            Phase::Fling(fling) => {
                let raw = offset + fling.advance(elapsed);
                let velocity = fling.velocity();
                let done = fling.done();
                let placed = raw.clamp(0.0, shape.max_offset);
                let mut overscroll = 0.0;
                if raw > placed && banding() {
                    held.phase = Phase::Bounce { velocity };
                    overscroll = raw - placed;
                } else if raw < placed && banding() {
                    held.phase = Phase::SheetBounce {
                        velocity: -velocity,
                    };
                    let shift = placed - raw;
                    drop(held);
                    self.place_content(0.0, 0.0);
                    self.place_shift(shift, self.height());
                    return true;
                } else if raw != placed || done {
                    held.phase = Phase::Still;
                }
                let moving = !matches!(held.phase, Phase::Still);
                drop(held);
                self.place_content(placed, overscroll);
                moving
            }
            Phase::SheetBounce { velocity } => {
                let mut shift = self.shifted.get();
                spring_back(&mut shift, velocity, elapsed, SCROLL_SPRING);
                let settled = shift == 0.0 && *velocity == 0.0;
                if settled {
                    held.phase = Phase::Still;
                }
                drop(held);
                self.place_shift(shift, self.height());
                !settled
            }
            Phase::Bounce { velocity } => {
                let mut stretch = unband(overscroll, shape.viewport);
                let back = bounce(&mut stretch, velocity, elapsed);
                if stretch == 0.0 {
                    held.phase = Fling::new(back).map_or(Phase::Still, Phase::Fling);
                }
                let moving = !matches!(held.phase, Phase::Still);
                drop(held);
                self.place_content(offset, rubber_band(stretch, shape.viewport));
                moving
            }
        }
    }
}

#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    #[prop(default = 0.0)] fade: f32,
    grip: Render<SheetGripHandle>,
    panel: Render<Child>,
    on_presence: Callback<f32>,
    on_gone: ClickCallback,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let mut stops = stops;
    stops.sort_by(f32::total_cmp);
    let shares: Rc<[f32]> = Rc::from(stops);
    let resting = match fit {
        true => Rest::Fitted,
        false => Rest::Share(rest),
    };
    let extent = create_memo(move || extent.get().max(1.0));
    let rest_at = create_signal(resting);
    let dragging = create_signal(false);
    let shift = create_signal(HIDDEN_SHIFT);
    let gone = create_signal(true);
    let present = create_memo(move || !gone.get());
    let sheet_sizing = create_memo(move || match rest_at.get() {
        Rest::Share(share) => ItemSize::Intrinsic.max(share * extent.get()).shrink(1.0),
        Rest::Fitted => ItemSize::Intrinsic.shrink(1.0),
        Rest::Free(height) => ItemSize::Fixed(height).shrink(1.0),
    });
    let content = NodeRef::new();
    let slot = NodeRef::new();
    let motion = store(Motion {
        extent,
        shares,
        resting,
        fit,
        rest_at,
        shifted: Cell::new(HIDDEN_SHIFT),
        slot,
        chrome: Cell::default(),
        dragging,
        content,
        held: RefCell::new(Held {
            travel: None,
            handle: None,
            dismiss_drag: false,
            phase: Phase::Still,
            slide: Slide::Gone,
            flung: 0.0,
            backing: false,
        }),
        animation: Cell::default(),
        open,
        shift,
        gone,
        on_presence,
        on_gone,
        on_close,
    });
    let position = create_signal(None::<ScrollPosition>);
    let edges =
        create_memo(move || super::scroll::fade_edges(Direction::Vertical, position.get(), fade));
    let animation = create_animation(
        move |frame| motion.value().step(frame).is_some(),
        move || motion.value().finish(),
    );
    motion.with(|motion| motion.animation.set(Some(animation)));
    let preferences = use_preferences();
    let animates = create_memo(move || preferences.get().motion.animates());
    create_effect(move || {
        if !animates.get() {
            let motion = motion.value();
            let (offset, _) = motion.content();
            motion.place_content(offset, 0.0);
        }
    });
    let was_open = Cell::new(false);
    create_effect(move || {
        let open = open.get();
        if open == was_open.replace(open) {
            return;
        }
        untrack(|| match open {
            true => motion.value().enter(motion),
            false => motion.value().leave(motion),
        });
    });
    let shifted = create_memo(move || vec2(0.0, shift.get()));
    let axis = create_memo(|| Some(Direction::Vertical));
    let catcher = NodeRef::new();
    let grip = grip.call(SheetGripHandle {
        dragging: dragging.read_only(),
        on_drag: Callback::new(move |gesture| motion.value().outside_drag(gesture)),
    });
    let sheet = view! {
        <List spacing=0.0>
            <ShowKeepAlive condition={present}>
                <Frame @sizing={sheet_sizing} @node_ref=&slot translate={shifted}>
                    {panel.call(view! {
                        <Frame>
                            <Interactive
                                @node_ref=&catcher
                                claim_at={move |(pos, _): (Pos2, beui_view::types::Modifiers)| {
                                    motion.value().flinging() && super::scroll::under(&catcher, pos)
                                }}
                                on_press={move |_: PointerPress| motion.value().stop_fling()}
                                on_wheel={gestures::scroll_wheel(axis, move |gesture: ScrollGesture| motion.value().wheel(gesture))}
                                on_pointer={gestures::scroll_drag(axis, move |gesture: DragGesture| motion.value().drag(gesture))}
                            >
                                <List spacing=0.0>
                                    <Interactive
                                        @test_id={"sheet.handle"}
                                        cursor=CursorIcon::ResizeVertical
                                        on_pointer={gestures::scroll_drag(axis, move |gesture: DragGesture| motion.value().outside_drag(gesture))}
                                        on_press={move |press: PointerPress| motion.value().handle_press(press)}
                                        on_drag={move |press: PointerPress| motion.value().handle_drag(press)}
                                        on_cancel={move || motion.value().handle_cancel()}
                                        on_active_change={move |active: bool| motion.value().handle_active(active)}
                                        children={Some(grip)}
                                    />
                                    <Frame
                                        fade_edges={edges}
                                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                    >
                                        <Offset
                                            @node_ref=&content
                                            fit=true
                                            on_change={move |value: ScrollPosition| {
                                                if !motion.value().settle_content(value) {
                                                    position.set(Some(value));
                                                }
                                            }}
                                        >
                                            {children}
                                        </Offset>
                                    </Frame>
                                </List>
                            </Interactive>
                        </Frame>
                    })}
                </Frame>
            </ShowKeepAlive>
        </List>
    };
    on_back(open, move |gesture| motion.value().back(gesture));
    sheet
}

#[component]
pub fn ModalSheet(
    open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    #[prop(default = 0.0)] fade: f32,
    scrim: Prop<Color32>,
    grip: Render<SheetGripHandle>,
    panel: Render<Child>,
    on_close: ClickCallback,
    #[prop(children)] content: Render,
) -> NodeId {
    let shown = open;
    view! {
        <Modal
            open
            scrim
            placement=ModalPlacement::Fill
            motion=ModalMotion::Reported
            on_dismiss={on_close}
        >|modal: ModalHandle|
            <ModalSheetBody
                open={shown}
                rest
                stops
                fit
                fade
                grip
                panel
                on_presence={modal.presence}
                on_close={on_close}
            >
                {content.call(())}
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
    fade: f32,
    grip: Render<SheetGripHandle>,
    panel: Render<Child>,
    on_presence: Callback<f32>,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let gone = create_signal(false);
    let reopened = open;
    create_effect(move || {
        if reopened.get() {
            gone.set(false);
        }
    });
    if let Some(presence) = use_overlay_presence() {
        presence.linger_while(create_memo(move || !gone.get()));
    }
    let drag = Rc::new(Cell::new(None::<Callback<DragGesture>>));
    let target = drag.clone();
    let grip = Render::new(move |handle: SheetGripHandle| {
        target.set(Some(handle.on_drag));
        grip.call(handle)
    });
    let extent = create_signal(0.0);
    let highest = stops.iter().copied().fold(0.0, f32::max);
    let room = create_memo(move || ItemSize::Percent(100.0).min(extent.get() * (1.0 - highest)));
    let built = view! {
        <List spacing=0.0>
            <Interactive
                @sizing={room}
                @test_id={"sheet.outside"}
                on_click={on_close}
                on_pointer={gestures::scroll_drag(Some(Direction::Vertical), move |gesture: DragGesture| {
                    if let Some(handler) = drag.get() { handler.call(gesture); }
                })}
            >
                <Spacer />
            </Interactive>
            <Sheet
                @sizing={ItemSize::Intrinsic.shrink(1.0)}
                extent
                open
                rest
                stops
                fit
                fade
                grip
                panel
                on_presence={move |presence: f32| on_presence.call(presence)}
                on_gone={move || gone.set(true)}
                on_close={on_close}
            >
                {children}
            </Sheet>
        </List>
    };
    let measured = measure(built);
    create_effect(move || {
        if let Some(rect) = measured.get() {
            extent.set(rect.height());
        }
    });
    built
}
