use std::cell::{Cell, RefCell};
use std::time::Duration;

use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color;
use beui_view::types::{BackEdge, BackGesture};
use beui_view::types::{Vec2, Velocity, pos2, vec2};

pub const BACK_DRAG_SHARE: f32 = 0.3;
const ENTER_SHARE: f32 = 0.25;
const BEHIND_SHARE: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Still,
    Dragging,
    Returning,
    Leaving,
    Entering,
}

struct State {
    phase: Phase,
    position: f32,
    direction: f32,
    /// The page's motion towards where it is going, its speed in points a second
    /// toward leaving: the gesture's as it is let go, then its spring's.
    motion: SpringMotion,
    /// The speed the gesture let a leaving page go at, which the page coming
    /// in starts at.
    thrown: f32,
    /// The gesture's progress on the frame clock, for the speed it is let go at.
    tracked: Option<Velocity>,
}

/// How a back slide's motions look: the springs a page slides in on (`enter`),
/// carries on off the edge on (`leave`), and comes back to rest on when a
/// gesture is cancelled (`settle`). Each starts at the speed the back gesture
/// was let go at, or at rest after a back with no gesture.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SlideCurves {
    pub enter: Spring,
    pub leave: Spring,
    pub settle: Spring,
}

struct Slide {
    curves: SlideCurves,
    state: RefCell<State>,
    animation: Cell<Option<Animation>>,
    shift: Signal<Vec2>,
    behind_shift: Signal<Vec2>,
    viewport: Memo<Vec2>,
    cover: Signal<f32>,
    revealing: Signal<bool>,
    behind: bool,
    enters: bool,
    open: Option<Prop<bool>>,
    on_back: Callback<()>,
    on_gone: Callback<()>,
    on_presence: Callback<f32>,
}

impl Slide {
    fn width(&self) -> f32 {
        self.viewport.get_untracked().x.max(1.0)
    }

    fn place(&self, position: f32) {
        let direction = {
            let mut state = self.state.borrow_mut();
            state.position = position;
            state.direction
        };
        let width = self.viewport.get_untracked().x;
        self.shift.set(vec2(direction * position * width, 0.0));
        let behind = -direction * BEHIND_SHARE * (1.0 - position.clamp(0.0, 1.0)) * width;
        self.behind_shift.set(vec2(behind, 0.0));
        self.cover.set(1.0 - position.clamp(0.0, 1.0));
        self.on_presence.call(1.0 - position.clamp(0.0, 1.0));
    }

    /// Follows the gesture's progress on the frame clock, so a release knows
    /// how fast the page was moving.
    fn track(&self, position: f32) {
        let point = pos2(position * self.width(), 0.0);
        let at = now();
        let mut state = self.state.borrow_mut();
        match state.tracked.as_mut() {
            Some(tracked) => tracked.sample(at, point),
            None => state.tracked = Some(Velocity::new(at, point)),
        }
    }

    /// The speed the gesture was let go at, in points a second toward leaving;
    /// zero for a back with no gesture.
    fn released(&self) -> f32 {
        self.state
            .borrow_mut()
            .tracked
            .take()
            .map_or(0.0, |tracked| tracked.get().x)
    }

    fn start(&self, phase: Phase, velocity: f32) {
        let animate = {
            let mut state = self.state.borrow_mut();
            state.phase = phase;
            state.motion.release(velocity);
            if phase == Phase::Leaving {
                state.thrown = velocity;
            }
            !matches!(phase, Phase::Still | Phase::Dragging)
        };
        self.reveal(phase);
        let Some(animation) = self.animation.get() else {
            return;
        };
        match animate {
            true => {
                animation.stop();
                animation.start();
            }
            false => animation.stop(),
        }
    }

    fn reveal(&self, phase: Phase) {
        self.revealing.set(
            self.behind && matches!(phase, Phase::Dragging | Phase::Returning | Phase::Leaving),
        );
    }

