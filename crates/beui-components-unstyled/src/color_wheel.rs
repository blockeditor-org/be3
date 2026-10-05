use std::cell::Cell;
use std::f32::consts::TAU;
use std::rc::Rc;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_core::color::{Hsva, Oklch};
use beui_core::document::Document;
use beui_core::geometry::{Pos2, Rect, Vec2, vec2};
use beui_core::input::{CursorIcon, Key, KeyPress, PointerPress};
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, Interactive, Memo, Prop, ReadSignal, Render, clone, component_accessibility,
    component_rect, create_effect, create_memo, create_signal, set_component_state,
};

const STEP: f32 = 0.01;
const PAGE: f32 = 0.1;
const HUE_STEP: f32 = 1.0;
const HUE_PAGE: f32 = 15.0;
const DEGENERATE: f32 = 1e-6;
pub const OKLCH_TIP_CHROMA: f32 = 0.38;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct WheelPoint {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
}

impl WheelPoint {
    pub fn new(hue: f32, saturation: f32, value: f32) -> Self {
        Self {
            hue: hue.rem_euclid(360.0),
            saturation: saturation.clamp(0.0, 1.0),
            value: value.clamp(0.0, 1.0),
        }
    }

    pub fn of_hsva(color: Hsva) -> Self {
        Self::new(color.hue, color.saturation, color.value)
    }

    pub fn to_hsva(self, alpha: f32) -> Hsva {
        Hsva::new(self.hue, self.saturation, self.value, alpha)
    }

    pub fn of_oklch(color: Oklch) -> Self {
        OklchTriangle::new(color.hue).point(color)
    }

