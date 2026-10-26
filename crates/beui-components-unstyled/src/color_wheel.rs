use std::f32::consts::TAU;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_view::types::{Hsva, Oklch};
use beui_view::types::{Pos2, Rect, Vec2, vec2};
use beui_view::types::{CursorIcon, Key, KeyPress, PointerPress};
use beui_view::reactive::NodeId;
use beui_view::primitives::*;
use beui_view::reactive::*;

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
        let (to_tip, to_white) = (tip - black, white - black);
        let area = to_tip.x * to_white.y - to_tip.y * to_white.x;
        if area.abs() < DEGENERATE {
            return None;
        }
        let weights = |pos: Pos2| {
            let to_pos = pos - black;
            (
                (to_pos.x * to_white.y - to_pos.y * to_white.x) / area,
                (to_tip.x * to_pos.y - to_tip.y * to_pos.x) / area,
            )
        };
        let (mut tip_weight, mut white_weight) = weights(pos);
        if tip_weight < 0.0 || white_weight < 0.0 || tip_weight + white_weight > 1.0 {
            let nearest = [(tip, white), (white, black), (black, tip)]
                .into_iter()
                .map(|(from, to)| nearest_on_segment(from, to, pos))
                .min_by(|first, second| {
                    (*first - pos).length().total_cmp(&(*second - pos).length())
                })
                .unwrap_or(pos);
            (tip_weight, white_weight) = weights(nearest);
        }
        let lit = (tip_weight + white_weight).clamp(0.0, 1.0);
        match lit < DEGENERATE {
            true => None,
            false => Some(((tip_weight.max(0.0) / lit).clamp(0.0, 1.0), lit)),
        }
    }
}

fn nearest_on_segment(from: Pos2, to: Pos2, pos: Pos2) -> Pos2 {
    let edge = to - from;
    let length = edge.dot(edge);
    if length < DEGENERATE {
        return from;
    }
    from + edge * ((pos - from).dot(edge) / length).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Part {
    Ring,
    Triangle,
}

pub struct ColorWheelHandle {
    pub point: Binding<WheelPoint>,
    pub dragging: ReadSignal<bool>,
    pub hovered: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
    pub disabled: Memo<bool>,
}

#[component]
pub fn ColorWheel(
    #[prop(default = ComponentRef::new())] api: ComponentRef<ColorWheelApi>,
    value: Binding<WheelPoint>,
    ring_width: f32,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = String::new())] label: Prop<String>,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<ColorWheelHandle>,
    on_drag_change: Callback<bool>,
    accessibility: Option<Prop<Node>>,
) -> NodeId {
    let point = value;
    let dragging = create_signal(false);
    let hovered = create_signal(false);
    let has_focus = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    api.fill(ColorWheelApi(point));

    match accessibility {
        Some(accessibility) => component_accessibility(accessibility),
        None => component_accessibility(create_memo(move || {
            let point = point.get();
            let mut node = Node::new(Role::Slider);
            let label = label.get();
            node.set_label(if label.is_empty() {
                "Color wheel".to_owned()
            } else {
                label
            });
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
        })),
    }

    let set = move |next: WheelPoint| {
        if disabled.get_untracked() {
            return;
        }
        if next != point.get_untracked() {
            point.set(next);
        }
    };
    let geometry =
        move |size: Vec2| WheelGeometry::new(Rect::from_min_size(Pos2::ZERO, size), ring_width);
    let over_thumb = move |press: PointerPress| {
        let (wheel, pos) = (geometry(press.size), press.local);
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
    };
    let part = StoredCell::new(Part::Triangle);
    let grab = StoredCell::new(Vec2::ZERO);
    let grabbed = move |press: PointerPress| {
        let (pressed, offset) = match over_thumb(press) {
            Some((pressed, centre)) => (pressed, centre - press.local),
            None => match geometry(press.size).on_ring(press.local) {
                true => (Part::Ring, Vec2::ZERO),
                false => (Part::Triangle, Vec2::ZERO),
            },
        };
        part.set(pressed);
        grab.set(offset);
    };
    let dragged_to = move |press: PointerPress| {
        let wheel = geometry(press.size);
        let target = press.local + grab.get();
        let current = point.get_untracked();
        match part.get() {
            Part::Ring => WheelPoint {
                hue: wheel.hue_at(target),
                ..current
            },
            Part::Triangle => match wheel.triangle_at(current.hue, target) {
                Some((saturation, value)) => WheelPoint {
                    saturation,
                    value,
                    ..current
                },
                None => WheelPoint {
                    value: 0.0,
                    ..current
                },
            },
        }
    };
    let pointer = create_signal(None::<PointerPress>);
    let cursor = create_memo(move || {
        if dragging.get() {
            return CursorIcon::Grabbing;
        }
        let _ = point.get();
        let over = hovered.get()
            && pointer
                .get()
                .is_some_and(|press| over_thumb(press).is_some());
        match over {
            true => CursorIcon::Grab,
            false => CursorIcon::Crosshair,
        }
    });
    let content_node = content.call(ColorWheelHandle {
        point,
        dragging: dragging.read_only(),
        hovered: hovered.read_only(),
        focused: has_focus.read_only(),
        disabled,
    });
    let tab_stop = create_memo(move || !disabled.get());
    view! {
        <Interactive
            focusable=true
            tab_stop
            focused
            on_focus_change={has_focus}
            on_step={move |delta: f32| {
                let point = point.get_untracked();
                set(WheelPoint::new(point.hue + delta * HUE_STEP, point.saturation, point.value));
            }}
            on_key={move |press: KeyPress| {
                if press.modifiers.ctrl || press.modifiers.alt {
                    return false;
                }
                let point = point.get_untracked();
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
                    set(next);
                }
                true
            }}
            cursor
            touch_drags=true
            on_hover_change={hovered}
            on_hover_move={move |press: PointerPress| pointer.set(Some(press))}
            on_press={grabbed}
            on_drag={move |press: PointerPress| set(dragged_to(press))}
            on_active_change={move |active: bool| {
                dragging.set(active);
                on_drag_change.call(active);
            }}
            children={content_node}
        />
    }
}

#[derive(Clone, Copy)]
pub struct ColorWheelApi(Binding<WheelPoint>);

impl ColorWheelApi {
    pub fn value(&self) -> WheelPoint {
        self.0.get_untracked()
    }
}
