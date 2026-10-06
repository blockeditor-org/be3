use std::sync::OnceLock;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::choice::{Choice, ChoiceKind, ChoiceOption, ChoiceOptionHandle};
use crate::color_area::{ColorArea, ColorAreaHandle};
use crate::slider::{Slider, SliderHandle};
use beui_core::base::Direction;
use beui_core::color::{Color32, Hsva, Oklch, format_hex, parse_hex};
use beui_core::geometry::{Rect, pos2};
use beui_core::image::Image;
use beui_core::node::NodeId;
use beui_view::reactive::{
    Callback, ForEach, Memo, Prop, ReadSignal, Render, RenderFn, WriteSignal, clone, create_effect,
    create_memo, create_signal,
};

const HUE_TEXELS: u32 = 96;
const PLANE_TEXELS: u32 = 32;
const STRIP_TEXELS: u32 = 16;

pub trait ColorModel: Copy + PartialEq + Default + 'static {
    fn from_color_keeping(color: Color32, previous: Self) -> Self;
    fn to_color(self) -> Color32;
}

impl ColorModel for Hsva {
    fn from_color_keeping(color: Color32, previous: Self) -> Self {
        Hsva::from_color_keeping(color, previous)
    }

    fn to_color(self) -> Color32 {
        Hsva::to_color(self)
    }
}

impl ColorModel for Oklch {
    fn from_color_keeping(color: Color32, previous: Self) -> Self {
        Oklch::from_color_keeping(color, previous)
    }

    fn to_color(self) -> Color32 {
        Oklch::to_color(self)
    }
}

#[derive(Clone)]
pub struct ColorPickerState<Model: ColorModel = Hsva> {
    color: ReadSignal<Model>,
    set_color: WriteSignal<Model>,
    reported: ReadSignal<Color32>,
    set_reported: WriteSignal<Color32>,
    dragging: ReadSignal<bool>,
    set_dragging: WriteSignal<bool>,
    disabled: Memo<bool>,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
}

impl<Model: ColorModel> ColorPickerState<Model> {
    pub fn new(
        value: Prop<Color32>,
        disabled: Prop<bool>,
        on_change: Callback<Color32>,
        on_preview: Callback<Option<Color32>>,
    ) -> Self {
        let first = value.peek();
        let (color, set_color) = create_signal(Model::from_color_keeping(first, Model::default()));
        let (reported, set_reported) = create_signal(first);
        let (dragging, set_dragging) = create_signal(false);
        create_effect(clone!(color set_color set_reported -> move || {
            let next = value.get();
            set_reported.set(next);
            let held = color.get_untracked();
            set_color.set(Model::from_color_keeping(next, held));
        }));
        Self {
            color,
            set_color,
            reported,
            set_reported,
            dragging,
            set_dragging,
            disabled: create_memo(move || disabled.get()),
            on_change,
            on_preview,
        }
    }

    pub fn color(&self) -> ReadSignal<Model> {
        self.color.clone()
    }

    pub fn shown(&self) -> Memo<Color32> {
        let color = self.color.clone();
        create_memo(move || color.get().to_color())
    }

    pub fn disabled(&self) -> Memo<bool> {
        self.disabled.clone()
    }

    pub fn apply(&self, next: Model) {
        if self.disabled.get_untracked() {
            return;
        }
        self.set_color.set(next);
        let color = next.to_color();
        if self.dragging.get_untracked() {
            self.on_preview.call(Some(color));
        } else {
            self.report(color);
        }
    }

    pub fn drag(&self, dragging: bool) {
        self.set_dragging.set(dragging);
        if !dragging {
            self.report(self.color.get_untracked().to_color());
            self.on_preview.call(None);
        }
    }

    pub fn pick(&self, color: Color32) {
        let held = self.color.get_untracked();
        self.apply(Model::from_color_keeping(color, held));
    }

    fn report(&self, color: Color32) {
        if color != self.reported.get_untracked() {
            self.set_reported.set(color);
            self.on_change.call(color);
        }
    }
}