    fn gesture(&self, gesture: BackGesture) {
        let phase = self.state.borrow().phase;
        if phase == Phase::Leaving {
            return;
        }
        match gesture {
            BackGesture::Started { edge } => {
                {
                    let mut state = self.state.borrow_mut();
                    state.direction = match edge {
                        BackEdge::Right => -1.0,
                        BackEdge::Left | BackEdge::None => 1.0,
                    };
                    state.tracked = None;
                }
                self.start(Phase::Dragging, 0.0);
                self.place(0.0);
                self.track(0.0);
            }
            BackGesture::Progressed(progress) => {
                if phase == Phase::Dragging {
                    let position = progress * BACK_DRAG_SHARE;
                    self.place(position);
                    self.track(position);
                }
            }
            BackGesture::Cancelled => {
                if phase == Phase::Dragging {
                    let velocity = self.released();
                    self.start(Phase::Returning, velocity);
                }
            }
            BackGesture::Invoked => {
                let velocity = match phase {
                    Phase::Dragging => self.released(),
                    _ => 0.0,
                };
                if phase == Phase::Still {
                    self.state.borrow_mut().direction = 1.0;
                }
                match self.open {
                    None => self.start(Phase::Leaving, velocity),
                    Some(open) => {
                        self.on_back.call(());
                        match open.get_untracked() {
                            true if phase == Phase::Dragging => {
                                self.start(Phase::Returning, velocity)
                            }
                            true => {}
                            false => self.leave(velocity),
                        }
                    }
                }
            }
        }
    }

    fn leave(&self, velocity: f32) {
        let (phase, position) = {
            let state = self.state.borrow();
            (state.phase, state.position)
        };
        if phase != Phase::Leaving && position < 1.0 {
            self.start(Phase::Leaving, velocity);
        }
    }

    fn reset(&self) {
        self.state.borrow_mut().tracked = None;
        self.start(Phase::Still, 0.0);
        self.place(0.0);
    }

    fn gone(&self) {
        let thrown = self.state.borrow().thrown;
        if self.open.is_some() {
            self.start(Phase::Still, 0.0);
            self.on_gone.call(());
            return;
        }
        self.on_back.call(());
        match self.enters && !self.behind {
            true => {
                self.state.borrow_mut().direction *= -1.0;
                self.place(ENTER_SHARE);
                // The page coming in carries on the way the gesture threw the
                // one going: toward its rest, at the speed it was let go at.
                self.start(Phase::Entering, -thrown.max(0.0));
            }
            false => self.reset(),
        }
    }

    fn finish(&self) {
        let phase = self.state.borrow().phase;
        match phase {
            Phase::Leaving => {
                self.place(1.0);
                self.gone();
            }
            Phase::Returning | Phase::Entering => self.reset(),
            _ => {}
        }
    }

    fn step(&self, frame: AnimationFrame) -> Option<Duration> {
        let (phase, position, mut motion) = {
            let state = self.state.borrow();
            (state.phase, state.position, state.motion)
        };
        let width = self.width();
        let seconds = frame.delta.as_secs_f32();
        let (spring, target) = match phase {
            Phase::Still | Phase::Dragging => return None,
            Phase::Returning => (self.curves.settle, 0.0),
            Phase::Leaving => (self.curves.leave, 1.0),
            Phase::Entering => (self.curves.enter, 0.0),
        };
        // It stops at its target (gone, or at rest without showing what is beside it) the
        // moment it reaches it, which the motion finds within the step however long it is.
        motion.displacement = (position - target) * width;
        let moving = motion.advance(spring, seconds);
        let position = target + motion.displacement / width;
        match phase {
            Phase::Leaving if position >= 1.0 || !moving => {
                self.place(1.0);
                self.gone();
                return self.running();
            }
            // A page coming to rest stops there rather than swinging past it
            // and showing what is beside it.
            Phase::Returning | Phase::Entering if position <= 0.0 => {
                self.reset();
                return None;
            }
            _ => {}
        }
        self.state.borrow_mut().motion = motion;
        self.place(position);
        self.running()
    }

