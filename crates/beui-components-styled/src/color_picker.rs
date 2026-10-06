use std::rc::Rc;

use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::number_input::NumberInput;
use crate::text::Caption;
use crate::text_input::TextInput;
use crate::theme::{BORDER_WIDTH, CHIP_RADIUS, FOCUS_RING_WIDTH, RADIUS, use_theme};
use beui_components_unstyled::{
    AlphaSlider, ChoiceOptionHandle, ColorAreaHandle, ColorPickerArea, ColorPickerState, HexText,
    HueSlider, SliderHandle, SwatchHandle, Swatches, alpha_image, hue_image, plane_image,
    texel_centres,
};
use beui_core::base::{Align, Direction};
use beui_core::color::{Color32, Hsva};
use beui_core::geometry::{Pos2, Rect, Vec2, pos2};
use beui_core::node::NodeId;
use beui_core::painter::Painter;
use beui_view::reactive::{
    Callback, Draw, Drawing, Frame, Grid, ItemSize, List, Prop, ReadSignal, Show, Track, clone,
    create_memo, focus_ring,
};

pub const PICKER_WIDTH: f32 = 244.0;
const AREA_HEIGHT: f32 = 156.0;
const SLIDER_HEIGHT: f32 = 16.0;
const PREVIEW_SIZE: f32 = 40.0;
const SWATCH_SIZE: f32 = 18.0;
const SPACING: f32 = 10.0;
const FOCUS_RING_OFFSET: f32 = 2.0;
const THUMB_RADIUS: f32 = 7.0;
const KNOB_RADIUS: f32 = 8.0;
const GRAB_SLOP: f32 = 10.0;
const CHECKER: f32 = 5.0;
const CHECKER_LIGHT: Color32 = Color32::from_gray(236);
const CHECKER_DARK: Color32 = Color32::from_gray(190);
const OUTLINE_DARK: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 110);
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
    let picker: ColorPickerState = ColorPickerState::new(value, disabled, on_change, on_preview);
    let color = picker.color();
    let has_swatches = !swatches.is_empty();
    let (hue_picker, alpha_picker, swatch_picker) =
        (picker.clone(), picker.clone(), picker.clone());
    let shown = picker.shown();
    view! {
        <Frame width=PICKER_WIDTH>
            <List spacing=SPACING>
                <ColorPickerArea
                    @sizing=ItemSize::Fixed(AREA_HEIGHT)
                    picker={picker.clone()}
                    thumb={THUMB_RADIUS * 2.0 + GRAB_SLOP}
                    focused
                >
                    {|handle: ColorAreaHandle| view! {
                        <PlaneFace handle />
                    }}
                </ColorPickerArea>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <List @sizing=ItemSize::Percent(100.0) spacing=8.0>
                        <HueSlider picker={hue_picker} thumb=SLIDER_HEIGHT>
                            {move |handle: SliderHandle| view! {
                                <StripFace handle strip=Strip::Hue />
                            }}
                        </HueSlider>
                        <Show condition=alpha>
                            {move || clone!(color -> view! {
                                <AlphaSlider picker={alpha_picker.clone()} thumb=SLIDER_HEIGHT>
                                    {move |handle: SliderHandle| view! {
                                        <StripFace handle strip={Strip::Alpha(color)} />
                                    }}
                                </AlphaSlider>
                            })}
                        </Show>
                    </List>
                    <ColorSwatch color={shown} width=PREVIEW_SIZE height=PREVIEW_SIZE />
                </List>
                <ColorFields picker={picker.clone()} alpha />
                <Show condition=has_swatches>
                    {move || clone!(swatches -> view! {
                        <Swatches picker={swatch_picker.clone()} swatches>
                            {|handle: SwatchHandle| view! {
                                <SwatchFace handle />
                            }}
                        </Swatches>
                    })}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn ColorFields(picker: ColorPickerState, alpha: bool) -> NodeId {
    let typed = picker.clone();
    let hex = HexText::new(
        picker.shown(),
        alpha,
        Callback::new(move |color: Color32| typed.pick(color)),
    );
    let (edit, submit) = (hex.clone(), hex.clone());
    let setter = |set: fn(&ColorPickerState, f64)| {
        let picker = picker.clone();
        move |typed: f64| set(&picker, typed)
    };
    let set_hue = setter(|picker, typed| picker.set_hue(typed as f32));
    let set_saturation = setter(ColorPickerState::set_saturation_percent);
    let set_value = setter(ColorPickerState::set_value_percent);
    let set_red = setter(|picker, typed| picker.set_channel(0, typed));
    let set_green = setter(|picker, typed| picker.set_channel(1, typed));
    let set_blue = setter(|picker, typed| picker.set_channel(2, typed));
    let set_opacity = setter(ColorPickerState::set_opacity_percent);
    let opacity = picker.opacity_percent();
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={hex.text()}
                    label="Hex"
                    placeholder={hex.placeholder()}
                    select_on_focus=true
                    on_change={move |typed: String| edit.edit(typed)}
                    on_submit={move |typed: String| submit.submit(typed)}
                />
                <Show condition=alpha>
                    {move || clone!(set_opacity -> view! {
                        <NumberInput
                            @sizing=ItemSize::Fixed(ALPHA_WIDTH)
                            value={opacity.clone()}
                            min=0.0
                            max=100.0
                            label="Opacity percent"
                            on_change={set_opacity}
                        />
                    })}
                </Show>
            </List>
            <Grid
                columns={[Track::Intrinsic, Track::Fraction(1.0)].repeat(3)}
                column_spacing=6.0
                row_spacing=8.0
            >
                <ChannelLabel content="R" />
                <NumberInput
                    value={picker.channel(0)}
                    min=0.0
                    max=255.0
                    label="Red"
                    on_change={set_red}
                />
                <ChannelLabel content="G" />
                <NumberInput
                    value={picker.channel(1)}
                    min=0.0
                    max=255.0
                    label="Green"
                    on_change={set_green}
                />
                <ChannelLabel content="B" />
                <NumberInput
                    value={picker.channel(2)}
                    min=0.0
                    max=255.0
                    label="Blue"
                    on_change={set_blue}
                />
                <ChannelLabel content="H" />
                <NumberInput
                    value={picker.hue_degrees()}
                    min=0.0
                    max=360.0
                    label="Hue degrees"
                    on_change={set_hue}
                />
                <ChannelLabel content="S" />
                <NumberInput
                    value={picker.saturation_percent()}
                    min=0.0
                    max=100.0
                    label="Saturation percent"
                    on_change={set_saturation}
                />
                <ChannelLabel content="V" />
                <NumberInput
                    value={picker.value_percent()}
                    min=0.0
                    max=100.0
                    label="Value percent"
                    on_change={set_value}
                />
            </Grid>
        </List>
    }
}