impl ColorPickerState<Hsva> {
    pub fn set_hue(&self, hue: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::new(
            hue.min(359.9),
            held.saturation,
            held.value,
            held.alpha,
        ));
    }

    pub fn set_alpha(&self, alpha: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva { alpha, ..held });
    }

    pub fn set_channel(&self, index: usize, value: f64) {
        let mut channels = self.color.get_untracked().to_color().to_array();
        channels[index] = value.round().clamp(0.0, 255.0) as u8;
        let [red, green, blue, alpha] = channels;
        self.pick(Color32::from_rgba_unmultiplied(red, green, blue, alpha));
    }

    pub fn set_saturation(&self, saturation: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::new(held.hue, saturation, held.value, held.alpha));
    }

    pub fn set_value(&self, value: f32) {
        let held = self.color.get_untracked();
        self.apply(Hsva::new(held.hue, held.saturation, value, held.alpha));
    }

    pub fn channel(&self, index: usize) -> Memo<f64> {
        let color = self.color.clone();
        create_memo(move || f64::from(color.get().to_color().to_array()[index]))
    }

    pub fn opacity_percent(&self) -> Memo<f64> {
        let color = self.color.clone();
        create_memo(move || (f64::from(color.get().to_color().alpha()) * 100.0 / 255.0).round())
    }

    pub fn hue_degrees(&self) -> Memo<f64> {
        let color = self.color.clone();
        create_memo(move || f64::from(color.get().hue.round()))
    }

    pub fn saturation_percent(&self) -> Memo<f64> {
        let color = self.color.clone();
        create_memo(move || f64::from((color.get().saturation * 100.0).round()))
    }

    pub fn value_percent(&self) -> Memo<f64> {
        let color = self.color.clone();
        create_memo(move || f64::from((color.get().value * 100.0).round()))
    }

    pub fn set_opacity_percent(&self, percent: f64) {
        self.set_channel(3, percent * 255.0 / 100.0);
    }

    pub fn set_saturation_percent(&self, percent: f64) {
        self.set_saturation(percent as f32 / 100.0);
    }

    pub fn set_value_percent(&self, percent: f64) {
        self.set_value(percent as f32 / 100.0);
    }

    pub fn hue_accessibility(&self) -> Memo<Node> {
        let color = self.color.clone();
        create_memo(move || {
            let mut node = Node::new(Role::Slider);
            node.set_label("Hue");
            node.set_value(format!("{} degrees", color.get().hue.round()));
            node
        })
    }

    pub fn alpha_accessibility(&self) -> Memo<Node> {
        let color = self.color.clone();
        create_memo(move || {
            let mut node = Node::new(Role::Slider);
            node.set_label("Opacity");
            node.set_value(format!("{}%", (color.get().alpha * 100.0).round()));
            node
        })
    }
}

#[derive(Clone)]
pub struct HexText {
    text: ReadSignal<String>,
    set_text: WriteSignal<String>,
    shown: Memo<Color32>,
    alpha: bool,
    apply: Callback<Color32>,
}

impl HexText {
    pub fn new(shown: Memo<Color32>, alpha: bool, apply: Callback<Color32>) -> Self {
        let (text, set_text) = create_signal(hex_text(shown.get_untracked(), alpha));
        create_effect(clone!(shown text set_text -> move || {
            let next = shown.get();
            if parse_typed(&text.get_untracked(), next) != Some(next) {
                set_text.set(hex_text(next, alpha));
            }
        }));
        Self {
            text,
            set_text,
            shown,
            alpha,
            apply,
        }
    }

    pub fn text(&self) -> ReadSignal<String> {
        self.text.clone()
    }

    pub fn placeholder(&self) -> &'static str {
        match self.alpha {
            true => "#RRGGBBAA",
            false => "#RRGGBB",
        }
    }

    pub fn edit(&self, typed: String) {
        self.set_text.set(typed.clone());
        if let Some(parsed) = parse_typed(&typed, self.shown.get_untracked()) {
            self.apply.call(parsed);
        }
    }

    pub fn submit(&self, typed: String) {
        match parse_hex(&typed) {
            Some(parsed) => {
                let held = self.shown.get_untracked();
                self.apply.call(keep_alpha(&typed, parsed, held));
            }
            None => self
                .set_text
                .set(hex_text(self.shown.get_untracked(), self.alpha)),
        }
    }
}

fn hex_text(color: Color32, alpha: bool) -> String {
    format_hex(color, alpha && color.alpha() < u8::MAX)
}

fn parse_typed(text: &str, held: Color32) -> Option<Color32> {
    let digits = text.trim().trim_start_matches('#');
    if digits.len() != 6 && digits.len() != 8 {
        return None;
    }
    parse_hex(text).map(|parsed| keep_alpha(text, parsed, held))
}

fn keep_alpha(text: &str, parsed: Color32, held: Color32) -> Color32 {
    let digits = text.trim().trim_start_matches('#').len();
    match digits {
        3 | 6 => {
            let [red, green, blue, _] = parsed.to_array();
            Color32::from_rgba_unmultiplied(red, green, blue, held.alpha())
        }
        _ => parsed,
    }
}

#[component]
pub fn ColorPickerArea(
    picker: ColorPickerState,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<ColorAreaHandle>,
) -> NodeId {
    let (changed, dragged) = (picker.clone(), picker.clone());
    view! {
        <ColorArea
            value={picker.color()}
            thumb
            focused
            disabled={picker.disabled()}
            on_change={move |next: Hsva| changed.apply(next)}
            on_drag_change={move |dragging: bool| dragged.drag(dragging)}
            content
        />
    }
}

