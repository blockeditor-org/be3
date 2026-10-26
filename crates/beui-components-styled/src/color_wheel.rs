use crate::focus_ring::FocusRing;
use crate::frame::{Frame, Radius};
use crate::tokens::{Size, Stroke};
use std::cell::RefCell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::color_picker::thumb;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ColorPickerState, ColorWheelHandle, WheelGeometry, WheelPoint};
use beui_core::color::{Color32, Hsva, Oklch};
use beui_core::geometry::{Rect, Vec2};
use beui_core::node::NodeId;
use beui_core::painter::Painter;
use beui_view::primitives::Drawing;
use beui_view::reactive::*;

pub const WHEEL_SIZE: f32 = 200.0;
const RING_WIDTH: f32 = 22.0;
const RING_GAP: f32 = 4.0;
const THUMB_RADIUS: f32 = 7.0;
const GRAB_SLOP: f32 = 10.0;
const WHEEL: Size = Size::Custom(WHEEL_SIZE);
const GAMUT_LINE_WIDTH: f32 = 1.25;
const OUTLINE_DARK: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 110);

#[cfg(feature = "gpu")]
mod gpu;

#[cfg(feature = "gpu")]
use gpu::Slot;

#[derive(Clone, Default)]
struct Drawn {
    slot: Slot,
    held: Rc<RefCell<Option<(Face, beui_core::drawing::Drawing)>>>,
}

impl Drawn {
    fn drawing(&self, face: Face) -> beui_core::drawing::Drawing {
        let mut held = self.held.borrow_mut();
        match held.as_ref() {
            Some((drawn, drawing)) if *drawn == face => drawing.clone(),
            _ => {
                let drawing = self.slot.drawing(face);
                *held = Some((face, drawing.clone()));
                drawing
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Model {
    Hsv,
    Oklch,
}

#[derive(Clone, Copy, PartialEq)]
struct Face {
    model: Model,
    hue: f32,
    ring_width: f32,
    ring_gap: f32,
    gamut_line: f32,
}

#[cfg(not(feature = "gpu"))]
#[derive(Clone, Default)]
struct Slot;

#[cfg(not(feature = "gpu"))]
impl Slot {
    fn drawing(&self, _face: Face) -> beui_core::drawing::Drawing {
        beui_core::drawing::Drawing::erased(())
    }
}

#[component]
pub fn ColorWheel(
    value: Binding<Color32>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let picker: ColorPickerState<Hsva> = ColorPickerState::new(value, disabled, on_preview);
    let color = picker.color();
    let point = create_memo(move || WheelPoint::of_hsva(color.get()));
    view! {
        <Frame width=WHEEL height=WHEEL>
            <unstyled::ColorWheel
                value={Binding::new(point, move |next: WheelPoint| {
                    let held = color.get_untracked();
                    picker.apply(next.to_hsva(held.alpha));
                })}
                ring_width=RING_WIDTH
                thumb={THUMB_RADIUS * 2.0 + GRAB_SLOP}
                focused
                disabled={picker.disabled()}
                on_drag_change={move |dragging: bool| picker.drag(dragging)}
            >|handle: ColorWheelHandle|
                <WheelFace handle model=Model::Hsv />
            </unstyled::ColorWheel>
        </Frame>
    }
}

#[component]
pub fn OklchColorWheel(
    value: Binding<Color32>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let picker: ColorPickerState<Oklch> = ColorPickerState::new(value, disabled, on_preview);
    let color = picker.color();
    let point = create_memo(move || WheelPoint::of_oklch(color.get()));
    let accessibility = create_memo(move || {
        let color = color.get();
        let mut node = accesskit::Node::new(accesskit::Role::Slider);
        node.set_label("OKLCH color wheel");
        node.set_numeric_value(f64::from(color.hue.round()));
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(360.0);
        node.set_value(format!(
            "lightness {}%, chroma {:.2}, hue {} degrees",
            (color.lightness * 100.0).round(),
            color.chroma,
            color.hue.round()
        ));
        node
    });
    view! {
        <Frame width=WHEEL height=WHEEL>
            <unstyled::ColorWheel
                value={Binding::new(point, move |next: WheelPoint| {
                    let held = color.get_untracked();
                    let was = WheelPoint::of_oklch(held);
                    let turned = next.hue != was.hue
                        && next.saturation == was.saturation
                        && next.value == was.value;
                    picker.apply(match turned {
                        true => Oklch::new(held.lightness, held.chroma, next.hue, held.alpha),
                        false => next.to_oklch(held.alpha),
                    });
                })}
                ring_width=RING_WIDTH
                thumb={THUMB_RADIUS * 2.0 + GRAB_SLOP}
                focused
                disabled={picker.disabled()}
                accessibility
                on_drag_change={move |dragging: bool| picker.drag(dragging)}
            >|handle: ColorWheelHandle|
                <WheelFace handle model=Model::Oklch />
            </unstyled::ColorWheel>
        </Frame>
    }
}

#[component]
fn WheelFace(handle: ColorWheelHandle, model: Model) -> NodeId {
    let ColorWheelHandle { point, focused, .. } = handle;
    let drawn = Drawn::default();
    let draw = Prop::dynamic(move || {
        let point = point.get();
        let drawn = drawn.clone();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let wheel = WheelGeometry::new(rect, RING_WIDTH);
            if wheel.outer <= 0.0 {
                return;
            }
            let bounds = Rect::from_center_size(
                wheel.centre,
                Vec2::new(wheel.outer * 2.0, wheel.outer * 2.0),
            );
            painter.drawing(
                bounds,
                &drawn.drawing(Face {
                    model,
                    hue: point.hue,
                    ring_width: RING_WIDTH,
                    ring_gap: RING_GAP,
                    gamut_line: GAMUT_LINE_WIDTH,
                }),
            );
            let rim = |radius: f32| {
                painter.rect_stroke(
                    Rect::from_center_size(wheel.centre, Vec2::new(radius * 2.0, radius * 2.0)),
                    radius,
                    Stroke::Hairline.px(),
                    OUTLINE_DARK,
                );
            };
            rim(wheel.outer);
            rim(wheel.inner + RING_GAP);
            thumb(
                painter,
                wheel.ring_point(point.hue),
                THUMB_RADIUS,
                color_at(model, WheelPoint::new(point.hue, 1.0, 1.0)),
            );
            thumb(
                painter,
                wheel.point(point),
                THUMB_RADIUS,
                color_at(model, point),
            );
        }) as Draw
    });
    view! {
        <FocusRing focused radius=Radius::Full offset=Stroke::Focus>
            <Frame width=WHEEL height=WHEEL>
                <Drawing draw />
            </Frame>
        </FocusRing>
    }
}

fn color_at(model: Model, point: WheelPoint) -> Color32 {
    match model {
        Model::Hsv => point.to_hsva(1.0).to_color(),
        Model::Oklch => point.to_oklch(1.0).to_color(),
    }
}
