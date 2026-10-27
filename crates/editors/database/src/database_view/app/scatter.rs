use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{NodeId, Pos2, TextAlign, Vec2};
use database_types::database::{DatabaseRow, DatabaseValue};
use database_types::database_schema::DatabaseField;
use uuid::Uuid;

use crate::database_view::app::data::Data;
use beui::ItemSize;

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
    let axes = axis_fields(data);
    let missing = create_memo(move || axes.get().is_none());
    let ready = create_memo(move || axes.get().is_some());
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=Space::None>
                <Show condition={missing}>
                    <Frame padding=Space::Lg>
                        <Text style=TextStyle::CAPTION>
                            "Choose X and Y fields to use the scatter view."
                        </Text>
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
    let size_held = create_signal(Vec2::ZERO);
    let size = size_held.read_only();
    let bounds = create_memo(move || {
        let (x_field, y_field) = axes.get()?;
        plot_bounds(&shown_rows(data), x_field.id, y_field.id)
    });
    let placed = create_memo(move || {
        let (Some((x_field, y_field)), Some(bounds)) = (axes.get(), bounds.get()) else {
            return Vec::new();
        };
        let area = size.get();
        place_points(
            &shown_rows(data),
            x_field.id,
            y_field.id,
            bounds,
            plot_area(area),
        )
    });
    let keys = create_memo(move || {
        placed.with(|placed| placed.iter().map(|point| point.row).collect::<Vec<usize>>())
    });
    let empty = create_memo(move || placed.with(Vec::is_empty));
    let plot_height = create_memo(move || plot_area(size.get()).1);
    let plot_width = create_memo(move || plot_area(size.get()).0);
    let rule_top = create_memo(move || AXIS_TOP + plot_height.get());
    let x_low = create_memo(move || {
        bounds
            .get()
            .map_or_else(String::new, |bounds| format_axis_value(bounds.0))
    });
    let x_high = create_memo(move || {
        bounds
            .get()
            .map_or_else(String::new, |bounds| format_axis_value(bounds.1))
    });
    let y_low = create_memo(move || {
        bounds
            .get()
            .map_or_else(String::new, |bounds| format_axis_value(bounds.2))
    });
    let y_high = create_memo(move || {
        bounds
            .get()
            .map_or_else(String::new, |bounds| format_axis_value(bounds.3))
    });
    let x_name = create_memo(move || {
        axes.get()
            .map_or_else(String::new, |(x_field, _)| x_field.name)
    });
    let y_name = create_memo(move || {
        axes.get()
            .map_or_else(String::new, |(_, y_field)| y_field.name)
    });
    let x_label_x = create_memo(move || AXIS_LEFT + plot_width.get() * 0.5 - 60.0);
    let x_high_x = create_memo(move || AXIS_LEFT + plot_width.get() - 60.0);
    let x_label_y = create_memo(move || rule_top.get() + LABEL_HEIGHT);
    let value_y = create_memo(move || rule_top.get() + 2.0);
    let y_low_y = create_memo(move || rule_top.get() - LABEL_HEIGHT);
    let pick = move |at: Pos2| {
        let chosen = placed.with_untracked(|placed| nearest(placed, at));
        match chosen {
            Some(row) => data.select(row, None),
            None => data.deselect(),
        }
    };
    let built = view! {
        <Interactive
            @test_id={"database-view.scatter"}
            on_click_at={move |press: block_editor_beui::beui::PointerPress| pick(press.local)}
        >
            <Canvas>
                <CanvasItem x=AXIS_LEFT y=AXIS_TOP width=RULE height={plot_height}>
                    <Frame fill=ColorRole::Border />
                </CanvasItem>
                <CanvasItem x=AXIS_LEFT y={rule_top} width={plot_width} height=RULE>
                    <Frame fill=ColorRole::Border />
                </CanvasItem>
                <CanvasItem x=AXIS_LEFT y={value_y} width=60.0 height=LABEL_HEIGHT>
                    <Text style=TextStyle::CAPTION>{x_low}</Text>
                </CanvasItem>
                <CanvasItem x={x_high_x} y={value_y} width=60.0 height=LABEL_HEIGHT>
                    <Text style=TextStyle::CAPTION align=TextAlign::End>{x_high}</Text>
                </CanvasItem>
                <CanvasItem x={x_label_x} y={x_label_y} width=120.0 height=LABEL_HEIGHT>
                    <Text style=TextStyle::CAPTION align=TextAlign::Center>{x_name}</Text>
                </CanvasItem>
                <CanvasItem x=0.0 y={y_low_y} width={AXIS_LEFT - 4.0} height=LABEL_HEIGHT>
                    <Text style=TextStyle::CAPTION align=TextAlign::End>{y_low}</Text>
                </CanvasItem>
                <CanvasItem x=0.0 y=AXIS_TOP width={AXIS_LEFT - 4.0} height=LABEL_HEIGHT>
                    <Text style=TextStyle::CAPTION align=TextAlign::End>{y_high}</Text>
                </CanvasItem>
                <CanvasItem x=0.0 y=0.0 width={AXIS_LEFT * 2.0} height=LABEL_HEIGHT>
                    <Text style=TextStyle::CAPTION>{y_name}</Text>
                </CanvasItem>
                <ForEach keys={keys}>|row: usize|
                    <Dot placed={placed} data={data} row={row} />
                </ForEach>
                <Show condition={empty}>
                    <CanvasItem x=AXIS_LEFT y=AXIS_TOP width=260.0 height=LABEL_HEIGHT>
                        <Text style=TextStyle::CAPTION>"No rows have both fields set"</Text>
                    </CanvasItem>
                </Show>
            </Canvas>
        </Interactive>
    };
    let size_measured = measure(built);
    create_effect(move || {
        if let Some(measured) = size_measured.get() {
            size_held.set(measured.size);
        }
    });
    built
}

#[component]
fn Dot(placed: Memo<Vec<Point>>, data: Data, row: usize) -> CanvasItem {
    let point = create_memo(move || {
        placed.with(|placed| placed.iter().find(|point| point.row == row).copied())
    });
    let selected = create_memo(move || {
        data.selected
            .get()
            .is_some_and(|selection| selection.row == row)
    });
    let side = create_memo(move || match selected.get() {
        true => SELECTED_POINT,
        false => POINT,
    });
    let x = create_memo(move || point.get().map_or(0.0, |point| point.x - side.get() * 0.5));
    let y = create_memo(move || point.get().map_or(0.0, |point| point.y - side.get() * 0.5));
    let color = create_memo(move || match selected.get() {
        true => ColorRole::Accent,
        false => ColorRole::Text,
    });
    view! {
        <CanvasItem x={x} y={y} width={side} height={side}>
            <Frame
                @test_id={format!("database-view.point.{row}")}
                fill={color}
                radius=Radius::Full
            />
        </CanvasItem>
    }
}

pub fn axis_fields(data: Data) -> Memo<Option<(DatabaseField, DatabaseField)>> {
    let fields = data.fields;
    let x = data.scatter_x;
    let y = data.scatter_y;
    create_memo(move || {
        let (x, y) = (x.get()?, y.get()?);
        fields.with(|fields| {
            let x = fields.iter().find(|field| field.id == x)?;
            let y = fields.iter().find(|field| field.id == y)?;
            Some((x.clone(), y.clone()))
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

fn shown_rows(data: Data) -> Vec<DatabaseRow> {
    data.rows.with(|rows| {
        rows.iter()
            .map(|row| match data.shows(row) {
                true => row.clone(),
                false => DatabaseRow::default(),
            })
            .collect()
    })
}
