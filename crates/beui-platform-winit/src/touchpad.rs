#[cfg(test)]
mod tests;

use beui_types::{TouchpadGesture, Vec2};
use winit::event::TouchPhase;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Gesture {
    Pan(Vec2),
    Pinch(f64),
    Rotation(f32),
    Hold,
}

#[derive(Default)]
pub(crate) struct Touchpad {
    pinching: bool,
    holding: bool,
    scale: f32,
    panned: Vec2,
}

impl Touchpad {
    pub(crate) fn event(&mut self, gesture: Gesture, phase: TouchPhase) -> Option<TouchpadGesture> {
        let ended = match phase {
            TouchPhase::Started | TouchPhase::Moved => None,
            TouchPhase::Ended => Some(false),
            TouchPhase::Cancelled => Some(true),
        };
        if gesture == Gesture::Hold {
            return match ended {
                None if !self.holding => {
                    self.holding = true;
                    Some(TouchpadGesture::HoldBegan(2))
                }
                None => None,
                Some(cancelled) => std::mem::take(&mut self.holding)
                    .then_some(TouchpadGesture::HoldEnded(cancelled)),
            };
        }
        if let Some(cancelled) = ended {
            self.panned = Vec2::ZERO;
            return std::mem::take(&mut self.pinching)
                .then_some(TouchpadGesture::PinchEnded(cancelled));
        }
        if !self.pinching {
            self.pinching = true;
            self.scale = 1.0;
            self.panned = Vec2::ZERO;
            return Some(TouchpadGesture::PinchBegan(2));
        }
        let rotation = match gesture {
            Gesture::Pan(delta) => {
                self.panned += delta;
                return None;
            }
            Gesture::Pinch(delta) if delta.is_finite() => {
                self.scale = (self.scale + delta as f32).max(0.0);
                0.0
            }
            Gesture::Rotation(delta) if delta != 0.0 && delta.is_finite() => -delta,
            _ => return None,
        };
        Some(TouchpadGesture::Pinched {
            delta: std::mem::take(&mut self.panned),
            scale: self.scale,
            rotation,
        })
    }
}
