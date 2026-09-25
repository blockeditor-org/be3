use block_editor_beui::be_block::database::{DatabaseRow, DatabaseValue};
use block_editor_beui::be_block::database_schema::{DatabaseField, DatabaseFieldType};
use block_editor_beui::beui::reactive::{
    Canvas, CanvasItem, ClickCatcher, ForEach, Frame, ItemSize, List, Memo, Show, clone, component,
    component_rect, component_size, create_memo, view,
};
use block_editor_beui::beui::styled::{Caption, use_theme};
use block_editor_beui::beui::{NodeId, Pos2, TextAlign};
use uuid::Uuid;

use crate::app::data::Data;

const AXIS_LEFT: f32 = 48.0;
const AXIS_BOTTOM: f32 = 32.0;
const AXIS_TOP: f32 = 12.0;
const AXIS_RIGHT: f32 = 12.0;
const RULE: f32 = 1.0;
const POINT: f32 = 8.0;
const SELECTED_POINT: f32 = 14.0;
const HIT_RADIUS: f32 = 12.0;
const LABEL_HEIGHT: f32 = 16.0;

#[derive(Clone, Copy, PartialEq)]
struct Point {
    row: usize,
    x: f32,
    y: f32,
}

#[component]
pub fn Scatter(data: Data) -> NodeId {
    let axes = axis_fields(&data);
    let missing = create_memo(clone!(axes -> move || axes.get().is_none()));
    let ready = create_memo(clone!(axes -> move || axes.get().is_some()));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Show condition={missing}>
                    <Frame padding_horizontal=12.0 padding_vertical=12.0>
                        <Caption content="Choose X and Y fields to use the scatter view." />
                    </Frame>
                </Show>
                <Show condition={ready}>
                    <Plot @sizing=ItemSize::Percent(100.0) data={data} axes={axes} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Plot(data: Data, axes: Memo<Option<(DatabaseField, DatabaseField)>>) -> NodeId {
    let size = component_size();
    let rect = component_rect();
    let bounds = create_memo(clone!(data axes -> move || {
        let (x_field, y_field) = axes.get()?;
        data.rows.with(|rows| plot_bounds(rows, x_field.id, y_field.id))
    }));
    let placed = create_memo(clone!(data axes bounds size -> move || {
        let (Some((x_field, y_field)), Some(bounds)) = (axes.get(), bounds.get()) else {
            return Vec::new();
        };
        let area = size.get();
        data.rows.with(|rows| {
            place_points(rows, x_field.id, y_field.id, bounds, plot_area(area))
        })
    }));
    let keys = create_memo(clone!(placed -> move || {
        placed.with(|placed| placed.iter().map(|point| point.row).collect::<Vec<usize>>())
    }));
    let empty = create_memo(clone!(placed -> move || placed.with(Vec::is_empty)));
    let plot_height = create_memo(clone!(size -> move || plot_area(size.get()).1));
    let plot_width = create_memo(clone!(size -> move || plot_area(size.get()).0));
    let rule_top = create_memo(clone!(plot_height -> move || AXIS_TOP + plot_height.get()));
    let x_low = create_memo(clone!(bounds -> move || {
        bounds.get().map_or_else(String::new, |bounds| format_axis_value(bounds.0))
    }));
    let x_high = create_memo(clone!(bounds -> move || {
        bounds.get().map_or_else(String::new, |bounds| format_axis_value(bounds.1))
    }));
    let y_low = create_memo(clone!(bounds -> move || {
        bounds.get().map_or_else(String::new, |bounds| format_axis_value(bounds.2))
    }));
    let y_high = create_memo(clone!(bounds -> move || {
        bounds.get().map_or_else(String::new, |bounds| format_axis_value(bounds.3))
    }));
    let x_name = create_memo(clone!(axes -> move || {
        axes.get().map_or_else(String::new, |(x_field, _)| x_field.name)
    }));
    let y_name = create_memo(clone!(axes -> move || {
        axes.get().map_or_else(String::new, |(_, y_field)| y_field.name)
    }));
    let x_label_x =
        create_memo(clone!(plot_width -> move || AXIS_LEFT + plot_width.get() * 0.5 - 60.0));
    let x_high_x = create_memo(clone!(plot_width -> move || AXIS_LEFT + plot_width.get() - 60.0));
    let x_label_y = create_memo(clone!(rule_top -> move || rule_top.get() + LABEL_HEIGHT));
    let value_y = create_memo(clone!(rule_top -> move || rule_top.get() + 2.0));
    let y_low_y = create_memo(clone!(rule_top -> move || rule_top.get() - LABEL_HEIGHT));
    let pick = clone!(data placed rect -> move |at: Pos2| {
        let origin = rect.get_untracked().min;
        let at = Pos2::new(at.x - origin.x, at.y - origin.y);
        let chosen = placed.with_untracked(|placed| nearest(placed, at));
        match chosen {
            Some(row) => data.select(row, None),
            None => data.deselect(),
        }
    });
    let theme = use_theme();
    let axis_color = theme.border.clone();
    let rule_color = theme.border.clone();
    view! {
        <ClickCatcher
            @test_id={"database-view.scatter"}
            on_click_at={move |press: block_editor_beui::beui::PointerPress| pick(press.pos)}
        >
            <Canvas>
                <CanvasItem x=AXIS_LEFT y=AXIS_TOP width=RULE height={plot_height}>
                    <Frame color={axis_color} />
                </CanvasItem>
                <CanvasItem x=AXIS_LEFT y={rule_top} width={plot_width} height=RULE>
                    <Frame color={rule_color} />
                </CanvasItem>
                <CanvasItem x=AXIS_LEFT y={value_y.clone()} width=60.0 height=LABEL_HEIGHT>
                    <Caption content={x_low} />
                </CanvasItem>
                <CanvasItem x={x_high_x} y={value_y} width=60.0 height=LABEL_HEIGHT>
                    <Caption content={x_high} align=TextAlign::End />
                </CanvasItem>
                <CanvasItem x={x_label_x} y={x_label_y} width=120.0 height=LABEL_HEIGHT>
                    <Caption content={x_name} align=TextAlign::Center />
                </CanvasItem>
                <CanvasItem x=0.0 y={y_low_y} width={AXIS_LEFT - 4.0} height=LABEL_HEIGHT>
                    <Caption content={y_low} align=TextAlign::End />
                </CanvasItem>
                <CanvasItem x=0.0 y=AXIS_TOP width={AXIS_LEFT - 4.0} height=LABEL_HEIGHT>
                    <Caption content={y_high} align=TextAlign::End />
                </CanvasItem>
                <CanvasItem x=0.0 y=0.0 width={AXIS_LEFT * 2.0} height=LABEL_HEIGHT>
                    <Caption content={y_name} />
                </CanvasItem>
                <ForEach keys={keys}>
                    {move |row: usize| {
                        let placed = placed.clone();
                        let data = data.clone();
                        view! {
                            <Dot placed={placed} data={data} row={row} />
                        }
                    }}
                </ForEach>
                <Show condition={empty}>
                    <CanvasItem x=AXIS_LEFT y=AXIS_TOP width=260.0 height=LABEL_HEIGHT>
                        <Caption content="No rows have both fields set" />
                    </CanvasItem>
                </Show>
            </Canvas>
        </ClickCatcher>
    }
}

#[component]
fn Dot(placed: Memo<Vec<Point>>, data: Data, row: usize) -> CanvasItem {
    let point = create_memo(clone!(placed -> move || {
        placed.with(|placed| placed.iter().find(|point| point.row == row).copied())
    }));
    let selected = create_memo(clone!(data -> move || {
        data.selected.get().is_some_and(|selection| selection.row == row)
    }));
    let side = create_memo(clone!(selected -> move || match selected.get() {
        true => SELECTED_POINT,
        false => POINT,
    }));
    let x = create_memo(clone!(point side -> move || {
        point.get().map_or(0.0, |point| point.x - side.get() * 0.5)
    }));
    let y = create_memo(clone!(point side -> move || {
        point.get().map_or(0.0, |point| point.y - side.get() * 0.5)
    }));
    let width = side.clone();
    let theme = use_theme();
    let color = create_memo(clone!(theme selected -> move || match selected.get() {
        true => theme.accent.get(),
        false => theme.text.get(),
    }));
    let radius = create_memo(clone!(side -> move || (side.get() * 0.5) as u8));
    view! {
        <CanvasItem x={x} y={y} width={width} height={side}>
            <Frame @test_id={format!("database-view.point.{row}")} color={color} radius={radius} />
        </CanvasItem>
    }
}

pub fn axis_fields(data: &Data) -> Memo<Option<(DatabaseField, DatabaseField)>> {
    let fields = data.fields.clone();
    let x = data.scatter_x.clone();
    let y = data.scatter_y.clone();
    create_memo(move || {
        let (x, y) = (x.get()?, y.get()?);
        fields.with(|fields| {
            let x = fields.iter().find(|field| field.id == x)?;
            let y = fields.iter().find(|field| field.id == y)?;
            Some((x.clone(), y.clone()))
        })
    })
}

pub fn number_fields(data: &Data) -> Memo<Vec<DatabaseField>> {
    let fields = data.fields.clone();
    create_memo(move || {
        fields.with(|fields| {
            fields
                .iter()
                .filter(|field| field.field_type == DatabaseFieldType::Number)
                .cloned()
                .collect()
        })
    })
}

fn plot_area(size: block_editor_beui::beui::Vec2) -> (f32, f32) {
    (
        (size.x - AXIS_LEFT - AXIS_RIGHT).max(1.0),
        (size.y - AXIS_TOP - AXIS_BOTTOM).max(1.0),
    )
}

fn number_value(row: &DatabaseRow, field_id: Uuid) -> Option<f64> {
    match row.value(field_id) {
        Some(DatabaseValue::Number(value)) => Some(*value),
        _ => None,
    }
}

fn plot_bounds(rows: &[DatabaseRow], x: Uuid, y: Uuid) -> Option<(f64, f64, f64, f64)> {
    let values: Vec<(f64, f64)> = rows
        .iter()
        .filter_map(|row| Some((number_value(row, x)?, number_value(row, y)?)))
        .collect();
    if values.is_empty() {
        return None;
    }
    let (x_min, x_max) = bounds(values.iter().map(|(x, _)| *x));
    let (y_min, y_max) = bounds(values.iter().map(|(_, y)| *y));
    Some((x_min, x_max, y_min, y_max))
}

fn place_points(
    rows: &[DatabaseRow],
    x_field: Uuid,
    y_field: Uuid,
    (x_min, x_max, y_min, y_max): (f64, f64, f64, f64),
    (width, height): (f32, f32),
) -> Vec<Point> {
    rows.iter()
        .enumerate()
        .filter_map(|(row, values)| {
            let x = number_value(values, x_field)?;
            let y = number_value(values, y_field)?;
            Some(Point {
                row,
                x: AXIS_LEFT + normalize(x, x_min, x_max) as f32 * width,
                y: AXIS_TOP + height - normalize(y, y_min, y_max) as f32 * height,
            })
        })
        .collect()
}

fn nearest(points: &[Point], at: Pos2) -> Option<usize> {
    points
        .iter()
        .map(|point| {
            let distance = ((point.x - at.x).powi(2) + (point.y - at.y).powi(2)).sqrt();
            (point.row, distance)
        })
        .filter(|(_, distance)| *distance <= HIT_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(row, _)| row)
}

fn bounds(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for value in values {
        min = min.min(value);
        max = max.max(value);
    }
    if min >= max {
        (min - 1.0, max + 1.0)
    } else {
        (min, max)
    }
}

fn normalize(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        0.5
    } else {
        (value - min) / (max - min)
    }
}

fn format_axis_value(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}
