use block_editor_beui::be_block::map::{MapColor, MapPoint};
use block_editor_beui::beui::icons::ICON_LOCATION_ON;
use block_editor_beui::beui::reactive::{
    Align, CanvasItem, Frame, List, Prop, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Caption, IconSized};
use block_editor_beui::beui::{Color32, Pos2, Rect, TextAlign};
use uuid::Uuid;

const MARKER_SIZE: f32 = 26.0;
const RING_SIZE: f32 = MARKER_SIZE + 6.0;
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
    let unit = create_memo(clone!(scale -> move || 1.0 / scale.get().max(f32::EPSILON)));
    let x = create_memo(clone!(tip unit -> move || tip.get().x - LABEL_WIDTH / 2.0 * unit.get()));
    let y = create_memo(clone!(tip unit -> move || tip.get().y - RING_SIZE * unit.get()));
    let width = create_memo(clone!(unit -> move || LABEL_WIDTH * unit.get()));
    let height = create_memo(clone!(unit -> move || MARKER_HEIGHT * unit.get()));
    let named = create_memo(clone!(label -> move || !label.get().is_empty()));
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <List spacing=0.0 align=Align::Center>
                <Frame
                    width=RING_SIZE
                    height=RING_SIZE
                    radius={(RING_SIZE / 2.0) as u8}
                    outline=SELECTION_COLOR
                    outline_width=2.0
                    outline_visible={selected}
                >
                    <IconSized
                        glyph={ICON_LOCATION_ON.to_owned()}
                        font_size=MARKER_SIZE
                        color={color}
                    />
                </Frame>
                <Frame
                    visible={named}
                    color=LABEL_BACKGROUND
                    radius=3
                    padding_horizontal=3.0
                    height=LABEL_HEIGHT
                >
                    <Caption content={label} align=TextAlign::Center color=LABEL_COLOR />
                </Frame>
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
        Pos2::new(tip.x - MARKER_SIZE / 2.0, tip.y - MARKER_SIZE),
        Pos2::new(tip.x + MARKER_SIZE / 2.0, tip.y),
    )
}
