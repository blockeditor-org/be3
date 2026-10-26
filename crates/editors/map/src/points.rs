use beui::Align;
use block_editor_beui::beui::icons::ICON_MAP_PIN;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, Pos2, Rect, TextAlign};
use map_types::{MapColor, MapPoint};
use uuid::Uuid;

const MARKER_SIZE: TextSize = TextSize::Xl;
const RING_SIZE: f32 = MARKER_SIZE.icon_px() + 6.0;
const LABEL_HEIGHT: f32 = 18.0;
const LABEL_WIDTH: f32 = 168.0;
const HIT_RADIUS: f32 = 14.0;
const DEFAULT_COLOR: Color32 = Color32::from_rgb(224, 49, 49);
const LABEL_BACKGROUND: Color32 = Color32::from_rgba_unmultiplied(255, 255, 255, 190);
const LABEL_COLOR: Color32 = Color32::from_gray(30);
const SELECTION_COLOR: Color32 = Color32::from_rgb(66, 153, 225);

pub(crate) const MARKER_HEIGHT: f32 = RING_SIZE + LABEL_HEIGHT;

pub(crate) fn marker_color(color: MapColor) -> Color32 {
    match color {
        MapColor::Default => DEFAULT_COLOR,
        MapColor::Rgb { red, green, blue } => Color32::from_rgb(red, green, blue),
    }
}

#[component]
pub(crate) fn Marker(
    tip: Prop<Pos2>,
    scale: Prop<f32>,
    color: Prop<Color32>,
    selected: Prop<bool>,
    label: Prop<String>,
) -> CanvasItem {
    let unit = create_memo(move || 1.0 / scale.get().max(f32::EPSILON));
    let x = create_memo(move || tip.get().x - LABEL_WIDTH / 2.0 * unit.get());
    let y = create_memo(move || tip.get().y - RING_SIZE * unit.get());
    let width = create_memo(move || LABEL_WIDTH * unit.get());
    let height = create_memo(move || MARKER_HEIGHT * unit.get());
    let named = create_memo({
        let label = label.clone();
        move || !label.get().is_empty()
    });
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <List spacing=Space::None align=Align::Center>
                <Frame
                    width={Size::Custom(RING_SIZE)}
                    height={Size::Custom(RING_SIZE)}
                    radius=Radius::Full
                    border={create_memo(move || selected.get().then_some(Border::new(ColorRole::Custom(SELECTION_COLOR)).width(Stroke::Focus)))}
                >
                    <Text
                        size=MARKER_SIZE
                        color={color.map(TextColor::Custom)}
                        align=TextAlign::Center
                    >
                        <Icon glyph=ICON_MAP_PIN />
                    </Text>
                </Frame>
                <Show condition={named}>|| {
                    let label = label.clone();
                    view! {
                        <Frame
                            fill={ColorRole::Custom(LABEL_BACKGROUND)}
                            radius=Radius::Xs
                            padding={Padding::horizontal(Space::Xs2)}
                            height={Size::Custom(LABEL_HEIGHT)}
                        >
                            <Text
                                style=TextStyle::CAPTION
                                align=TextAlign::Center
                                color={TextColor::Custom(LABEL_COLOR)}
                            >
                                {label}
                            </Text>
                        </Frame>
                    }
                }</Show>
            </List>
        </CanvasItem>
    }
}

pub(crate) fn point_at(
    points: &[MapPoint],
    tip: impl Fn(&MapPoint) -> Pos2,
    position: Pos2,
) -> Option<Uuid> {
    points
        .iter()
        .rev()
        .find(|point| {
            let tip = tip(point);
            marker_rect(tip).expand(2.0).contains(position)
                || (tip - position).length() <= HIT_RADIUS
        })
        .map(|point| point.id)
}

fn marker_rect(tip: Pos2) -> Rect {
    Rect::from_min_max(
        Pos2::new(
            tip.x - MARKER_SIZE.icon_px() / 2.0,
            tip.y - MARKER_SIZE.icon_px(),
        ),
        Pos2::new(tip.x + MARKER_SIZE.icon_px() / 2.0, tip.y),
    )
}
