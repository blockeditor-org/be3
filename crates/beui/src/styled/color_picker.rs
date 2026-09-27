use std::rc::Rc;
use std::sync::OnceLock;

use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::{Align, Direction};
use crate::color::{Color32, Hsva, format_hex, parse_hex};
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::image::Image;
use crate::node::NodeId;
use crate::painter::Painter;
use crate::reactive::{
    Callback, Draw, Drawing, ForEach, Frame, ItemSize, List, Memo, Prop, ReadSignal, Show,
    WriteSignal, clone, create_effect, create_memo, create_signal,
};
use crate::styled::number_input::NumberInput;
use crate::styled::text::Caption;
use crate::styled::text_input::TextInput;
use crate::styled::theme::{BORDER_WIDTH, CHIP_RADIUS, RADIUS, use_theme};
use crate::unstyled;
use crate::unstyled::{
    ChoiceKind, ChoiceOption, ChoiceOptionHandle, ColorAreaHandle, SliderHandle,
};

pub const PICKER_WIDTH: f32 = 244.0;
const AREA_HEIGHT: f32 = 156.0;
const SLIDER_HEIGHT: f32 = 16.0;
const PREVIEW_SIZE: f32 = 40.0;
const SWATCH_SIZE: f32 = 18.0;
const SPACING: f32 = 10.0;
const FOCUS_RING_WIDTH: f32 = 2.0;
const FOCUS_RING_OFFSET: f32 = 2.0;
const THUMB_RADIUS: f32 = 7.0;
const KNOB_RADIUS: f32 = 8.0;
const CHECKER: f32 = 5.0;
const CHECKER_LIGHT: Color32 = Color32::from_gray(236);
const CHECKER_DARK: Color32 = Color32::from_gray(190);
const OUTLINE_DARK: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 110);
const HUE_TEXELS: u32 = 96;
const PLANE_TEXELS: u32 = 32;
const STRIP_TEXELS: u32 = 16;
const ALPHA_WIDTH: f32 = 64.0;

pub const DEFAULT_SWATCHES: [Color32; 10] = [
    Color32::from_rgb(0xE5, 0x48, 0x4D),
    Color32::from_rgb(0xF7, 0x6B, 0x15),
    Color32::from_rgb(0xFF, 0xC5, 0x3D),
    Color32::from_rgb(0x30, 0xA4, 0x6C),
    Color32::from_rgb(0x12, 0xA5, 0x94),
    Color32::from_rgb(0x00, 0x90, 0xFF),
    Color32::from_rgb(0x3E, 0x63, 0xDD),
    Color32::from_rgb(0x8E, 0x4E, 0xC6),
    Color32::from_rgb(0xD6, 0x40, 0x9F),
    Color32::from_rgb(0x6F, 0x6F, 0x6F),
];

#[derive(Clone)]
struct Picker {
    color: ReadSignal<Hsva>,
    set_color: WriteSignal<Hsva>,
    reported: ReadSignal<Color32>,
    set_reported: WriteSignal<Color32>,
    dragging: ReadSignal<bool>,
    set_dragging: WriteSignal<bool>,
    disabled: Memo<bool>,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
}

