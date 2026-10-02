use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_macros::{component, view};

use super::back_slide::BACK_DRAG_SHARE;
use super::fling::Fling;
use super::rubber_band::{
    MAX_ANIMATION_STEP, SCROLL_SPRING, WINDOW_SPRING, rubber_band, spring_back, unband,
};
use beui_core::base::Direction;
use beui_core::base::offset::{OffsetNode, ScrollPosition};
use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::color::Color32;
use beui_core::geometry::{Pos2, Rect, vec2};
use beui_core::input::{BackGesture, CursorIcon, DragGesture, PointerPress, ScrollGesture};
use beui_core::node::{NodeId, NodeOf};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    BackHandler, Callback, Child, ClickCallback, Frame, Interactive, ItemSize, List, Memo, NodeRef,
    Offset, Prop, ReadSignal, Render, Shift, Spacer, Timer, WriteSignal, clone, component_size,
    create_effect, create_memo, create_signal, create_timer, on_cleanup, untrack, with_document,
};

pub const SHEET_STOPS: [f32; 3] = [0.3, 0.5, 0.9];

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

pub struct SheetGripHandle {
    pub dragging: ReadSignal<bool>,
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
    Gone,
}

struct Held {
    travel: Option<f32>,
    handle: Option<f32>,
    phase: Phase,
    slide: Slide,
    flung: f32,
    backing: bool,
    stepped: Instant,
}

#[derive(Clone)]
struct Motion {
    extent: Memo<f32>,
    shares: Rc<[f32]>,
    resting: Rest,
    fit: bool,
    rest_at: ReadSignal<Rest>,
    shifted: Rc<Cell<f32>>,
    slot: NodeRef,
    chrome: Rc<Cell<f32>>,
    set_rest: WriteSignal<Rest>,
    set_dragging: WriteSignal<bool>,
    content: NodeRef,
    held: Rc<RefCell<Held>>,
    animation: Rc<RefCell<Option<Timer>>>,
    open: Prop<bool>,
    set_shift: WriteSignal<f32>,
    set_gone: WriteSignal<bool>,
    on_presence: Callback<f32>,
    on_gone: ClickCallback,
    on_close: ClickCallback,
}

fn banding() -> bool {
    with_document(|document| document.rubber_banding())
}

impl Motion {
    fn node(&self) -> Option<NodeOf<OffsetNode>> {
        let content = self.content.try_get()?;
        with_document(|document| document.first_offset_within(content))
    }

    fn laid_out(&self) -> (Option<Rect>, Option<Rect>, ScrollPosition) {
        let node = self.node();
        let slot = self.slot.try_get();
        with_document(|document| {
            let sheet = slot.and_then(|slot| document.node_rect(slot));
            let node = node.filter(|node| document.contains(*node));
            let body = node.and_then(|node| document.node_rect(node));
            let position = node
                .and_then(|node| document.offset_position(node))
                .unwrap_or_default();
            (sheet, body, position)
        })
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
        let highest = self.shares.last().copied().unwrap_or(SHEET_STOPS[2]);
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
            false => Rest::Share(self.shares.last().copied().unwrap_or(SHEET_STOPS[2])),
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
        with_document(|document| {
            if !document.contains(node) {
                return (0.0, 0.0);
            }
            (
                document.offset_value(node),
                document.scroll_overscroll(node.id()),
            )
        })
    }

    fn place_content(&self, offset: f32, overscroll: f32) {
        let Some(node) = self.node() else {
            return;
        };
        with_document(|document| {
            if document.contains(node) {
                document.drive_offset(node, offset);
                document.set_offset_overscroll(node, overscroll);
            }
        });
    }