    pub fn to_oklch(self, alpha: f32) -> Oklch {
        OklchTriangle::new(self.hue).color(self, alpha)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct OklchTriangle {
    pub hue: f32,
    pub tip_lightness: f32,
}

impl OklchTriangle {
    pub fn new(hue: f32) -> Self {
        Self {
            hue,
            tip_lightness: Oklch::cusp(hue).lightness,
        }
    }

    pub fn point(self, color: Oklch) -> WheelPoint {
        let tip = color.chroma / OKLCH_TIP_CHROMA;
        let value = color.lightness + tip * (1.0 - self.tip_lightness);
        let saturation = match value < DEGENERATE {
            true => 0.0,
            false => tip / value,
        };
        WheelPoint::new(self.hue, saturation, value)
    }

    pub fn color(self, point: WheelPoint, alpha: f32) -> Oklch {
        let tip = point.value * point.saturation;
        Oklch::new(
            point.value - tip * (1.0 - self.tip_lightness),
            tip * OKLCH_TIP_CHROMA,
            self.hue,
            alpha,
        )
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct WheelGeometry {
    pub centre: Pos2,
    pub outer: f32,
    pub inner: f32,
}

impl WheelGeometry {
    pub fn new(rect: Rect, ring_width: f32) -> Self {
        let outer = rect.width().min(rect.height()) / 2.0;
        Self {
            centre: rect.center(),
            outer,
            inner: (outer - ring_width).max(0.0),
        }
    }

    pub fn direction(hue: f32) -> Vec2 {
        let (sine, cosine) = hue.to_radians().sin_cos();
        vec2(cosine, -sine)
    }

    pub fn hue_at(&self, pos: Pos2) -> f32 {
        let offset = pos - self.centre;
        (-offset.y).atan2(offset.x).rem_euclid(TAU).to_degrees() % 360.0
    }

    pub fn on_ring(&self, pos: Pos2) -> bool {
        (pos - self.centre).length() >= self.inner
    }

    pub fn ring_point(&self, hue: f32) -> Pos2 {
        self.centre + Self::direction(hue) * ((self.outer + self.inner) / 2.0)
    }

    pub fn corners(&self, hue: f32) -> [Pos2; 3] {
        let corner = |turn: f32| self.centre + Self::direction(hue + turn) * self.inner;
        [corner(0.0), corner(120.0), corner(240.0)]
    }

    pub fn point(&self, point: WheelPoint) -> Pos2 {
        let [tip, white, black] = self.corners(point.hue);
        let top = white + (tip - white) * point.saturation;
        black + (top - black) * point.value
    }

    pub fn triangle_at(&self, hue: f32, pos: Pos2) -> Option<(f32, f32)> {
        let [tip, white, black] = self.corners(hue);
        let (to_tip, to_white, to_pos) = (tip - black, white - black, pos - black);
        let area = to_tip.x * to_white.y - to_tip.y * to_white.x;
        if area.abs() < DEGENERATE {
            return None;
        }
        let tip_weight = (to_pos.x * to_white.y - to_pos.y * to_white.x) / area;
        let white_weight = (to_tip.x * to_pos.y - to_tip.y * to_pos.x) / area;
        let lit = tip_weight + white_weight;
        let saturation = match lit.abs() < DEGENERATE {
            true => None,
            false => Some((tip_weight / lit).clamp(0.0, 1.0)),
        };
        saturation.map(|saturation| (saturation, lit.clamp(0.0, 1.0)))
    }

    pub fn distance_inside_triangle(&self, hue: f32, pos: Pos2) -> f32 {
        let [tip, white, black] = self.corners(hue);
        [(tip, white), (white, black), (black, tip)]
            .into_iter()
            .map(|(from, to)| {
                let edge = to - from;
                let length = edge.length().max(DEGENERATE);
                (edge.y * (pos.x - from.x) - edge.x * (pos.y - from.y)) / length
            })
            .fold(f32::INFINITY, f32::min)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Ring,
    Triangle,
}

pub struct ColorWheelHandle {
    pub point: ReadSignal<WheelPoint>,
    pub dragging: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[component]
pub fn ColorWheel(
    value: Prop<WheelPoint>,
    ring_width: f32,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<ColorWheelHandle>,
    on_change: Callback<WheelPoint>,
    on_drag_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let (point, set_point) = create_signal(value.peek());
    create_effect(clone!(set_point -> move || set_point.set(value.get())));
    let (dragging, set_dragging) = create_signal(false);
    let (hovered, set_hovered) = create_signal(false);
    let (has_focus, set_has_focus) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    set_component_state(point.clone());

    match accessibility {
        Some(accessibility) => component_accessibility(accessibility),
        None => component_accessibility(create_memo(clone!(point disabled -> move || {
            let point = point.get();
            let mut node = Node::new(Role::Slider);
            let label = label.get();
            node.set_label(if label.is_empty() { "Color wheel".to_owned() } else { label });
            node.set_numeric_value(f64::from(point.hue.round()));
            node.set_min_numeric_value(0.0);
            node.set_max_numeric_value(360.0);
            node.set_numeric_value_step(f64::from(HUE_STEP));
            node.set_value(format!(
                "hue {} degrees, saturation {}%, brightness {}%",
                point.hue.round(),
                (point.saturation * 100.0).round(),
                (point.value * 100.0).round()
            ));
            if disabled.get() {
                node.set_disabled();
            }
            node
        }))),
    }

    let set = {
        let point = point.clone();
        let disabled = disabled.clone();
        move |next: WheelPoint| {
            if disabled.get_untracked() {
                return;
            }
            if next != point.get_untracked() {
                set_point.set(next);
                on_change.call(next);
            }
        }
    };
    let (drag_set, key_set, step_set) = (set.clone(), set.clone(), set);
    let placed = component_rect();
    let geometry = move |rect: Rect| WheelGeometry::new(rect, ring_width);
    let over_thumb = {
        let point = point.clone();
        move |rect: Rect, pos: Pos2| {
            let wheel = geometry(rect);
            let point = point.get_untracked();
            [
                (Part::Triangle, wheel.point(point)),
                (Part::Ring, wheel.ring_point(point.hue)),
            ]
            .into_iter()
            .map(|(part, centre)| (part, centre, (centre - pos).length()))
            .filter(|(_, _, distance)| thumb > 0.0 && *distance <= thumb / 2.0)
            .min_by(|first, second| first.2.total_cmp(&second.2))
            .map(|(part, centre, _)| (part, centre))
        }
    };
    let part = Rc::new(Cell::new(Part::Triangle));
    let grab = Rc::new(Cell::new(Vec2::ZERO));
    let grabbed = clone!(part grab placed over_thumb -> move |press: PointerPress| {
        let rect = placed.get_untracked();
        let (pressed, offset) = match over_thumb(rect, press.pos) {
            Some((pressed, centre)) => (pressed, centre - press.pos),
            None => match geometry(rect).on_ring(press.pos) {
                true => (Part::Ring, Vec2::ZERO),
                false => (Part::Triangle, Vec2::ZERO),
            },
        };
        part.set(pressed);
        grab.set(offset);
    });
    let drag_point = point.clone();
    let dragged_to = clone!(placed -> move |press: PointerPress| {
        let wheel = geometry(placed.get_untracked());
        let target = press.pos + grab.get();
        let current = drag_point.get_untracked();
        match part.get() {
            Part::Ring => WheelPoint { hue: wheel.hue_at(target), ..current },
            Part::Triangle => match wheel.triangle_at(current.hue, target) {
                Some((saturation, value)) => WheelPoint { saturation, value, ..current },
                None => WheelPoint { value: 0.0, ..current },
            },
        }
    });
    let (pointer, set_pointer) = create_signal(None::<Pos2>);
    let cursor = create_memo(clone!(point placed dragging hovered -> move || {
        if dragging.get() {
            return CursorIcon::Grabbing;
        }
        let _ = point.get();
        let over = hovered.get()
            && pointer.get().is_some_and(|pos| over_thumb(placed.get(), pos).is_some());
        match over {
            true => CursorIcon::Grab,
            false => CursorIcon::Crosshair,
        }
    }));
    let (key_point, step_point) = (point.clone(), point.clone());
    let content_node = content.call(ColorWheelHandle {
        point,
        dragging: dragging.clone(),
        hovered,
        focused: has_focus,
        disabled: disabled.clone(),
    });
    let tab_stop = create_memo(clone!(disabled -> move || !disabled.get()));
    view! {
        <Interactive
            focusable=true
            tab_stop
            focused
            on_focus_change={move |focused: bool| set_has_focus.set(focused)}
            on_step={move |delta: f32| {
                let point = step_point.get_untracked();
                step_set(WheelPoint::new(point.hue + delta * HUE_STEP, point.saturation, point.value));
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let point = key_point.get_untracked();
                let step = if press.modifiers.shift { PAGE } else { STEP };
                let WheelPoint { hue, saturation, value } = point;
                let next = match press.key {
                    Key::ArrowLeft => WheelPoint::new(hue, saturation - step, value),
                    Key::ArrowRight => WheelPoint::new(hue, saturation + step, value),
                    Key::ArrowUp => WheelPoint::new(hue, saturation, value + step),
                    Key::ArrowDown => WheelPoint::new(hue, saturation, value - step),
                    Key::PageUp => WheelPoint::new(hue + HUE_PAGE, saturation, value),
                    Key::PageDown => WheelPoint::new(hue - HUE_PAGE, saturation, value),
                    Key::Home => WheelPoint::new(hue, 0.0, value),
                    Key::End => WheelPoint::new(hue, 1.0, value),
                    _ => return false,
                };
                if press.pressed {
                    key_set(next);
                }
                true
            }}
            cursor
            touch_drags=true
            on_hover_change={move |hovered: bool| set_hovered.set(hovered)}
            on_hover_move={move |press: PointerPress| set_pointer.set(Some(press.pos))}
            on_press={grabbed}
            on_drag={move |press: PointerPress| drag_set(dragged_to(press))}
            on_active_change={move |active: bool| {
                set_dragging.set(active);
                on_drag_change.call(active);
            }}
            children={content_node}
        />
    }
}

pub fn color_wheel_value(document: &Document, wheel: NodeId) -> WheelPoint {
    document
        .component_state::<ReadSignal<WheelPoint>>(wheel)
        .get_untracked()
}