    fn running(&self) -> Option<Duration> {
        match self.state.borrow().phase {
            Phase::Still | Phase::Dragging => None,
            _ => Some(Duration::ZERO),
        }
    }
}

/// The motion for [`on_back`](beui_view::reactive::on_back), which only routes.
///
/// Held, the page follows the finger; let go, it carries on off the edge on
/// the `leave` spring, at the speed it was let go at, before `on_back` runs,
/// and the next page comes in on `enter` at that speed; a cancelled gesture
/// springs it back on `settle`; a back with no gesture carries it off from
/// rest. The gesture's speed is read from its progress on the frame clock.
/// `behind` builds the page back goes to under the moving one for the
/// gesture's length, so `on_back` swaps in what was already showing; the
/// moving page needs an opaque background, and a page that must keep its state
/// is built once and handed to a `Portal` in both places. Given `open`, a back
/// only asks: `on_back` runs at once, a held page springs back, and the page
/// slides away when `open` falls, calling `on_gone` once it's off.
#[component]
pub fn BackSlide(
    curves: SlideCurves,
    #[prop(default = true)] enabled: Prop<bool>,
    #[prop(default = true)] enters: bool,
    #[prop(default = Callback::default())] on_back: Callback<()>,
    open: Option<Prop<bool>>,
    #[prop(default = Callback::default())] on_gone: Callback<()>,
    #[prop(default = Callback::default())] on_presence: Callback<f32>,
    behind: Option<BuildFn<NodeId>>,
    #[prop(default = Color::TRANSPARENT)] shade: Prop<Color>,
    children: Child,
) -> NodeId {
    let shift = create_signal(Vec2::ZERO);
    let behind_shift = create_signal(Vec2::ZERO);
    let cover = create_signal(1.0_f32);
    let shade = create_memo(move || shade.get().scale_alpha(cover.get()));
    let revealing = create_signal(false);
    let slide = store(Slide {
        curves,
        state: RefCell::new(State {
            phase: Phase::Still,
            position: 0.0,
            direction: 1.0,
            motion: SpringMotion::default(),
            thrown: 0.0,
            tracked: None,
        }),
        animation: Cell::new(None),
        shift,
        behind_shift,
        viewport: use_viewport_size(),
        cover,
        revealing,
        behind: behind.is_some(),
        enters,
        open,
        on_back,
        on_gone,
        on_presence,
    });
    let animation = create_animation(
        move |frame| slide.value().step(frame).is_some(),
        move || slide.value().finish(),
    );
    slide.with(|slide| slide.animation.set(Some(animation)));
    create_effect(move || {
        if !enabled.get() {
            untrack(|| {
                let slide = slide.value();
                let phase = slide.state.borrow().phase;
                if matches!(phase, Phase::Dragging | Phase::Returning | Phase::Leaving) {
                    slide.reset();
                }
            });
        }
    });
    if let Some(open) = open {
        create_effect_on(
            move || open.get(),
            move |&open, was| {
                if open == was.copied().unwrap_or(true) {
                    return;
                }
                let slide = slide.value();
                match open {
                    true => slide.reset(),
                    false => slide.leave(0.0),
                }
            },
        );
    }
    let page = view! {
        <Frame translate={shift}>{children}</Frame>
    };
    beui_view::reactive::on_back(enabled, move |gesture| slide.value().gesture(gesture));
    let Some(behind) = behind else {
        return page;
    };
    view! {
        <Layers>
            <Show value={revealing}>|| {
                let page = behind.call();
                view! {
                    <Frame translate={behind_shift}>
                        <Layers>
                            {page}
                            <Frame fill=shade />
                        </Layers>
                    </Frame>
                }
            }</Show>
            {page}
        </Layers>
    }
}
