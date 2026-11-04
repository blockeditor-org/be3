use std::cell::Cell;
use std::rc::Rc;

use crate::measure::measure;
use crate::reactive::{
    Animation, Child, Get, NodeId, Prop, Set, create_animation, create_binding, create_effect,
    create_signal, use_preferences,
};
use crate::types::{GlideHold, GlideMotion, Pos2, Vec2};
use beui_core_html::css;
use beui_core_html::document::{append, create, set_state, set_style, state};

#[derive(Default)]
struct Glide {
    motion: Cell<GlideMotion>,
    armed: Cell<Option<Pos2>>,
    from: Cell<Option<Pos2>>,
    hold: Cell<Option<GlideHold>>,
    resting: Cell<Option<Pos2>>,
    placed: Cell<Option<Pos2>>,
}

#[derive(Clone)]
struct Held {
    glide: Rc<Glide>,
    animates: crate::reactive::Memo<bool>,
    shown: crate::reactive::Signal<Vec2>,
    frames: Animation,
}

impl Held {
    fn show(&self) {
        let displacement = self.glide.motion.get().displacement;
        if self.shown.get_untracked() != displacement {
            self.shown.set(displacement);
        }
    }

    fn start(&self) {
        if self.animates.get_untracked() {
            self.frames.start();
        } else {
            self.glide.motion.set(GlideMotion::default());
        }
        self.show();
    }

    fn take_hold(&self) {
        let (Some(hold), Some(resting)) = (self.glide.hold.get(), self.glide.resting.get()) else {
            return;
        };
        let mut motion = self.glide.motion.get();
        motion.hold(hold.along, hold.at - resting);
        self.glide.motion.set(motion);
        self.show();
    }
}

pub(crate) fn glide(arm: Prop<u64>, from: Prop<Option<Pos2>>, children: Child) -> NodeId {
    let node = create("Glide", "div");
    let shift = create("GlideShift", "div");
    for element in [node, shift] {
        set_style(element, "display", Some("grid".to_owned()));
        set_style(
            element,
            "grid-template",
            Some("minmax(0, 1fr) / minmax(0, 1fr)".to_owned()),
        );
    }
    append(shift, children);
    append(node, shift);
    let glide = Rc::new(Glide::default());
    let shown = create_signal(Vec2::ZERO);
    let frames = {
        let stepped = Rc::clone(&glide);
        let settled = Rc::clone(&glide);
        create_animation(
            move |frame| {
                let glide = &stepped;
                if glide.hold.get().is_some() {
                    return false;
                }
                let mut motion = glide.motion.get();
                let moving = motion.step(frame.delta.as_secs_f32());
                glide.motion.set(motion);
                shown.set(motion.displacement);
                moving
            },
            {
                move || {
                    settled.motion.set(GlideMotion::default());
                    shown.set(Vec2::ZERO);
                }
            },
        )
    };
    let preferences = use_preferences();
    let held = Held {
        glide: Rc::clone(&glide),
        animates: crate::reactive::create_memo(move || preferences.get().motion.animates()),
        shown,
        frames,
    };
    set_state(node, Box::new(held.clone()));
    create_binding(move || {
        let shift_by = shown.get();
        set_style(
            shift,
            "transform",
            (shift_by != Vec2::ZERO).then(|| {
                format!(
                    "translate({}, {})",
                    css::px(shift_by.x),
                    css::px(shift_by.y)
                )
            }),
        );
    });
    let measured = measure(node);
    let first = Cell::new(true);
    {
        let glide = Rc::clone(&glide);
        create_effect(move || {
            arm.get();
            if first.replace(false) {
                return;
            }
            glide.armed.set(glide.placed.get());
        });
    }
    {
        let glide = Rc::clone(&glide);
        create_effect(move || {
            if let Some(from) = from.get() {
                glide.from.set(Some(from));
            }
        });
    }
    create_effect(move || {
        let Some(rect) = measured.get().map(|measured| measured.rect) else {
            return;
        };
        let scrolled = beui_core_html::measure::scrolled_by(node);
        let resting = rect.min;
        glide.resting.set(Some(resting));
        let placed = resting + scrolled;
        let mut moved = Vec2::ZERO;
        if let Some(from) = glide.from.take() {
            moved += from - resting;
        }
        if let Some(armed) = glide.armed.take() {
            moved += armed - placed;
        }
        glide.placed.set(Some(placed));
        if glide.hold.get().is_some() {
            held.take_hold();
        } else if moved != Vec2::ZERO {
            let mut motion = glide.motion.get();
            motion.displacement += moved;
            glide.motion.set(motion);
            held.start();
        }
    });
    node
}

pub(crate) fn hold_glide(glide: NodeId, hold: Option<GlideHold>) {
    let Some(held) = state::<Held>(glide) else {
        return;
    };
    let released = held.glide.hold.get().is_some() && hold.is_none();
    held.glide.hold.set(hold);
    match released {
        true if held.glide.motion.get().displacement != Vec2::ZERO => held.start(),
        true => {}
        false => held.take_hold(),
    }
}

pub(crate) fn glide_displacement(glide: NodeId) -> Option<Vec2> {
    state::<Held>(glide).map(|held| held.glide.motion.get().displacement)
}

pub(crate) fn set_raised(node: NodeId, raised: bool) {
    set_style(node, "position", raised.then(|| "relative".to_owned()));
    set_style(node, "z-index", raised.then(|| "1".to_owned()));
}