impl Picker {
    fn apply(&self, next: Hsva) {
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

    fn report(&self, color: Color32) {
        if color != self.reported.get_untracked() {
            self.set_reported.set(color);
            self.on_change.call(color);
        }
    }

    fn drag(&self, dragging: bool) {
        self.set_dragging.set(dragging);
        if !dragging {
            self.report(self.color.get_untracked().to_color());
            self.on_preview.call(None);
        }
    }
}

#[component]
pub fn ColorPicker(
    value: Prop<Color32>,
    #[prop(default = true)] alpha: bool,
    #[prop(default = DEFAULT_SWATCHES.to_vec())] swatches: Vec<Color32>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_change: Callback<Color32>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let initial = value.peek();
    let (color, set_color) = create_signal(Hsva::from_color(initial));
    let (reported, set_reported) = create_signal(initial);
    let (dragging, set_dragging) = create_signal(false);
    let disabled = create_memo(move || disabled.get());
    let picker = Picker {
        color: color.clone(),
        set_color: set_color.clone(),
        reported,
        set_reported: set_reported.clone(),
        dragging,
        set_dragging,
        disabled: disabled.clone(),
        on_change,
        on_preview,
    };
    create_effect(clone!(color -> move || {
        let next = value.get();
        set_reported.set(next);
        let held = color.get_untracked();
        set_color.set(Hsva::from_color_keeping(next, held));
    }));

    let shown = create_memo(clone!(color -> move || color.get().to_color()));
    let hue = create_memo(clone!(color -> move || color.get().hue));
    let opacity = create_memo(clone!(color -> move || color.get().alpha));
    let area = picker.clone();
    let area_drag = picker.clone();
    let hue_picker = picker.clone();
    let hue_drag = picker.clone();
    let alpha_picker = picker.clone();
    let alpha_drag = picker.clone();
    let swatch_picker = picker.clone();
    let hue_accessibility = create_memo(clone!(hue -> move || {
        let mut node = Node::new(Role::Slider);
        node.set_label("Hue");
        node.set_value(format!("{} degrees", hue.get().round()));
        node
    }));
    let alpha_accessibility = create_memo(clone!(opacity -> move || {
        let mut node = Node::new(Role::Slider);
        node.set_label("Opacity");
        node.set_value(format!("{}%", (opacity.get() * 100.0).round()));
        node
    }));
    let swatch_colors = swatches.clone();
    let chosen = create_memo(clone!(shown -> move || {
        swatch_colors.iter().position(|swatch| *swatch == shown.get())
    }));
    let face_swatches = Rc::new(swatches.clone());
    let has_swatches = !swatches.is_empty();
    let labels: Vec<String> = swatches
        .iter()
        .map(|swatch| format_hex(*swatch, false))
        .collect();
    let indices: Vec<usize> = (0..swatches.len()).collect();
    let alpha_color = color.clone();
    view! {
        <Frame width=PICKER_WIDTH>
            <List spacing=SPACING>
                <unstyled::ColorArea
                    @sizing=ItemSize::Fixed(AREA_HEIGHT)
                    value={color.clone()}
                    focused
                    disabled={disabled.clone()}
                    on_change={move |next: Hsva| area.apply(next)}
                    on_drag_change={move |dragging: bool| area_drag.drag(dragging)}
                >
                    {|handle: ColorAreaHandle| view! {
                        <PlaneFace handle />
                    }}
                </unstyled::ColorArea>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <List @sizing=ItemSize::Percent(100.0) spacing=8.0>
                        <unstyled::Slider
                            value={hue.clone()}
                            min=0.0
                            max=360.0
                            disabled={disabled.clone()}
                            accessibility={hue_accessibility}
                            on_change={move |hue: f32| {
                                let color = hue_picker.color.get_untracked();
                                hue_picker.apply(Hsva::new(hue.min(359.9), color.saturation, color.value, color.alpha));
                            }}
                            on_drag_change={move |dragging: bool| hue_drag.drag(dragging)}
                        >
                            {move |handle: SliderHandle| view! {
                                <StripFace handle strip=Strip::Hue />
                            }}
                        </unstyled::Slider>
                        <Show condition=alpha>
                            <unstyled::Slider
                                value={opacity.clone()}
                                disabled={disabled.clone()}
                                accessibility={alpha_accessibility}
                                on_change={move |alpha: f32| {
                                    let color = alpha_picker.color.get_untracked();
                                    alpha_picker.apply(Hsva { alpha, ..color });
                                }}
                                on_drag_change={move |dragging: bool| alpha_drag.drag(dragging)}
                            >
                                {move |handle: SliderHandle| view! {
                                    <StripFace handle strip={Strip::Alpha(alpha_color)} />
                                }}
                            </unstyled::Slider>
                        </Show>
                    </List>
                    <ColorSwatch color={shown.clone()} width=PREVIEW_SIZE height=PREVIEW_SIZE />
                </List>
                <ColorFields picker={picker.clone()} alpha />
                <Show condition=has_swatches>
                    <unstyled::Choice
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
                            if let Some(swatch) = index.and_then(|index| swatches.get(index)) {
                                let held = swatch_picker.color.get_untracked();
                                swatch_picker.apply(Hsva::from_color_keeping(*swatch, held));
                            }
                        }}
                    >
                        {move |handle: ChoiceOptionHandle| {
                            let color = face_swatches[handle.index];
                            view! {
                                <SwatchFace handle color />
                            }
                        }}
                    </unstyled::Choice>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ColorFields(picker: Picker, alpha: bool) -> NodeId {
    let shown = create_memo(clone!(picker -> move || picker.color.get().to_color()));
    let (text, set_text) = create_signal(hex_text(shown.get_untracked(), alpha));
    create_effect(clone!(shown text set_text -> move || {
        let next = shown.get();
        if parse_typed(&text.get_untracked(), next) != Some(next) {
            set_text.set(hex_text(next, alpha));
        }
    }));
    let typed = clone!(picker set_text -> move |typed: String| {
        set_text.set(typed.clone());
        let held = picker.color.get_untracked();
        if let Some(parsed) = parse_typed(&typed, held.to_color()) {
            picker.apply(Hsva::from_color_keeping(parsed, held));
        }
    });
    let submitted = clone!(picker shown set_text -> move |typed: String| {
        let held = picker.color.get_untracked();
        match parse_hex(&typed) {
            Some(parsed) => {
                let parsed = keep_alpha(&typed, parsed, held.to_color());
                picker.apply(Hsva::from_color_keeping(parsed, held));
            }
            None => set_text.set(hex_text(shown.get_untracked(), alpha)),
        }
    });
    let channel = |index: usize| {
        let shown = shown.clone();
        create_memo(move || f64::from(shown.get().to_array()[index]))
    };
    let (red, green, blue) = (channel(0), channel(1), channel(2));
    let opacity = create_memo(clone!(shown -> move || {
        (f64::from(shown.get().alpha()) * 100.0 / 255.0).round()
    }));
    let set_channel = move |index: usize| {
        let picker = picker.clone();
        move |typed: f64| {
            let held = picker.color.get_untracked();
            let mut channels = held.to_color().to_array();
            channels[index] = typed.round().clamp(0.0, 255.0) as u8;
            let [r, g, b, a] = channels;
            picker.apply(Hsva::from_color_keeping(
                Color32::from_rgba_unmultiplied(r, g, b, a),
                held,
            ));
        }
    };
    let set_opacity = set_channel(3);
    let set_red = set_channel(0);
    let set_green = set_channel(1);
    let set_blue = set_channel(2);
    let placeholder = if alpha { "#RRGGBBAA" } else { "#RRGGBB" };
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={text}
                    label="Hex"
                    placeholder
                    select_on_focus=true
                    on_change={typed}
                    on_submit={submitted}
                />
                <Show condition=alpha>
                    <NumberInput
                        @sizing=ItemSize::Fixed(ALPHA_WIDTH)
                        value={opacity}
                        min=0.0
                        max=100.0
                        label="Opacity percent"
                        on_change={move |percent: f64| set_opacity(percent * 255.0 / 100.0)}
                    />
                </Show>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Caption content="R" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    value={red}
                    min=0.0
                    max=255.0
                    label="Red"
                    on_change={set_red}
                />
                <Caption content="G" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    value={green}
                    min=0.0
                    max=255.0
                    label="Green"
                    on_change={set_green}
                />
                <Caption content="B" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    value={blue}
                    min=0.0
                    max=255.0
                    label="Blue"
                    on_change={set_blue}
                />
            </List>
        </List>
    }
}

