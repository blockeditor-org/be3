use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_DROPLET_OFF, ICON_PALETTE};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use infinite_canvas_types::{
    CanvasColor, CanvasEntity, CanvasFill, CanvasHue, CanvasLook, CanvasShape, CanvasStrokeStyle,
    CanvasStyle, CanvasTextSize, CanvasTone,
};

use super::selection_bar::Swatch;
use super::sidebar::{closed, colored, stroked};
use super::state::{CanvasState, CommonValue, common_value};

const LABEL_WIDTH: f32 = 72.0;
const ROW_SPACING: Space = Space::Xs;
const SECTION_SPACING: Space = Space::Sm;

const fn palette(hue: CanvasHue, tone: CanvasTone) -> CanvasColor {
    CanvasColor::Palette { hue, tone }
}

pub(crate) const STROKES: [(&str, CanvasColor); 5] = [
    ("Default", CanvasColor::Auto),
    ("Red", palette(CanvasHue::Red, CanvasTone::Strong)),
    ("Green", palette(CanvasHue::Green, CanvasTone::Strong)),
    ("Blue", palette(CanvasHue::Blue, CanvasTone::Strong)),
    ("Orange", palette(CanvasHue::Orange, CanvasTone::Strong)),
];

pub(crate) const BACKGROUNDS: [(&str, CanvasColor); 5] = [
    ("Red", palette(CanvasHue::Red, CanvasTone::Soft)),
    ("Green", palette(CanvasHue::Green, CanvasTone::Soft)),
    ("Blue", palette(CanvasHue::Blue, CanvasTone::Soft)),
    ("Yellow", palette(CanvasHue::Yellow, CanvasTone::Soft)),
    ("Violet", palette(CanvasHue::Violet, CanvasTone::Soft)),
];

const FILLS: [(&str, CanvasFill); 3] = [
    ("None", CanvasFill::None),
    ("Hatched", CanvasFill::Hatched),
    ("Solid", CanvasFill::Solid),
];

pub(crate) const STROKE_WIDTHS: [(&str, f32); 3] =
    [("Thin", 1.0), ("Bold", 2.0), ("Extra bold", 4.0)];

const STROKE_STYLES: [(&str, CanvasStrokeStyle); 3] = [
    ("Solid", CanvasStrokeStyle::Solid),
    ("Dashed", CanvasStrokeStyle::Dashed),
    ("Dotted", CanvasStrokeStyle::Dotted),
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Roughness {
    look: CanvasLook,
    roughness: f32,
}

impl Roughness {
    fn of(style: &CanvasStyle) -> Self {
        match style.look {
            CanvasLook::Clean => CLEAN,
            CanvasLook::HandDrawn => Self {
                look: CanvasLook::HandDrawn,
                roughness: style.roughness,
            },
        }
    }

    fn apply(self, style: &mut CanvasStyle) {
        style.look = self.look;
        if self.look == CanvasLook::HandDrawn {
            style.roughness = self.roughness;
        }
    }
}

const CLEAN: Roughness = Roughness {
    look: CanvasLook::Clean,
    roughness: 0.0,
};

const ROUGHNESSES: [(&str, Roughness); 4] = [
    ("Clean", CLEAN),
    (
        "Low",
        Roughness {
            look: CanvasLook::HandDrawn,
            roughness: 0.5,
        },
    ),
    (
        "Medium",
        Roughness {
            look: CanvasLook::HandDrawn,
            roughness: 1.0,
        },
    ),
    (
        "High",
        Roughness {
            look: CanvasLook::HandDrawn,
            roughness: 2.0,
        },
    ),
];

const TEXT_SIZES: [(&str, CanvasTextSize); 4] = [
    ("S", CanvasTextSize::Small),
    ("M", CanvasTextSize::Medium),
    ("L", CanvasTextSize::Large),
    ("XL", CanvasTextSize::ExtraLarge),
];

pub(crate) fn shade(hue: CanvasHue, tone: CanvasTone, ink: Color32) -> Color32 {
    let [red, green, blue, _] = ink.to_array();
    let dark = u32::from(red) + u32::from(green) + u32::from(blue) > 3 * 128;
    let [r, g, b] = match (tone, dark) {
        (CanvasTone::Strong, false) => match hue {
            CanvasHue::Red => [224, 49, 49],
            CanvasHue::Orange => [240, 140, 0],
            CanvasHue::Yellow => [230, 170, 0],
            CanvasHue::Green => [47, 158, 68],
            CanvasHue::Blue => [25, 113, 194],
            CanvasHue::Violet => [103, 65, 217],
        },
        (CanvasTone::Strong, true) => match hue {
            CanvasHue::Red => [255, 135, 135],
            CanvasHue::Orange => [255, 169, 77],
            CanvasHue::Yellow => [255, 212, 59],
            CanvasHue::Green => [105, 219, 124],
            CanvasHue::Blue => [77, 171, 247],
            CanvasHue::Violet => [151, 117, 250],
        },
        (CanvasTone::Soft, false) => match hue {
            CanvasHue::Red => [255, 201, 201],
            CanvasHue::Orange => [255, 216, 168],
            CanvasHue::Yellow => [255, 236, 153],
            CanvasHue::Green => [178, 242, 187],
            CanvasHue::Blue => [165, 216, 255],
            CanvasHue::Violet => [208, 191, 255],
        },
        (CanvasTone::Soft, true) => match hue {
            CanvasHue::Red => [92, 38, 42],
            CanvasHue::Orange => [94, 60, 24],
            CanvasHue::Yellow => [86, 74, 22],
            CanvasHue::Green => [30, 72, 42],
            CanvasHue::Blue => [24, 58, 94],
            CanvasHue::Violet => [58, 44, 100],
        },
    };
    Color32::from_rgb(r, g, b)
}

fn texted(entity: &CanvasEntity) -> bool {
    entity.shape == CanvasShape::Text || (colored(entity) && !entity.text.is_empty())
}

fn style_value<T: Copy + PartialEq + 'static>(
    state: CanvasState,
    applies: fn(&CanvasEntity) -> bool,
    read: fn(&CanvasStyle) -> T,
) -> Memo<CommonValue<T>> {
    create_memo(move || {
        let selected = state.selected_entities();
        match selected.is_empty() {
            true => CommonValue::Uniform(read(&state.next_style())),
            false => common_value(
                selected
                    .iter()
                    .filter(|entity| applies(entity))
                    .map(|entity| read(&entity.style)),
            ),
        }
    })
}

