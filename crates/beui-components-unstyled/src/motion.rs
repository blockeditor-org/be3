use std::cell::RefCell;
use std::rc::Rc;

use beui_core::motion::Motion;
use beui_view::reactive::with_document;

use super::rubber_band::MAX_ANIMATION_STEP;

pub const FINISHED: f32 = 1.0e6;

pub fn motion() -> Motion {
    with_document(|document| document.motion())
}

pub fn animation_step(elapsed: f32) -> f32 {
    match motion().animates() {
        true => elapsed.min(MAX_ANIMATION_STEP),
        false => FINISHED,
    }
}

type Change = Box<dyn FnOnce()>;

#[derive(Clone, Default)]
pub struct GestureHold(Rc<RefCell<Vec<Change>>>);

impl GestureHold {
    pub fn run(&self, change: impl FnOnce() + 'static) {
        match motion().follows_gestures() {
            true => {
                self.release();
                change();
            }
            false => self.0.borrow_mut().push(Box::new(change)),
        }
    }

    pub fn release(&self) {
        let held = std::mem::take(&mut *self.0.borrow_mut());
        for change in held {
            change();
        }
    }
}
