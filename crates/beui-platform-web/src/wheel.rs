use beui_core::geometry::Vec2;
use beui_core::input::{Input, ScrollDelta};
use beui_core::runner::{PINCH_SPEED, scrolled};

const DOM_DELTA_LINE: u32 = 1;
const DOM_DELTA_PAGE: u32 = 2;

pub(crate) fn wheel(delta: Vec2, mode: u32, ctrl: bool, pinch: &Pinch) -> Option<Input> {
    let delta = match mode {
        DOM_DELTA_LINE => ScrollDelta::Lines(-delta),
        DOM_DELTA_PAGE => ScrollDelta::Pages(-delta),
        _ => ScrollDelta::Pixels(-delta),
    };
    match (ctrl, pinch.active()) {
        (true, true) => None,
        (true, false) => {
            let pinched = scrolled(delta, 1.0).y;
            (pinched != 0.0).then(|| Input::Zoom((pinched * PINCH_SPEED).exp()))
        }
        (false, _) => Some(Input::Scroll(delta)),
    }
}

#[derive(Default)]
pub(crate) struct Pinch {
    scale: Option<f32>,
}

impl Pinch {
    pub(crate) fn start(&mut self) {
        self.scale = Some(1.0);
    }

    pub(crate) fn change(&mut self, scale: f32, touching: bool) -> Option<f32> {
        let last = self.scale?;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        self.scale = Some(scale);
        let factor = scale / last;
        (!touching && factor != 1.0).then_some(factor)
    }

    pub(crate) fn end(&mut self) {
        self.scale = None;
    }

    pub(crate) fn active(&self) -> bool {
        self.scale.is_some()
    }
}

#[cfg(test)]
mod tests;