#[component]
fn ChannelLabel(content: Prop<String>) -> NodeId {
    view! {
        <Frame align_vertical=Align::Center>
            <Caption content />
        </Frame>
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
                texel_centres(image.width(), image.height()),
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
    view! {
        <FocusRing focused radius={RADIUS + 2} offset=FOCUS_RING_OFFSET>
            <Frame height=AREA_HEIGHT>
                <Drawing draw />
            </Frame>
        </FocusRing>
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
    view! {
        <FocusRing focused radius=10 offset=FOCUS_RING_OFFSET>
            <Frame height=SLIDER_HEIGHT>
                <Drawing draw />
            </Frame>
        </FocusRing>
    }
}

#[component]
pub fn ColorSwatch(color: Prop<Color32>, width: f32, height: f32) -> NodeId {
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
fn SwatchFace(handle: SwatchHandle) -> NodeId {
    let SwatchHandle {
        color,
        option: ChoiceOptionHandle {
            selected, focused, ..
        },
    } = handle;
    let theme = use_theme();
    let keyboard = focus_ring(focused);
    let ring = create_memo(clone!(selected keyboard -> move || selected.get() || keyboard.get()));
    let ring_color = create_memo(clone!(theme -> move || match keyboard.get() {
        true => theme.accent.get(),
        false => theme.text.get(),
    }));
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

pub fn checkerboard(painter: &Painter, rect: Rect) {
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

pub(crate) fn thumb(painter: &Painter, centre: Pos2, radius: f32, fill: Color32) {
    let outer = Rect::from_center_size(centre, Vec2::new(radius * 2.0, radius * 2.0));
    painter.rect_filled(outer, radius, Color32::WHITE);
    let inner = outer.shrink(2.0);
    painter.rect_filled(inner, radius - 2.0, fill);
    painter.rect_stroke(outer, radius, BORDER_WIDTH, OUTLINE_DARK);
}
