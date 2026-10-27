use crate::frame::{Border, ColorRole, Frame, Radius};
use std::rc::Rc;

use beui_macros::{component, view};

use crate::focus_ring::FocusRing;
use crate::list::{Grid, List};
use crate::number_input::NumberInput;
use crate::text::{Text, TextStyle};
use crate::text_input::TextInput;
use crate::tokens::{Size, Space, Stroke};
use beui_components_unstyled::{
    AlphaSlider, ChoiceOptionHandle, ColorAreaHandle, ColorPickerArea, ColorPickerState, HexText,
    HueSlider, SliderHandle, SwatchHandle, Swatches, alpha_image, hue_image, plane_image,
    texel_centres,
};
use beui_view::primitives::Drawing;
use beui_view::reactive::NodeId;
use beui_view::reactive::Painter;
use beui_view::reactive::*;
use beui_view::types::{Align, Direction};
use beui_view::types::{Align2, ItemSize, Track};
use beui_view::types::{Color32, Hsva};
use beui_view::types::{Pos2, Rect, Vec2, pos2};

pub const PICKER_WIDTH: f32 = 244.0;
const AREA_HEIGHT: f32 = 156.0;
const SLIDER_HEIGHT: Size = Size::Xs;
const PREVIEW_SIZE: Size = Size::Xl;
const SWATCH_SIZE: Size = Size::Custom(18.0);
const SPACING: Space = Space::Md;
const FIELD_SPACING: Space = Space::Md;
const COLUMN_SPACING: Space = Space::Sm;
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
    value: Binding<Color32>,
    #[prop(default = true)] alpha: bool,
    #[prop(default = DEFAULT_SWATCHES.to_vec())] swatches: Vec<Color32>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    on_preview: Callback<Option<Color32>>,
) -> NodeId {
    let picker: ColorPickerState = ColorPickerState::new(value, disabled, on_preview);
    let color = picker.color();
    let has_swatches = !swatches.is_empty();
    let shown = picker.shown();
    view! {
        <Frame width={Size::Custom(PICKER_WIDTH)}>
            <List spacing=SPACING>
                <ColorPickerArea
                    @sizing=ItemSize::Fixed(AREA_HEIGHT)
                    picker={picker}
                    thumb={THUMB_RADIUS * 2.0 + GRAB_SLOP}
                    focused
                >|handle: ColorAreaHandle|
                    <PlaneFace handle />
                </ColorPickerArea>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <List @sizing=ItemSize::Percent(100.0) spacing=FIELD_SPACING>
                        <HueSlider picker={picker} thumb={SLIDER_HEIGHT.px()}>|handle: SliderHandle|
                            <StripFace handle strip=Strip::Hue />
                        </HueSlider>
                        <Show condition=alpha>
                            <AlphaSlider
                                picker={picker}
                                thumb={SLIDER_HEIGHT.px()}
                            >|handle: SliderHandle|
                                <StripFace handle strip={Strip::Alpha(color)} />
                            </AlphaSlider>
                        </Show>
                    </List>
                    <ColorSwatch color={shown} width=PREVIEW_SIZE height=PREVIEW_SIZE />
                </List>
                <ColorFields picker={picker} alpha />
                <Show condition=has_swatches>|| {
                    let swatches = swatches.clone(); view! {
                        <Swatches
                            picker={picker}
                            swatches
                            spacing={crate::choice::OPTION_SPACING.px()}
                        >|handle: SwatchHandle|
                            <SwatchFace handle />
                        </Swatches>
                    }
                }</Show>
            </List>
        </Frame>
    }
}

#[component]
fn ColorFields(picker: ColorPickerState, alpha: bool) -> NodeId {
    let hex = HexText::new(
        picker.shown(),
        alpha,
        Callback::new(move |color: Color32| picker.pick(color)),
    );
    let setter = |set: fn(&ColorPickerState, f64)| move |typed: f64| set(&picker, typed);
    let set_hue = setter(|picker, typed| picker.set_hue(typed as f32));
    let set_saturation = setter(ColorPickerState::set_saturation_percent);
    let set_value = setter(ColorPickerState::set_value_percent);
    let set_red = setter(|picker, typed| picker.set_channel(0, typed));
    let set_green = setter(|picker, typed| picker.set_channel(1, typed));
    let set_blue = setter(|picker, typed| picker.set_channel(2, typed));
    let set_opacity = setter(ColorPickerState::set_opacity_percent);
    let opacity = picker.opacity_percent();
    view! {
        <List spacing=FIELD_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=COLUMN_SPACING>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={hex.text()}
                    label="Hex"
                    placeholder={hex.placeholder()}
                    select_on_focus=true
                    on_submit={move |typed: String| hex.submit(typed)}
                />
                <Show condition=alpha>
                    <NumberInput
                        @sizing=ItemSize::Fixed(ALPHA_WIDTH)
                        value={Binding::new(opacity, set_opacity)}
                        min=0.0
                        max=100.0
                        label="Opacity percent"
                    />
                </Show>
            </List>
            <Grid
                columns={[Track::Intrinsic, Track::Fraction(1.0)].repeat(3)}
                column_spacing=COLUMN_SPACING
                row_spacing=FIELD_SPACING
            >
                <ChannelLabel content="R" />
                <NumberInput
                    value={Binding::new(picker.channel(0), set_red)}
                    min=0.0
                    max=255.0
                    label="Red"
                />
                <ChannelLabel content="G" />
                <NumberInput
                    value={Binding::new(picker.channel(1), set_green)}
                    min=0.0
                    max=255.0
                    label="Green"
                />
                <ChannelLabel content="B" />
                <NumberInput
                    value={Binding::new(picker.channel(2), set_blue)}
                    min=0.0
                    max=255.0
                    label="Blue"
                />
                <ChannelLabel content="H" />
                <NumberInput
                    value={Binding::new(picker.hue_degrees(), set_hue)}
                    min=0.0
                    max=360.0
                    label="Hue degrees"
                />
                <ChannelLabel content="S" />
                <NumberInput
                    value={Binding::new(picker.saturation_percent(), set_saturation)}
                    min=0.0
                    max=100.0
                    label="Saturation percent"
                />
                <ChannelLabel content="V" />
                <NumberInput
                    value={Binding::new(picker.value_percent(), set_value)}
                    min=0.0
                    max=100.0
                    label="Value percent"
                />
            </Grid>
        </List>
    }
}