    fn place_height(&self, rest: Rest) {
        self.set_rest.set(rest);
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
        let shape = self.shape();
        let travel = self.travel(&shape);
        let mut held = self.held.borrow_mut();
        held.travel = Some(travel);
        held.phase = Phase::Still;
        self.set_dragging.set(true);
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
        self.set_dragging.set(false);
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
            Fling::new(velocity).map_or(Phase::Still, Phase::Fling)
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
        if untrack(|| self.open.get()) {
            self.held.borrow_mut().flung = 0.0;
            self.start(Phase::Snap {
                target: self.resting,
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
                let mut held = self.held.borrow_mut();
                if height.is_some()
                    && held.travel.is_none()
                    && matches!(held.slide, Slide::Shown | Slide::Returning { .. })
                {
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
        self.set_shift.set(shift);
        self.on_presence.call(1.0 - shift / height);
    }

    fn sheet_height(&self) -> Option<f32> {
        self.laid_out().0.map(|sheet| sheet.height())
    }

    fn enter(&self) {
        self.set_gone.set(false);
        self.reset();
        {
            let mut held = self.held.borrow_mut();
            held.slide = Slide::Entering {
                elapsed: 0.0,
                waited: 0,
            };
        }
        self.shifted.set(HIDDEN_SHIFT);
        self.set_shift.set(HIDDEN_SHIFT);
        self.on_presence.call(0.0);
        self.kick();
    }

    fn leave(&self) {
        let Some(height) = self.sheet_height().filter(|height| *height > 0.0) else {
            self.held.borrow_mut().slide = Slide::Gone;
            self.on_presence.call(0.0);
            self.set_gone.set(true);
            self.on_gone.call();
            return;
        };
        {
            let mut held = self.held.borrow_mut();
            held.travel = None;
            held.handle = None;
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
        self.set_dragging.set(false);
        self.kick();
    }

    fn kick(&self) {
        let Some(animation) = self.animation.borrow().clone() else {
            return;
        };
        if !animation.running() {
            self.held.borrow_mut().stepped = beui_core::timer::now();
        }
        animation.start(Duration::ZERO);
    }

    fn step_slide(&self, elapsed: f32) -> bool {
        let slide = self.held.borrow().slide;
        let height = match (slide, self.sheet_height().filter(|height| *height > 0.0)) {
            (Slide::Shown | Slide::Gone, _) => return false,
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
                self.set_shift.set(0.0);
                self.on_presence.call(1.0);
                return false;
            }
            (Slide::Returning { .. }, None) => {
                self.held.borrow_mut().slide = Slide::Shown;
                self.shifted.set(0.0);
                self.set_shift.set(0.0);
                self.on_presence.call(1.0);
                return false;
            }
            (Slide::Leaving { .. }, None) => {
                self.held.borrow_mut().slide = Slide::Gone;
                self.on_presence.call(0.0);
                self.set_gone.set(true);
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
                let progress = (elapsed / ENTER_SECONDS).min(1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
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
                let progress = (elapsed / RETURN_SECONDS).min(1.0);
                let eased = 1.0 - (1.0 - progress).powi(3);
                let next = match progress >= 1.0 {
                    true => Slide::Shown,
                    false => Slide::Returning { elapsed, from },
                };
                (from * (1.0 - eased), next)
            }
            Slide::Shown | Slide::Gone => return false,
        };
        {
            let mut held = self.held.borrow_mut();
            held.slide = next;
        }
        self.place_shift(shift, height);
        match next {
            Slide::Gone => {
                self.set_gone.set(true);
                self.on_gone.call();
                false
            }
            Slide::Shown => false,
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
        let Some(animation) = self.animation.borrow().clone() else {
            return;
        };
        if !animation.running() {
            held.stepped = beui_core::timer::now();
        }
        animation.start(Duration::ZERO);
    }

    fn flinging(&self) -> bool {
        matches!(self.held.borrow().phase, Phase::Fling(_))
    }

    fn stop_fling(&self) {
        let mut held = self.held.borrow_mut();
        if matches!(held.phase, Phase::Fling(_)) {
            held.phase = Phase::Still;
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
        self.held.borrow_mut().phase = Phase::Still;
        let (offset, _) = self.content();
        self.place_content((offset - wheel).clamp(0.0, shape.max_offset), 0.0);
        if fling != 0.0 {
            self.start(Fling::new(-fling).map_or(Phase::Still, Phase::Fling));
        }
    }

    fn handle_press(&self, press: PointerPress) {
        self.grab();
        self.held.borrow_mut().handle = Some(press.pos.y);
    }

    fn handle_drag(&self, press: PointerPress) {
        let Some(last) = self.held.borrow_mut().handle.replace(press.pos.y) else {
            return;
        };
        self.pull(press.pos.y - last);
    }

    fn handle_cancel(&self) {
        if self.held.borrow_mut().handle.take().is_some() {
            self.release(0.0);
        }
    }

    fn handle_active(&self, active: bool) {
        if active || self.held.borrow_mut().handle.take().is_none() {
            return;
        }
        self.release(0.0);
    }

    fn reset(&self) {
        let mut held = self.held.borrow_mut();
        held.travel = None;
        held.handle = None;
        held.phase = Phase::Still;
        drop(held);
        self.set_dragging.set(false);
        self.place_height(self.resting);
        self.place_content(0.0, 0.0);
    }

    fn step(&self) -> Option<Duration> {
        let now = beui_core::timer::now();
        let mut held = self.held.borrow_mut();
        let elapsed = now
            .duration_since(held.stepped)
            .as_secs_f32()
            .min(MAX_ANIMATION_STEP);
        held.stepped = now;
        drop(held);
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
                } else if raw != placed || done {
                    held.phase = Phase::Still;
                }
                let moving = !matches!(held.phase, Phase::Still);
                drop(held);
                self.place_content(placed, overscroll);
                moving
            }
            Phase::Bounce { velocity } => {
                let mut overscroll = overscroll;
                spring_back(&mut overscroll, velocity, elapsed, SCROLL_SPRING);
                let settled = overscroll == 0.0 && *velocity == 0.0;
                if settled {
                    held.phase = Phase::Still;
                }
                drop(held);
                self.place_content(offset, overscroll);
                !settled
            }
        }
    }
}

#[component]
pub fn Sheet(
    extent: Prop<f32>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = SHEET_STOPS[1])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    grip: Render<SheetGripHandle>,
    panel: Render<Child>,
    on_presence: Callback<f32>,
    on_gone: ClickCallback,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let mut stops = match stops.is_empty() {
        true => SHEET_STOPS.to_vec(),
        false => stops,
    };
    stops.sort_by(f32::total_cmp);
    let shares: Rc<[f32]> = Rc::from(stops);
    let resting = match fit {
        true => Rest::Fitted,
        false => Rest::Share(rest),
    };
    let extent = create_memo(move || extent.get().max(1.0));
    let (rest_at, set_rest) = create_signal(resting);
    let (dragging, set_dragging) = create_signal(false);
    let (shift, set_shift) = create_signal(HIDDEN_SHIFT);
    let (gone, set_gone) = create_signal(true);
    let sizing = create_memo(clone!(extent rest_at -> move || match rest_at.get() {
        _ if gone.get() => ItemSize::Fixed(0.0).into(),
        Rest::Share(share) => ItemSize::Intrinsic.max(share * extent.get()).shrink(1.0),
        Rest::Fitted => ItemSize::Intrinsic.shrink(1.0),
        Rest::Free(height) => ItemSize::Fixed(height).shrink(1.0),
    }));
    let content = NodeRef::new();
    let slot = NodeRef::new();
    let motion = Motion {
        extent,
        shares,
        resting,
        fit,
        rest_at,
        shifted: Rc::new(Cell::new(HIDDEN_SHIFT)),
        slot: slot.clone(),
        chrome: Rc::default(),
        set_rest,
        set_dragging,
        content: content.clone(),
        held: Rc::new(RefCell::new(Held {
            travel: None,
            handle: None,
            phase: Phase::Still,
            slide: Slide::Gone,
            flung: 0.0,
            backing: false,
            stepped: beui_core::timer::now(),
        })),
        animation: Rc::default(),
        open: open.clone(),
        set_shift,
        set_gone,
        on_presence,
        on_gone,
        on_close,
    };
    let animation = create_timer(clone!(motion -> move || motion.step()));
    motion.animation.replace(Some(animation));
    on_cleanup(clone!(motion -> move || drop(motion.animation.take())));
    let shown = open.clone();
    let was_open = Cell::new(false);
    create_effect(clone!(motion -> move || {
        let open = shown.get();
        if open == was_open.replace(open) {
            return;
        }
        untrack(|| match open {
            true => motion.enter(),
            false => motion.leave(),
        });
    }));
    let shifted = create_memo(move || vec2(0.0, shift.get()));
    let axis = create_memo(|| Some(Direction::Vertical));
    let (tapped, stopped) = (motion.clone(), motion.clone());
    let (wheeled, dragged) = (motion.clone(), motion.clone());
    let (pressed, handled) = (motion.clone(), motion.clone());
    let (cancelled, released) = (motion.clone(), motion.clone());
    let grip = grip.call(SheetGripHandle { dragging });
    view! {
        <List spacing=0.0>
            <BackHandler
                @sizing={sizing}
                enabled={open}
                on_gesture={move |gesture: BackGesture| motion.back(gesture)}
            >
                <Shift @node_ref=&slot by={shifted}>
                    {panel.call(view! {
                        <Frame>
                            <Interactive
                                scroll_axis={axis}
                                intercept_at={move |_: Pos2| tapped.flinging()}
                                on_press={move |_: PointerPress| stopped.stop_fling()}
                                on_scroll={move |gesture: ScrollGesture| wheeled.wheel(gesture)}
                                on_scroll_drag={move |gesture: DragGesture| dragged.drag(gesture)}
                            >
                                <List spacing=0.0>
                                    <Interactive
                                        @test_id={"sheet.handle"}
                                        cursor=CursorIcon::ResizeVertical
                                        on_press={move |press: PointerPress| pressed.handle_press(press)}
                                        on_drag={move |press: PointerPress| handled.handle_drag(press)}
                                        on_cancel={move || cancelled.handle_cancel()}
                                        on_active_change={move |active: bool| released.handle_active(active)}
                                        children={Some(grip)}
                                    />
                                    <Offset
                                        @node_ref=&content
                                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                                        fit=true
                                    >
                                        {children}
                                    </Offset>
                                </List>
                            </Interactive>
                        </Frame>
                    })}
                </Shift>
            </BackHandler>
        </List>
    }
}

#[component]
pub fn ModalSheet(
    open: Prop<bool>,
    #[prop(default = SHEET_STOPS[2])] rest: f32,
    #[prop(default = SHEET_STOPS.to_vec())] stops: Vec<f32>,
    #[prop(default = false)] fit: bool,
    #[prop(default = Color32::TRANSPARENT)] scrim: Color32,
    grip: Render<SheetGripHandle>,
    panel: Render<Child>,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let dismiss = on_close.clone();
    let (gone, set_gone) = create_signal(true);
    let (presence, set_presence) = create_signal(0.0_f32);
    let opened = open.clone();
    let appearing = set_gone.clone();
    create_effect(move || {
        if opened.get() {
            appearing.set(false);
        }
    });
    let shown = create_memo(clone!(open -> move || open.get() || !gone.get()));
    let faded = create_memo(move || scrim.scale_alpha(presence.get()));
    view! {
        <Overlay
            anchor=OverlayAnchor::Point(Pos2::ZERO)
            open={shown}
            placement=Placement::Fill
            scrim={faded}
            on_dismiss={move || dismiss.call()}
        >
            <ModalSheetBody
                open
                rest
                stops
                fit
                grip
                panel
                on_presence={move |presence: f32| set_presence.set(presence)}
                on_gone={move || set_gone.set(true)}
                on_close={move || on_close.call()}
            >
                {children}
            </ModalSheetBody>
        </Overlay>
    }
}

#[component]
fn ModalSheetBody(
    open: Prop<bool>,
    rest: f32,
    stops: Vec<f32>,
    fit: bool,
    grip: Render<SheetGripHandle>,
    panel: Render<Child>,
    on_presence: Callback<f32>,
    on_gone: ClickCallback,
    on_close: ClickCallback,
    children: Child,
) -> NodeId {
    let size = component_size();
    let extent = create_memo(move || size.get().y);
    let highest = stops.iter().copied().fold(SHEET_STOPS[0], f32::max);
    let room = create_memo(clone!(extent -> move || {
        ItemSize::Percent(100.0).min(extent.get() * (1.0 - highest))
    }));
    let outside = on_close.clone();
    let closing = on_close;
    view! {
        <List spacing=0.0>
            <Interactive
                @sizing={room}
                @test_id={"sheet.outside"}
                on_click={move || outside.call()}
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
                grip
                panel
                on_presence={move |presence: f32| on_presence.call(presence)}
                on_gone={move || on_gone.call()}
                on_close={move || closing.call()}
            >
                {children}
            </Sheet>
        </List>
    }
}
