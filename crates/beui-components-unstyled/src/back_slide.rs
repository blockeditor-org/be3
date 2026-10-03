use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use beui_core::geometry::{Vec2, vec2};
use beui_core::input::{BackEdge, BackGesture};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::reactive::{
    BackHandler, Callback, Child, ClickCallback, Prop, Shift, Timer, WriteSignal, clone,
    create_effect, create_signal, create_timer, on_cleanup, untrack, with_document,
};

use super::rubber_band::MAX_ANIMATION_STEP;

pub const BACK_DRAG_SHARE: f32 = 0.3;
const RETURN_SECONDS: f32 = 0.25;
const LEAVE_SECONDS: f32 = 0.2;
const ENTER_SECONDS: f32 = 0.3;
const ENTER_SHARE: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Still,
    Dragging,
    Returning { from: f32, elapsed: f32 },
    Leaving { from: f32, elapsed: f32 },
    Entering { elapsed: f32 },
}

struct State {
    phase: Phase,
    position: f32,
    direction: f32,
    stepped: Instant,
}

#[derive(Clone)]
struct Slide {
    state: Rc<RefCell<State>>,
    animation: Rc<RefCell<Option<Timer>>>,
    set_shift: WriteSignal<Vec2>,
    enters: bool,
    on_back: ClickCallback,
    on_presence: Callback<f32>,
}

fn ease_out(progress: f32, power: i32) -> f32 {
    1.0 - (1.0 - progress).powi(power)
}

impl Slide {
    fn place(&self, position: f32) {
        let direction = {
            let mut state = self.state.borrow_mut();
            state.position = position;
            state.direction
        };
        let width = with_document(|document| document.viewport_rect().width());
        self.set_shift.set(vec2(direction * position * width, 0.0));
        self.on_presence.call(1.0 - position.clamp(0.0, 1.0));
    }

    fn start(&self, phase: Phase) {
        let animate = {
            let mut state = self.state.borrow_mut();
            state.phase = phase;
            !matches!(phase, Phase::Still | Phase::Dragging)
        };
        let Some(animation) = self.animation.borrow().clone() else {
            return;
        };
        match animate {
            true => {
                if !animation.running() {
                    self.state.borrow_mut().stepped = beui_core::timer::now();
                }
                animation.start(Duration::ZERO);
            }
            false => animation.stop(),
        }
    }

    fn gesture(&self, gesture: BackGesture) {
        let (phase, position) = {
            let state = self.state.borrow();
            (state.phase, state.position)
        };
        if matches!(phase, Phase::Leaving { .. }) {
            return;
        }
        match gesture {
            BackGesture::Started { edge } => {
                self.state.borrow_mut().direction = match edge {
                    BackEdge::Right => -1.0,
                    BackEdge::Left | BackEdge::None => 1.0,
                };
                self.start(Phase::Dragging);
                self.place(0.0);
            }
            BackGesture::Progressed(progress) => {
                if phase == Phase::Dragging {
                    self.place(progress * BACK_DRAG_SHARE);
                }
            }
            BackGesture::Cancelled => {
                if phase == Phase::Dragging {
                    self.start(Phase::Returning {
                        from: position,
                        elapsed: 0.0,
                    });
                }
            }
            BackGesture::Invoked => match phase {
                Phase::Dragging | Phase::Returning { .. } => self.start(Phase::Leaving {
                    from: position,
                    elapsed: 0.0,
                }),
                _ => self.on_back.call(),
            },
        }
    }

    fn reset(&self) {
        self.start(Phase::Still);
        self.place(0.0);
    }

    fn gone(&self) {
        self.on_back.call();
        match self.enters {
            true => {
                self.state.borrow_mut().direction *= -1.0;
                self.start(Phase::Entering { elapsed: 0.0 });
                self.place(ENTER_SHARE);
            }
            false => self.reset(),
        }
    }

    fn step(&self) -> Option<Duration> {
        let now = beui_core::timer::now();
        let (phase, elapsed) = {
            let mut state = self.state.borrow_mut();
            let elapsed = now
                .duration_since(state.stepped)
                .as_secs_f32()
                .min(MAX_ANIMATION_STEP);
            state.stepped = now;
            (state.phase, elapsed)
        };
        let (position, next) = match phase {
            Phase::Still | Phase::Dragging => return None,
            Phase::Returning {
                from,
                elapsed: before,
            } => {
                let elapsed = before + elapsed;
                let progress = (elapsed / RETURN_SECONDS).min(1.0);
                let next = match progress >= 1.0 {
                    true => Phase::Still,
                    false => Phase::Returning { from, elapsed },
                };
                (from * (1.0 - ease_out(progress, 3)), next)
            }
            Phase::Leaving {
                from,
                elapsed: before,
            } => {
                let elapsed = before + elapsed;
                let progress = (elapsed / LEAVE_SECONDS).min(1.0);
                if progress >= 1.0 {
                    self.place(1.0);
                    self.gone();
                    return self.running();
                }
                (
                    from + (1.0 - from) * ease_out(progress, 2),
                    Phase::Leaving { from, elapsed },
                )
            }
            Phase::Entering { elapsed: before } => {
                let elapsed = before + elapsed;
                let progress = (elapsed / ENTER_SECONDS).min(1.0);
                let next = match progress >= 1.0 {
                    true => Phase::Still,
                    false => Phase::Entering { elapsed },
                };
                (ENTER_SHARE * (1.0 - ease_out(progress, 3)), next)
            }
        };
        self.state.borrow_mut().phase = next;
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

#[component]
pub fn BackSlide(
    #[prop(default = true)] enabled: Prop<bool>,
    #[prop(default = true)] enters: bool,
    #[prop(default = ClickCallback::default())] on_back: ClickCallback,
    #[prop(default = Callback::default())] on_presence: Callback<f32>,
    children: Child,
) -> NodeId {
    let (shift, set_shift) = create_signal(Vec2::ZERO);
    let slide = Slide {
        state: Rc::new(RefCell::new(State {
            phase: Phase::Still,
            position: 0.0,
            direction: 1.0,
            stepped: beui_core::timer::now(),
        })),
        animation: Rc::new(RefCell::new(None)),
        set_shift,
        enters,
        on_back,
        on_presence,
    };
    let animation = create_timer(clone!(slide -> move || slide.step()));
    slide.animation.replace(Some(animation));
    on_cleanup(clone!(slide -> move || drop(slide.animation.take())));
    let watched = enabled.clone();
    create_effect(clone!(slide -> move || {
        if !watched.get() {
            untrack(|| {
                let phase = slide.state.borrow().phase;
                if matches!(
                    phase,
                    Phase::Dragging | Phase::Returning { .. } | Phase::Leaving { .. }
                ) {
                    slide.reset();
                }
            });
        }
    }));
    view! {
        <BackHandler
            enabled={enabled}
            on_gesture={move |gesture: BackGesture| slide.gesture(gesture)}
        >
            <Shift by={shift}>{children}</Shift>
        </BackHandler>
    }
}
