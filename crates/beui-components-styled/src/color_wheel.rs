use std::cell::RefCell;
use std::rc::Rc;

use beui_macros::{component, view};

use crate::color_picker::thumb;
use crate::theme::{BORDER_WIDTH, use_theme};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{ColorPickerState, ColorWheelHandle, WheelGeometry, WheelPoint};
use beui_core::color::{Color32, Hsva, Oklch, gamma_from_linear};
use beui_core::geometry::{Pos2, Rect, Vec2, pos2};
use beui_core::image::Image;
use beui_core::node::NodeId;
use beui_core::painter::Painter;
use beui_view::reactive::{Callback, Draw, Drawing, Frame, Prop, clone, create_memo, focus_ring};

pub const WHEEL_SIZE: f32 = 200.0;
const RING_WIDTH: f32 = 22.0;
const RING_GAP: f32 = 4.0;
const THUMB_RADIUS: f32 = 7.0;
const GRAB_SLOP: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 2.0;
const RING_HUES: usize = 720;
const LIGHTNESS_STEPS: usize = 256;
const OUTLINE_DARK: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 110);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Model {
    Hsv,
    Oklch,
}

#[component]
pub fn ColorWheel(
    value: Prop<Color32>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let picker: ColorPickerState<Hsva> =
        ColorPickerState::new(value, disabled, on_change, on_preview);
    let color = picker.color();
    let point = create_memo(clone!(color -> move || WheelPoint::of_hsva(color.get())));
    let (change, drag) = (picker.clone(), picker.clone());
    view! {
        <Frame width=WHEEL_SIZE height=WHEEL_SIZE>
            <unstyled::ColorWheel
                value={point}
                ring_width=RING_WIDTH
                thumb={THUMB_RADIUS * 2.0 + GRAB_SLOP}
                focused
                disabled={picker.disabled()}
                on_change={move |next: WheelPoint| {
                    let held = color.get_untracked();
                    change.apply(next.to_hsva(held.alpha));
                }}
                on_drag_change={move |dragging: bool| drag.drag(dragging)}
            >
                {|handle: ColorWheelHandle| view! {
                    <WheelFace handle model=Model::Hsv />
                }}
            </unstyled::ColorWheel>
        </Frame>
    }
}

#[component]
pub fn OklchColorWheel(
    value: Prop<Color32>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let picker: ColorPickerState<Oklch> =
        ColorPickerState::new(value, disabled, on_change, on_preview);
    let color = picker.color();
    let point = create_memo(clone!(color -> move || WheelPoint::of_oklch(color.get())));
    let (change, drag) = (picker.clone(), picker.clone());
    let accessibility = create_memo(clone!(color -> move || {
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
    }));
    view! {
        <Frame width=WHEEL_SIZE height=WHEEL_SIZE>
            <unstyled::ColorWheel
                value={point}
                ring_width=RING_WIDTH
                thumb={THUMB_RADIUS * 2.0 + GRAB_SLOP}
                focused
                disabled={picker.disabled()}
                accessibility
                on_change={move |next: WheelPoint| {
                    let held = color.get_untracked();
                    change.apply(next.to_oklch(held.alpha));
                }}
                on_drag_change={move |dragging: bool| drag.drag(dragging)}
            >
                {|handle: ColorWheelHandle| view! {
                    <WheelFace handle model=Model::Oklch />
                }}
            </unstyled::ColorWheel>
        </Frame>
    }
}

#[component]
fn WheelFace(handle: ColorWheelHandle, model: Model) -> NodeId {
    let ColorWheelHandle { point, focused, .. } = handle;
    let hues = Rc::new(ring_colors(model));
    let ring = Rc::new(RefCell::new(None::<(u32, Image)>));
    let triangle = Rc::new(RefCell::new(None::<(u32, u32, Image)>));
    let draw = Prop::Dynamic(Rc::new(move || {
        let point = point.get();
        let (hues, ring, triangle) = (hues.clone(), ring.clone(), triangle.clone());
        Rc::new(move |painter: &Painter, rect: Rect| {
            let wheel = WheelGeometry::new(rect, RING_WIDTH);
            if wheel.outer <= 0.0 {
                return;
            }
            let pixels =
                ((wheel.outer * 2.0 * painter.ctx().pixels_per_point()).round() as u32).max(1);
            let scale = pixels as f32 / (wheel.outer * 2.0);
            let local = WheelGeometry {
                centre: pos2(pixels as f32 / 2.0, pixels as f32 / 2.0),
                outer: wheel.outer * scale,
                inner: wheel.inner * scale,
            };
            let bounds = Rect::from_center_size(
                wheel.centre,
                Vec2::new(wheel.outer * 2.0, wheel.outer * 2.0),
            );
            let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let ring_image = cached(&ring, pixels, || {
                ring_image(&hues, local, RING_GAP * scale, pixels)
            });
            painter.image(bounds, whole, &ring_image, Color32::WHITE, 0.0, true);
            let rim = |radius: f32| {
                painter.rect_stroke(
                    Rect::from_center_size(wheel.centre, Vec2::new(radius * 2.0, radius * 2.0)),
                    radius,
                    BORDER_WIDTH,
                    OUTLINE_DARK,
                );
            };
            rim(wheel.outer);
            rim(wheel.inner + RING_GAP);
            let triangle_image = {
                let mut held = triangle.borrow_mut();
                match held.as_ref() {
                    Some((size, hue, image)) if *size == pixels && *hue == point.hue.to_bits() => {
                        image.clone()
                    }
                    _ => {
                        let image = triangle_image(model, local, point.hue, pixels);
                        *held = Some((pixels, point.hue.to_bits(), image.clone()));
                        image
                    }
                }
            };
            painter.image(bounds, whole, &triangle_image, Color32::WHITE, 0.0, true);
            let hue_fill = hues[hue_index(point.hue)];
            thumb(
                painter,
                wheel.ring_point(point.hue),
                THUMB_RADIUS,
                opaque(hue_fill),
            );
            thumb(
                painter,
                wheel.point(point),
                THUMB_RADIUS,
                color_at(model, point),
            );
        }) as Draw
    }));
    let theme = use_theme();
    let ring_radius = (WHEEL_SIZE / 2.0) as u8 + 2;
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=ring_radius
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focus_ring(focused)}
        >
            <Frame width=WHEEL_SIZE height=WHEEL_SIZE>
                <Drawing draw />
            </Frame>
        </Frame>
    }
}

