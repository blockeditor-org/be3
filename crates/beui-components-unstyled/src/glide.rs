use std::cell::Cell;

use beui_macros::{component, view};
use beui_view::measure::measure;
use beui_view::primitives::Frame;
use beui_view::reactive::*;
use beui_view::types::{Direction, Measured, Pos2, Spring, Vec2};

/// The spring a glide settles by.
pub const GLIDE_SPRING: Spring = Spring {
    stiffness: 500.0,
    damping: 42.0,
};

/// Where a held glide is drawn: its resting top-left corner at `at`, in window
/// coordinates, moved along one axis only if `along` is given.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GlideHold {
    pub at: Pos2,
    pub along: Option<Direction>,
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
struct GlideMotion {
    displacement: Vec2,
    velocity: Vec2,
}

impl GlideMotion {
    fn moving(self) -> bool {
        self.displacement != Vec2::ZERO || self.velocity != Vec2::ZERO
    }

    fn step(&mut self, seconds: f32) -> bool {
        GLIDE_SPRING.advance(&mut self.displacement.x, &mut self.velocity.x, seconds);
        GLIDE_SPRING.advance(&mut self.displacement.y, &mut self.velocity.y, seconds);
        self.moving()
    }

    fn hold(&mut self, along: Option<Direction>, moved: Vec2) {
        self.displacement = match along {
            Some(Direction::Horizontal) => Vec2::new(moved.x, 0.0),
            Some(Direction::Vertical) => Vec2::new(0.0, moved.y),
            None => moved,
        };
        self.velocity = Vec2::ZERO;
    }
}

#[derive(Default)]
struct Motion {
    motion: Cell<GlideMotion>,
    armed: Cell<bool>,
    from: Cell<Option<Pos2>>,
    hold: Cell<Option<GlideHold>>,
    resting: Cell<Option<Pos2>>,
    placed: Cell<Option<Pos2>>,
}

/// A `Glide`'s handle: hold its child at a point while a drag carries it, and read how
/// far from its resting place the child is drawn.
#[derive(Clone, Copy)]
pub struct GlideApi {
    state: Stored<Motion>,
    shown: Signal<Vec2>,
    frames: Animation,
    content: ReadSignal<Option<Measured>>,
}

impl GlideApi {
    fn show(self) {
        let Some(displacement) = self.state.try_with(|state| state.motion.get().displacement)
        else {
            return;
        };
        if self.shown.get_untracked() != displacement {
            self.shown.set(displacement);
        }
    }

    fn start(self) {
        self.frames.start();
        self.show();
    }

    fn take_hold(self) {
        let Some(state) = self.state.try_value() else {
            return;
        };
        let (Some(hold), Some(resting)) = (state.hold.get(), state.resting.get()) else {
            return;
        };
        let mut motion = state.motion.get();
        motion.hold(hold.along, hold.at - resting);
        state.motion.set(motion);
        self.show();
    }

    /// Holds the child with its resting top-left corner at `hold.at` (window
    /// coordinates), along one axis if given, until handed `None`; let go, it springs
    /// home.
    pub fn hold(self, hold: Option<GlideHold>) {
        let Some(state) = self.state.try_value() else {
            return;
        };
        let released = state.hold.get().is_some() && hold.is_none();
        state.hold.set(hold);
        match released {
            true if state.motion.get().displacement != Vec2::ZERO => self.start(),
            true => {}
            false => self.take_hold(),
        }
    }

    /// How far the child is drawn from where layout put the glide.
    pub fn displacement(self) -> Vec2 {
        self.state
            .try_with(|state| state.motion.get().displacement)
            .unwrap_or(Vec2::ZERO)
    }
}