#[component]
pub(crate) fn StylePopover(state: CanvasState) -> NodeId {
    view! {
        <Popover
            label="Style"
            glyph=ICON_PALETTE
            icon_only=true
            variant=ButtonVariant::Ghost
            @test_id={"infinite-canvas.style"}
        >|_|
            <StyleControls state={state} />
        </Popover>
    }
}

#[component]
fn StyleControls(state: CanvasState) -> NodeId {
    let stroke = style_value(state, colored, |style| style.stroke_color);
    let fill = style_value(state, closed, |style| style.fill);
    let background = style_value(state, closed, |style| match style.fill {
        CanvasFill::None => None,
        _ => Some(style.fill_color),
    });
    let width = style_value(state, stroked, |style| style.stroke_width);
    let stroke_style = style_value(state, stroked, |style| style.stroke_style);
    let roughness = style_value(state, stroked, Roughness::of);
    let size = style_value(state, texted, |style| style.text_size);
    let opacity = style_value(state, colored, |style| style.opacity);
    let shows = |value: Memo<CommonValue<CanvasColor>>| create_memo(move || !value.get().absent());
    let stroked_shown = create_memo(move || !width.get().absent());
    let filled_shown = create_memo(move || !fill.get().absent());
    let sized_shown = create_memo(move || !size.get().absent());
    let transparent = create_memo(move || background.get() == CommonValue::Uniform(None));
    let background_color = create_memo(move || match background.get() {
        CommonValue::Uniform(Some(color)) => CommonValue::Uniform(color),
        CommonValue::Mixed => CommonValue::Mixed,
        CommonValue::Uniform(None) | CommonValue::None => CommonValue::None,
    });
    let pick_background = move |color: Option<CanvasColor>| {
        state.update_selected(closed, move |style| match color {
            Some(color) => {
                style.fill_color = color;
                if style.fill == CanvasFill::None {
                    style.fill = CanvasFill::Hatched;
                }
            }
            None => style.fill = CanvasFill::None,
        });
    };
    let opacity_value = create_memo(move || opacity.get().or(1.0));
    let opacity_set = move |value: f32| {
        state.update_selected(colored, move |style| style.opacity = value);
    };
    view! {
        <List spacing=SECTION_SPACING @test_id={"infinite-canvas.style.panel"}>
            <Show condition={shows(stroke)}>
                <StyleRow label="Stroke">
                    <ForEach keys={(0..STROKES.len()).collect::<Vec<usize>>()}>|index: usize| {
                        let (name, color) = STROKES[index];
                        view! {
                            <Swatch
                                kind="stroke"
                                name
                                color
                                value={stroke}
                                on_pick={move |color| {
                                    state.update_selected(colored, move |style| style.stroke_color = color);
                                }}
                            />
                        }
                    }</ForEach>
                </StyleRow>
            </Show>
            <Show condition={filled_shown}>
                <List spacing=SECTION_SPACING>
                    <StyleRow label="Background">
                        <ToggleButton
                            label="Transparent"
                            glyph=ICON_DROPLET_OFF
                            icon_only=true
                            pressed={Binding::new(transparent, move |_: bool| pick_background(None))}
                            @test_id={"infinite-canvas.background.none"}
                        />
                        <ForEach
                            keys={(0..BACKGROUNDS.len()).collect::<Vec<usize>>()}
                        >|index: usize| {
                            let (name, color) = BACKGROUNDS[index];
                            view! {
                                <Swatch
                                    kind="background"
                                    name
                                    color
                                    value={background_color}
                                    on_pick={move |color| pick_background(Some(color))}
                                />
                            }
                        }</ForEach>
                    </StyleRow>
                    <StyleRow label="Fill">
                        <ForEach keys={(0..FILLS.len()).collect::<Vec<usize>>()}>|index: usize| {
                            let (name, choice) = FILLS[index];
                            view! {
                                <StyleChoice
                                    kind="fill-style"
                                    name
                                    chosen={create_memo(move || fill.get() == CommonValue::Uniform(choice))}
                                    on_pick={move |()| state.update_selected(closed, move |style| style.fill = choice)}
                                />
                            }
                        }</ForEach>
                    </StyleRow>
                </List>
            </Show>
            <Show condition={stroked_shown}>
                <List spacing=SECTION_SPACING>
                    <StyleRow label="Width">
                        <ForEach
                            keys={(0..STROKE_WIDTHS.len()).collect::<Vec<usize>>()}
                        >|index: usize| {
                            let (name, choice) = STROKE_WIDTHS[index];
                            view! {
                                <StyleChoice
                                    kind="width"
                                    name
                                    chosen={create_memo(move || width.get() == CommonValue::Uniform(choice))}
                                    on_pick={move |()| state.update_selected(stroked, move |style| style.stroke_width = choice)}
                                />
                            }
                        }</ForEach>
                    </StyleRow>
                    <StyleRow label="Line">
                        <ForEach
                            keys={(0..STROKE_STYLES.len()).collect::<Vec<usize>>()}
                        >|index: usize| {
                            let (name, choice) = STROKE_STYLES[index];
                            view! {
                                <StyleChoice
                                    kind="stroke-style"
                                    name
                                    chosen={create_memo(move || stroke_style.get() == CommonValue::Uniform(choice))}
                                    on_pick={move |()| state.update_selected(stroked, move |style| style.stroke_style = choice)}
                                />
                            }
                        }</ForEach>
                    </StyleRow>
                    <StyleRow label="Roughness">
                        <ForEach
                            keys={(0..ROUGHNESSES.len()).collect::<Vec<usize>>()}
                        >|index: usize| {
                            let (name, choice) = ROUGHNESSES[index];
                            view! {
                                <StyleChoice
                                    kind="roughness"
                                    name
                                    chosen={create_memo(move || roughness.get() == CommonValue::Uniform(choice))}
                                    on_pick={move |()| state.update_selected(stroked, move |style| choice.apply(style))}
                                />
                            }
                        }</ForEach>
                    </StyleRow>
                </List>
            </Show>
            <Show condition={sized_shown}>
                <StyleRow label="Text size">
                    <ForEach keys={(0..TEXT_SIZES.len()).collect::<Vec<usize>>()}>|index: usize| {
                        let (name, choice) = TEXT_SIZES[index];
                        view! {
                            <StyleChoice
                                kind="text-size"
                                name
                                chosen={create_memo(move || size.get() == CommonValue::Uniform(choice))}
                                on_pick={move |()| state.update_selected(texted, move |style| style.text_size = choice)}
                            />
                        }
                    }</ForEach>
                </StyleRow>
            </Show>
            <Show condition={shows(stroke)}>
                <StyleRow label="Opacity">
                    <Slider
                        @sizing=ItemSize::Percent(100.0)
                        label="Opacity"
                        value={Binding::new(opacity_value, opacity_set)}
                        @test_id={"infinite-canvas.style.opacity"}
                    />
                </StyleRow>
            </Show>
        </List>
    }
}

#[component]
fn StyleRow(label: &'static str, #[prop(children)] children: Children<ListChild>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <Text @sizing=ItemSize::Fixed(LABEL_WIDTH) style=TextStyle::CAPTION>{label}</Text>
            <List
                @sizing={ItemSize::Percent(100.0).shrink(1.0)}
                direction=Direction::Horizontal
                align=Align::Center
                spacing=ROW_SPACING
                wrap=true
                children={children}
            />
        </List>
    }
}

#[component]
fn StyleChoice(
    kind: &'static str,
    name: &'static str,
    chosen: Memo<bool>,
    on_pick: Callback<()>,
) -> NodeId {
    view! {
        <ToggleButton
            label={name}
            pressed={Binding::new(chosen, move |_: bool| on_pick.call(()))}
            @test_id={format!("infinite-canvas.{kind}.{}", name.replace(' ', "-"))}
        />
    }
}