#[component]
fn PlaneFace(handle: ColorAreaHandle) -> NodeId {
    let ColorAreaHandle { color, focused, .. } = handle;
    let hue = create_memo(clone!(color -> move || color.get().hue));
    let plane = create_memo(move || plane_image(hue.get()));
    let place = create_memo(move || {
        let color = color.get();
        (
            color.saturation,
            1.0 - color.value,
            color.opaque().to_color(),
        )
    });
    let draw = Prop::Dynamic(Rc::new(move || {
        let image = plane.get();
        let (x, y, fill) = place.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            painter.image(
                rect,
                texel_centres(PLANE_TEXELS, PLANE_TEXELS),
                &image,
                Color32::WHITE,
                RADIUS as f32,
                true,
            );
            let centre = pos2(
                rect.left() + x * rect.width(),
                rect.top() + y * rect.height(),
            );
            thumb(painter, centre, THUMB_RADIUS, fill);
        }) as Draw
    }));
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius={RADIUS + 2}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame height=AREA_HEIGHT>
                <Drawing draw />
            </Frame>
        </Frame>
    }
}

#[derive(Clone)]
enum Strip {
    Hue,
    Alpha(ReadSignal<Hsva>),
}

#[component]
fn StripFace(handle: SliderHandle, strip: Strip) -> NodeId {
    let SliderHandle {
        fraction, focused, ..
    } = handle;
    let knob = create_memo(clone!(strip fraction -> move || match &strip {
        Strip::Hue => Hsva::new(fraction.get() * 360.0, 1.0, 1.0, 1.0).to_color(),
        Strip::Alpha(color) => color.get().to_color(),
    }));
    let image = create_memo(clone!(strip -> move || match &strip {
        Strip::Hue => hue_image(),
        Strip::Alpha(color) => alpha_image(color.get().opaque().to_color()),
    }));
    let checkered = matches!(strip, Strip::Alpha(_));
    let draw = Prop::Dynamic(Rc::new(move || {
        let image = image.get();
        let knob = knob.get();
        let fraction = fraction.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let radius = rect.height() / 2.0;
            if checkered {
                checkerboard(painter, rect);
            }
            painter.image(
                rect,
                texel_centres(image.width(), 1),
                &image,
                Color32::WHITE,
                radius,
                true,
            );
            painter.rect_stroke(rect, radius, BORDER_WIDTH, OUTLINE_DARK);
            let travel = (rect.width() - 2.0 * radius).max(0.0);
            let centre = pos2(rect.left() + radius + fraction * travel, rect.center().y);
            thumb(painter, centre, KNOB_RADIUS, knob);
        }) as Draw
    }));
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=FOCUS_RING_WIDTH
            radius=10
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={focused}
        >
            <Frame height=SLIDER_HEIGHT>
                <Drawing draw />
            </Frame>
        </Frame>
    }
}

