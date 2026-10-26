use std::cell::{Cell, RefCell};
use std::time::Duration;

use beui_macros::{component, view};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Color32;
use beui_view::types::{BackEdge, BackGesture};
use beui_view::types::{Vec2, vec2};

pub const BACK_DRAG_SHARE: f32 = 0.3;
const RETURN_SECONDS: f32 = 0.25;
const LEAVE_SECONDS: f32 = 0.2;
const ENTER_SECONDS: f32 = 0.3;
const ENTER_SHARE: f32 = 0.25;
const BEHIND_SHARE: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Still,
    Dragging,
    Returning { from: f32 },
    Leaving { from: f32 },
    Entering,
}

struct State {
    phase: Phase,
    position: f32,
    direction: f32,
}

struct Slide {
    state: RefCell<State>,
    animation: Cell<Option<Animation>>,
    shift: Signal<Vec2>,
    behind_shift: Signal<Vec2>,
    cover: Signal<f32>,
    revealing: Signal<bool>,
    behind: bool,
    enters: bool,
    open: Option<Prop<bool>>,
    on_back: ClickCallback,
    on_gone: ClickCallback,
    on_presence: Callback<f32>,
}

impl Slide {
    fn place(&self, position: f32) {
        let direction = {
            let mut state = self.state.borrow_mut();
            state.position = position;
            state.direction
        };
        let width = viewport_rect().width();
        self.shift.set(vec2(direction * position * width, 0.0));
        let behind = -direction * BEHIND_SHARE * (1.0 - position.clamp(0.0, 1.0)) * width;
        self.behind_shift.set(vec2(behind, 0.0));
        self.cover.set(1.0 - position.clamp(0.0, 1.0));
        self.on_presence.call(1.0 - position.clamp(0.0, 1.0));
    }

    fn start(&self, phase: Phase) {
        let animate = {
            let mut state = self.state.borrow_mut();
            state.phase = phase;
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
            self.behind
                && matches!(
                    phase,
                    Phase::Dragging | Phase::Returning { .. } | Phase::Leaving { .. }
                ),
        );
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
                    self.start(Phase::Returning { from: position });
                }
            }
            BackGesture::Invoked => {
                if phase == Phase::Still {
                    self.state.borrow_mut().direction = 1.0;
                }
                match self.open {
                    None => self.start(Phase::Leaving { from: position }),
                    Some(open) => {
                        self.on_back.call();
                        match open.get_untracked() {
                            true if phase == Phase::Dragging => {
                                self.start(Phase::Returning { from: position })
                            }
                            true => {}
                            false => self.leave(),
                        }
                    }
                }
            }
        }
    }

    fn leave(&self) {
        let (phase, position) = {
            let state = self.state.borrow();
            (state.phase, state.position)
        };
        if !matches!(phase, Phase::Leaving { .. }) && position < 1.0 {
            self.start(Phase::Leaving { from: position });
        }
    }

    fn reset(&self) {
        self.start(Phase::Still);
        self.place(0.0);
    }

    fn gone(&self) {
        if self.open.is_some() {
            self.start(Phase::Still);
            self.on_gone.call();
            return;
        }
        self.on_back.call();
        match self.enters && !self.behind {
            true => {
                self.state.borrow_mut().direction *= -1.0;
                self.place(ENTER_SHARE);
                self.start(Phase::Entering);
            }
            false => self.reset(),
        }
    }

    fn finish(&self) {
        let phase = self.state.borrow().phase;
        match phase {
            Phase::Leaving { .. } => {
                self.place(1.0);
                self.gone();
            }
            Phase::Returning { .. } | Phase::Entering => self.reset(),
            _ => {}
        }
    }

    fn step(&self, frame: AnimationFrame) -> Option<Duration> {
        let phase = self.state.borrow().phase;
        let elapsed = frame.elapsed.as_secs_f32();
        let (position, next) = match phase {
            Phase::Still | Phase::Dragging => return None,
            Phase::Returning { from } => {
                let progress = tween(elapsed, RETURN_SECONDS, Easing::OutCubic);
                let next = if progress >= 1.0 { Phase::Still } else { phase };
                (from * (1.0 - progress), next)
            }
            Phase::Leaving { from } => {
                let progress = tween(elapsed, LEAVE_SECONDS, Easing::OutQuad);
                if progress >= 1.0 {
                    self.place(1.0);
                    self.gone();
                    return self.running();
                }
                (from + (1.0 - from) * progress, phase)
            }
            Phase::Entering => {
                let progress = tween(elapsed, ENTER_SECONDS, Easing::OutCubic);
                let next = if progress >= 1.0 { Phase::Still } else { phase };
                (ENTER_SHARE * (1.0 - progress), next)
            }
        };
        self.state.borrow_mut().phase = next;
        self.reveal(next);
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
    #[prop(default = None)] open: Option<Prop<bool>>,
    #[prop(default = ClickCallback::default())] on_gone: ClickCallback,
    #[prop(default = Callback::default())] on_presence: Callback<f32>,
    #[prop(default = None)] behind: Option<RenderFn<()>>,
    #[prop(default = Color32::TRANSPARENT)] shade: Prop<Color32>,
    children: Child,
) -> NodeId {
    let shift = create_signal(Vec2::ZERO);
    let behind_shift = create_signal(Vec2::ZERO);
    let cover = create_signal(1.0_f32);
    let shade = create_memo(move || shade.get().scale_alpha(cover.get()));
    let revealing = create_signal(false);
    let slide = store(Slide {
        state: RefCell::new(State {
            phase: Phase::Still,
            position: 0.0,
            direction: 1.0,
        }),
        animation: Cell::new(None),
        shift,
        behind_shift,
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
                if matches!(
                    phase,
                    Phase::Dragging | Phase::Returning { .. } | Phase::Leaving { .. }
                ) {
                    slide.reset();
                }
            });
        }
    });
    if let Some(open) = open {
        let was_open = Cell::new(true);
        create_effect(move || {
            let open = open.get();
            if open == was_open.replace(open) {
                return;
            }
            untrack(|| {
                let slide = slide.value();
                match open {
                    true => slide.reset(),
                    false => slide.leave(),
                }
            });
        });
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
            <Show condition={revealing}>|| {
                let page = behind.call(());
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