#[component]
pub fn HueSlider(
    picker: ColorPickerState,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<SliderHandle>,
) -> NodeId {
    let color = picker.color();
    let hue = create_memo(move || color.get().hue);
    let (changed, dragged) = (picker.clone(), picker.clone());
    view! {
        <Slider
            value={hue}
            min=0.0
            max=360.0
            thumb
            disabled={picker.disabled()}
            accessibility={picker.hue_accessibility()}
            on_change={move |hue: f32| changed.set_hue(hue)}
            on_drag_change={move |dragging: bool| dragged.drag(dragging)}
            content
        />
    }
}

#[component]
pub fn AlphaSlider(
    picker: ColorPickerState,
    #[prop(default = 0.0)] thumb: f32,
    #[prop(children)] content: Render<SliderHandle>,
) -> NodeId {
    let color = picker.color();
    let alpha = create_memo(move || color.get().alpha);
    let (changed, dragged) = (picker.clone(), picker.clone());
    view! {
        <Slider
            value={alpha}
            thumb
            disabled={picker.disabled()}
            accessibility={picker.alpha_accessibility()}
            on_change={move |alpha: f32| changed.set_alpha(alpha)}
            on_drag_change={move |dragging: bool| dragged.drag(dragging)}
            content
        />
    }
}

pub struct SwatchHandle {
    pub color: Color32,
    pub option: ChoiceOptionHandle,
}

#[component]
pub fn Swatches(
    picker: ColorPickerState,
    swatches: Vec<Color32>,
    #[prop(children)] content: RenderFn<SwatchHandle>,
) -> NodeId {
    let shown = picker.shown();
    let held = swatches.clone();
    let chosen = create_memo(move || held.iter().position(|swatch| *swatch == shown.get()));
    let labels: Vec<String> = swatches
        .iter()
        .map(|swatch| format_hex(*swatch, false))
        .collect();
    let indices: Vec<usize> = (0..swatches.len()).collect();
    let picked = swatches.clone();
    view! {
        <Choice
            options={view! {
                <ForEach keys={indices}>
                    {move |index: usize| view! {
                        <ChoiceOption label={labels[index].clone()} />
                    }}
                </ForEach>
            }}
            selected={chosen}
            kind=ChoiceKind::Radio
            direction=Direction::Horizontal
            wrap=true
            on_change={move |index: Option<usize>| {
                if let Some(swatch) = index.and_then(|index| picked.get(index)) {
                    picker.pick(*swatch);
                }
            }}
        >
            {move |option: ChoiceOptionHandle| {
                let color = swatches[option.index];
                content.call(SwatchHandle { color, option })
            }}
        </Choice>
    }
}

pub fn texel_centres(width: u32, height: u32) -> Rect {
    let inset = |texels: u32| 0.5 / texels as f32;
    Rect::from_min_max(
        pos2(inset(width), inset(height)),
        pos2(1.0 - inset(width), 1.0 - inset(height)),
    )
}

pub fn hue_image() -> Image {
    static HUE: OnceLock<Image> = OnceLock::new();
    HUE.get_or_init(|| {
        let pixels = (0..HUE_TEXELS)
            .flat_map(|texel| {
                let hue = 360.0 * texel as f32 / (HUE_TEXELS - 1) as f32;
                Hsva::new(hue.min(359.999), 1.0, 1.0, 1.0)
                    .to_color()
                    .to_array()
            })
            .collect();
        Image::from_rgba(HUE_TEXELS, 1, pixels)
    })
    .clone()
}

pub fn plane_image(hue: f32) -> Image {
    let last = (PLANE_TEXELS - 1) as f32;
    let pixels = (0..PLANE_TEXELS)
        .flat_map(|row| {
            (0..PLANE_TEXELS).flat_map(move |column| {
                Hsva::new(hue, column as f32 / last, 1.0 - row as f32 / last, 1.0)
                    .to_color()
                    .to_array()
            })
        })
        .collect();
    Image::from_rgba(PLANE_TEXELS, PLANE_TEXELS, pixels)
}

pub fn alpha_image(color: Color32) -> Image {
    let [red, green, blue, _] = color.to_array();
    let last = (STRIP_TEXELS - 1) as f32;
    let pixels = (0..STRIP_TEXELS)
        .flat_map(|texel| {
            let alpha = (255.0 * texel as f32 / last).round() as u8;
            [red, green, blue, alpha]
        })
        .collect();
    Image::from_rgba(STRIP_TEXELS, 1, pixels)
}