#[component]
pub(crate) fn ColorSwatch(color: Prop<Color32>, width: f32, height: f32) -> NodeId {
    let draw = Prop::Dynamic(Rc::new(move || {
        let color = color.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let radius = RADIUS as f32;
            if color.alpha() < u8::MAX {
                checkerboard(painter, rect);
            }
            painter.rect_filled(rect, radius, color);
            painter.rect_stroke(rect, radius, BORDER_WIDTH, OUTLINE_DARK);
        }) as Draw
    }));
    view! {
        <Frame width height>
            <Drawing draw />
        </Frame>
    }
}

#[component]
fn SwatchFace(handle: ChoiceOptionHandle, color: Color32) -> NodeId {
    let ChoiceOptionHandle {
        selected,
        hovered,
        focused,
        ..
    } = handle;
    let theme = use_theme();
    let ring = create_memo(clone!(selected focused -> move || selected.get() || focused.get()));
    let ring_color = create_memo(
        clone!(theme -> move || match (focused.get(), hovered.get()) {
            (true, _) => theme.accent.get(),
            (false, _) => theme.text.get(),
        }),
    );
    view! {
        <Frame
            outline={ring_color}
            outline_width=FOCUS_RING_WIDTH
            radius={CHIP_RADIUS + 2}
            outline_offset=FOCUS_RING_OFFSET
            outline_visible={ring}
        >
            <Frame
                width=SWATCH_SIZE
                height=SWATCH_SIZE
                color
                outline={OUTLINE_DARK}
                outline_width=BORDER_WIDTH
                outline_visible=true
                radius=CHIP_RADIUS
            />
        </Frame>
    }
}

pub(crate) fn checkerboard(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, CHECKER_LIGHT);
    let columns = (rect.width() / CHECKER).ceil() as usize;
    let rows = (rect.height() / CHECKER).ceil() as usize;
    for row in 0..rows {
        for column in (row % 2..columns).step_by(2) {
            let min = pos2(
                rect.left() + column as f32 * CHECKER,
                rect.top() + row as f32 * CHECKER,
            );
            let cell = Rect::from_min_max(
                min,
                pos2(
                    (min.x + CHECKER).min(rect.right()),
                    (min.y + CHECKER).min(rect.bottom()),
                ),
            );
            painter.rect_filled(cell, 0.0, CHECKER_DARK);
        }
    }
}

fn thumb(painter: &Painter, centre: Pos2, radius: f32, fill: Color32) {
    let outer = Rect::from_center_size(centre, Vec2::new(radius * 2.0, radius * 2.0));
    painter.rect_filled(outer, radius, Color32::WHITE);
    let inner = outer.shrink(2.0);
    painter.rect_filled(inner, radius - 2.0, fill);
    painter.rect_stroke(outer, radius, BORDER_WIDTH, OUTLINE_DARK);
}

fn texel_centres(width: u32, height: u32) -> Rect {
    let inset = |texels: u32| 0.5 / texels as f32;
    Rect::from_min_max(
        pos2(inset(width), inset(height)),
        pos2(1.0 - inset(width), 1.0 - inset(height)),
    )
}

fn hue_image() -> Image {
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

fn plane_image(hue: f32) -> Image {
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

fn alpha_image(color: Color32) -> Image {
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