/// Animates its child from where it was to where layout puts it. Bumping `arm`
/// before a change (a reorder) makes the next move glide rather than jump; `from`
/// starts a glide from a window point (an item dropped from elsewhere), at once if it is
/// already placed. The glide is
/// two frames: the outer one is laid out in place and measured, the inner one is drawn
/// offset by the spring, timed on the frame clock. `raised` paints it above its
/// siblings. A glide drawn inside another names it `within`: its moves are then measured
/// against that glide's drawn content, so that glide's offset (a row held under a finger)
/// is no move of its own.
#[component]
pub fn Glide(
    #[prop(default = ComponentRef::new())] api: ComponentRef<GlideApi>,
    #[prop(default = 0)] arm: Prop<u64>,
    #[prop(default = None)] from: Prop<Option<Pos2>>,
    #[prop(default = false)] raised: Prop<bool>,
    #[prop(default = None)] within: Option<ComponentRef<GlideApi>>,
    children: Child,
) -> NodeId {
    let stored = store(Motion::default());
    let shown = create_signal(Vec2::ZERO);
    let frames = create_animation(
        move |frame| {
            let state = stored.value();
            if state.hold.get().is_some() {
                return false;
            }
            let mut motion = state.motion.get();
            let moving = motion.step(frame.delta.as_secs_f32());
            state.motion.set(motion);
            shown.set(motion.displacement);
            moving
        },
        move || {
            stored.try_with(|state| {
                if state.hold.get().is_none() {
                    state.motion.set(GlideMotion::default());
                    shown.set(Vec2::ZERO);
                }
            });
        },
    );
    let glide = GlideApi {
        state: stored,
        shown,
        frames,
        // The child, laid out inside the translated frame, so its place carries the offset.
        content: measure(children),
    };
    api.fill(glide);
    let node = view! {
        <Frame raised>
            <Frame translate={shown}>{children}</Frame>
        </Frame>
    };
    let measured = measure(node);
    // An arm lasts until a layout has run, and every move that layout makes glides: a frame
    // can lay out more than once, and moves that cancel out then net to nothing. It ends there,
    // or it would turn the next move it did not ask for into a glide. An animation's first step
    // can come in the frame that armed it, before that frame's layout, so it ends at its second.
    let waited = StoredCell::new(0_u8);
    let disarm = create_animation(
        move |_| {
            waited.update(|steps| steps + 1);
            waited.get() < 2
        },
        move || {
            stored.try_with(|state| state.armed.set(false));
        },
    );
    let arm_now = move || {
        stored.with(|state| state.armed.set(true));
        waited.set(0);
        disarm.stop();
        disarm.start();
    };
    let first = Cell::new(true);
    create_effect(move || {
        arm.get();
        if first.replace(false) {
            return;
        }
        arm_now();
    });
    create_effect(move || {
        let Some(from) = from.get() else {
            return;
        };
        let state = stored.value();
        match state.resting.get() {
            // Placed already, it starts from there now, and is armed so that a move this
            // same update makes is added on.
            Some(resting) => {
                arm_now();
                let mut motion = state.motion.get();
                motion.displacement += from - resting;
                state.motion.set(motion);
                glide.start();
            }
            None => state.from.set(Some(from)),
        }
    });
    create_effect(move || {
        let state = stored.value();
        let Some(resting) = measured.get().map(|measured| measured.rect.min) else {
            return;
        };
        // Inside another glide, a move is one against that glide's drawn content, which this
        // layout measured with the same offset as this one.
        let content = within
            .and_then(|within| within.try_get())
            .and_then(|within| within.content.get_untracked());
        let placed = match content {
            Some(content) => Pos2::ZERO + (resting - content.rect.min),
            None => resting,
        };
        let mut moved = Vec2::ZERO;
        if let Some(from) = state.from.take() {
            moved += from - resting;
        }
        if let Some(was) = state.placed.get().filter(|_| state.armed.get()) {
            moved += was - placed;
        }
        state.resting.set(Some(resting));
        state.placed.set(Some(placed));
        if state.hold.get().is_some() {
            glide.take_hold();
        } else if moved != Vec2::ZERO {
            let mut motion = state.motion.get();
            motion.displacement += moved;
            state.motion.set(motion);
            glide.start();
        }
    });
    node
}