fn cached(
    held: &RefCell<Option<(u32, Image)>>,
    pixels: u32,
    build: impl FnOnce() -> Image,
) -> Image {
    let mut held = held.borrow_mut();
    match held.as_ref() {
        Some((size, image)) if *size == pixels => image.clone(),
        _ => {
            let image = build();
            *held = Some((pixels, image.clone()));
            image
        }
    }
}

fn opaque([red, green, blue]: [u8; 3]) -> Color32 {
    Color32::from_rgb(red, green, blue)
}

fn color_at(model: Model, point: WheelPoint) -> Color32 {
    match model {
        Model::Hsv => point.to_hsva(1.0).to_color(),
        Model::Oklch => point.to_oklch(1.0).to_color(),
    }
}

fn hue_index(hue: f32) -> usize {
    ((hue.rem_euclid(360.0) / 360.0 * RING_HUES as f32).round() as usize) % RING_HUES
}

fn ring_colors(model: Model) -> Vec<[u8; 3]> {
    (0..RING_HUES)
        .map(|index| {
            let hue = index as f32 * 360.0 / RING_HUES as f32;
            let color = color_at(model, WheelPoint::new(hue, 1.0, 1.0));
            let [red, green, blue, _] = color.to_array();
            [red, green, blue]
        })
        .collect()
}

fn coverage(depth: f32) -> f32 {
    (depth + 0.5).clamp(0.0, 1.0)
}

fn pixel_centre(index: usize, pixels: u32) -> Pos2 {
    let (x, y) = (index as u32 % pixels, index as u32 / pixels);
    pos2(x as f32 + 0.5, y as f32 + 0.5)
}

fn ring_image(hues: &[[u8; 3]], wheel: WheelGeometry, gap: f32, pixels: u32) -> Image {
    let inner = wheel.inner + gap;
    let pixels_out = (0..(pixels * pixels) as usize)
        .flat_map(|index| {
            let pos = pixel_centre(index, pixels);
            let distance = (pos - wheel.centre).length();
            let alpha = coverage((distance - inner).min(wheel.outer - distance));
            let [red, green, blue] = hues[hue_index(wheel.hue_at(pos))];
            [red, green, blue, (alpha * 255.0).round() as u8]
        })
        .collect();
    Image::from_rgba(pixels, pixels, pixels_out)
}

fn triangle_image(model: Model, wheel: WheelGeometry, hue: f32, pixels: u32) -> Image {
    let limits: Vec<f32> = match model {
        Model::Hsv => Vec::new(),
        Model::Oklch => (0..=LIGHTNESS_STEPS)
            .map(|step| Oklch::max_chroma(step as f32 / LIGHTNESS_STEPS as f32, hue))
            .collect(),
    };
    let shade = |point: WheelPoint| -> [u8; 3] {
        match model {
            Model::Hsv => {
                let [red, green, blue, _] = point.to_hsva(1.0).to_color().to_array();
                [red, green, blue]
            }
            Model::Oklch => {
                let color = point.to_oklch(1.0);
                let step = color.lightness * LIGHTNESS_STEPS as f32;
                let below = (step.floor() as usize).min(LIGHTNESS_STEPS);
                let above = (below + 1).min(LIGHTNESS_STEPS);
                let blend = step - below as f32;
                let limit = limits[below] + (limits[above] - limits[below]) * blend;
                let [red, green, blue] = Oklch {
                    chroma: color.chroma.min(limit),
                    ..color
                }
                .linear_rgb();
                [
                    gamma_from_linear(red),
                    gamma_from_linear(green),
                    gamma_from_linear(blue),
                ]
            }
        }
    };
    let pixels_out = (0..(pixels * pixels) as usize)
        .flat_map(|index| {
            let pos = pixel_centre(index, pixels);
            let alpha = coverage(wheel.distance_inside_triangle(hue, pos));
            if alpha <= 0.0 {
                return [0, 0, 0, 0];
            }
            let (saturation, value) = wheel.triangle_at(hue, pos).unwrap_or((0.0, 0.0));
            let [red, green, blue] = shade(WheelPoint::new(hue, saturation, value));
            [red, green, blue, (alpha * 255.0).round() as u8]
        })
        .collect();
    Image::from_rgba(pixels, pixels, pixels_out)
}