#[component]
fn ChannelLabel(content: Prop<String>) -> NodeId {
    view! {
        <Frame align={Align2::vertical(Align::Center)}>
            <Text style=TextStyle::CAPTION>{content}</Text>
        </Frame>
    }
}

#[component]
fn PlaneFace(handle: ColorAreaHandle) -> NodeId {
    let ColorAreaHandle { color, focused, .. } = handle;
    let hue = create_memo(move || color.get().hue);
    let plane = create_memo(move || plane_image(hue.get()));
    let place = create_memo(move || {
        let color = color.get();
        (
            color.saturation,
            1.0 - color.value,
            color.opaque().to_color(),
        )
    });
    let draw = Prop::dynamic(move || {
        let image = plane.get();
        let (x, y, fill) = place.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            painter.image(
                rect,
                texel_centres(image.width(), image.height()),
                &image,
                Color32::WHITE,
                Radius::Md.px(),
                true,
            );
            let centre = pos2(
                rect.left() + x * rect.width(),
                rect.top() + y * rect.height(),
            );
            thumb(painter, centre, THUMB_RADIUS, fill);
        }) as Draw
    });
    view! {
        <FocusRing focused radius=Radius::Md offset=Stroke::Focus>
            <Frame height={Size::Custom(AREA_HEIGHT)}>
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
    let knob = create_memo({
        let strip = strip.clone();
        move || match &strip {
            Strip::Hue => Hsva::new(fraction.get() * 360.0, 1.0, 1.0, 1.0).to_color(),
            Strip::Alpha(color) => color.get().to_color(),
        }
    });
    let image = create_memo({
        let strip = strip.clone();
        move || match &strip {
            Strip::Hue => hue_image(),
            Strip::Alpha(color) => alpha_image(color.get().opaque().to_color()),
        }
    });
    let checkered = matches!(strip, Strip::Alpha(_));
    let draw = Prop::dynamic(move || {
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
            painter.rect_stroke(rect, radius, Stroke::Hairline.px(), OUTLINE_DARK);
            let travel = (rect.width() - 2.0 * radius).max(0.0);
            let centre = pos2(rect.left() + radius + fraction * travel, rect.center().y);
            thumb(painter, centre, KNOB_RADIUS, knob);
        }) as Draw
    });
    view! {
        <FocusRing focused radius=Radius::Full offset=Stroke::Focus>
            <Frame height=SLIDER_HEIGHT>
                <Drawing draw />
            </Frame>
        </FocusRing>
    }
}

#[component]
pub fn ColorSwatch(color: Prop<Color32>, width: Size, height: Size) -> NodeId {
    let draw = Prop::dynamic(move || {
        let color = color.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            let radius = Radius::Md.px();
            if color.alpha() < u8::MAX {
                checkerboard(painter, rect);
            }
            painter.rect_filled(rect, radius, color);
            painter.rect_stroke(rect, radius, Stroke::Hairline.px(), OUTLINE_DARK);
        }) as Draw
    });
    view! {
        <Frame width=width height=height>
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
    let keyboard = focus_ring(focused);
    let ring = create_memo(move || {
        let color = match keyboard.get() {
            true => ColorRole::Accent,
            false => ColorRole::Text,
        };
        (selected.get() || keyboard.get()).then(|| {
            Border::new(color)
                .width(Stroke::Focus)
                .offset(Stroke::Focus)
        })
    });
    view! {
        <Frame border={ring} radius=Radius::Md>
            <Frame
                width=SWATCH_SIZE
                height=SWATCH_SIZE
                fill={ColorRole::custom(color)}
                border={Border::new(ColorRole::Custom(OUTLINE_DARK.into()))}
                radius=Radius::Sm
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
    painter.rect_stroke(outer, radius, Stroke::Hairline.px(), OUTLINE_DARK);
}
