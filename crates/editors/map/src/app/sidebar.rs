use beui::Overflow;
use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::accesskit::{Node, Role};
use block_editor_beui::beui::icons::{ICON_ARROW_LEFT, ICON_CIRCLE, ICON_LOCATE_FIXED, ICON_TRASH};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::Pressable;
use block_editor_beui::{ChildBlock, ChildMode, ChildTarget};
use map_types::Map;
use map_types::{MAX_LATITUDE, MapColor, MapPoint};
use uuid::Uuid;

use crate::points::marker_color;

use super::state::MapState;
use block_editor_beui::beui::TextAlign;

const PREVIEW_HEIGHT: f32 = 130.0;

const COLOR_PRESETS: [(&str, MapColor); 5] = [
    ("Default", MapColor::Default),
    (
        "Orange",
        MapColor::Rgb {
            red: 240,
            green: 140,
            blue: 0,
        },
    ),
    (
        "Green",
        MapColor::Rgb {
            red: 47,
            green: 158,
            blue: 68,
        },
    ),
    (
        "Blue",
        MapColor::Rgb {
            red: 25,
            green: 113,
            blue: 194,
        },
    ),
    (
        "Violet",
        MapColor::Rgb {
            red: 121,
            green: 80,
            blue: 242,
        },
    ),
];

#[component]
pub(crate) fn MapSidebar(state: MapState) -> NodeId {
    let selected = create_memo(move || state.selected.get());
    let listing = create_memo(move || selected.get().is_none());
    let chosen = create_memo(move || selected.get().is_some());
    let error = state.import_error;
    let failed = create_memo(move || error.get().is_some());
    let reason = create_memo(move || error.get().unwrap_or_default());

    view! {
        <List spacing=10.0>
            <Text style=TextStyle::HEADING>"Map"</Text>
            <Separator />
            <RegionInspector state={state} />
            <Separator />
            <Show condition={listing}>
                <PointList state={state} />
            </Show>
            <Show condition={chosen}>
                <PointDetails state={state} selected={selected} />
            </Show>
            <Show condition={failed}>
                <List spacing=4.0>
                    <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
                    <Button
                        label="Dismiss"
                        variant=ButtonVariant::Secondary
                        on_click={move || state.dismiss_import_error()}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RegionInspector(state: MapState) -> NodeId {
    let region = state.preview_region;
    let fixed = create_memo(move || region.get().is_some());
    let loose = create_memo(move || !fixed.get());
    let loose_color = TextColor::Muted;
    view! {
        <List spacing=6.0>
            <Text>"Preview region"</Text>
            <Checkbox
                label="Show a fixed region"
                checked={Binding::new(fixed, move |on: bool| {
                    let region = on.then(|| state.visible_region.get_untracked());
                    state.record(Map::set_preview_region(region));
                })}
                @test_id={"map.preview-region"}
            />
            <Show condition={loose}>
                <Text style=TextStyle::CAPTION color={loose_color}>
                    "Previews show the whole world."
                </Text>
            </Show>
            <Show condition={fixed}>
                <List spacing=6.0>
                    <RegionEdges state={state} />
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Button
                            label="Use current view"
                            variant=ButtonVariant::Secondary
                            @test_id={"map.capture-region"}
                            on_click={move || {
                                let region = Some(state.visible_region.get_untracked());
                                state.record(Map::set_preview_region(region));
                            }}
                        />
                        <Button
                            label="Zoom to region"
                            variant=ButtonVariant::Secondary
                            on_click={move || state.request_fit()}
                        />
                    </List>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RegionEdges(state: MapState) -> NodeId {
    let edge = |label: &'static str,
                read: fn(&MapRegionEdges) -> f64,
                write: fn(&mut MapRegionEdges, f64)| {
        let value = create_memo(move || {
            state
                .preview_region
                .get()
                .map_or(0.0, |region| read(&MapRegionEdges::from(region)))
        });
        (label, value, move |next: f64| {
            let Some(region) = state.preview_region.get_untracked() else {
                return;
            };
            let mut edges = MapRegionEdges::from(region);
            write(&mut edges, next);
            state.record(Map::set_preview_region(Some(edges.into())));
        })
    };
    let (north_label, north, set_north) = edge(
        "North",
        |edges| edges.north,
        |edges, value| edges.north = value,
    );
    let (south_label, south, set_south) = edge(
        "South",
        |edges| edges.south,
        |edges, value| edges.south = value,
    );
    let (west_label, west, set_west) = edge(
        "West",
        |edges| edges.west,
        |edges, value| edges.west = value,
    );
    let (east_label, east, set_east) = edge(
        "East",
        |edges| edges.east,
        |edges, value| edges.east = value,
    );
    view! {
        <List spacing=4.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Coordinate
                    @sizing=ItemSize::Percent(50.0)
                    label={north_label}
                    value={north}
                    limit=MAX_LATITUDE
                    on_change={set_north}
                />
                <Coordinate
                    @sizing=ItemSize::Percent(50.0)
                    label={south_label}
                    value={south}
                    limit=MAX_LATITUDE
                    on_change={set_south}
                />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Coordinate
                    @sizing=ItemSize::Percent(50.0)
                    label={west_label}
                    value={west}
                    limit=180.0
                    on_change={set_west}
                />
                <Coordinate
                    @sizing=ItemSize::Percent(50.0)
                    label={east_label}
                    value={east}
                    limit=180.0
                    on_change={set_east}
                />
            </List>
        </List>
    }
}

struct MapRegionEdges {
    north: f64,
    south: f64,
    west: f64,
    east: f64,
}

impl From<map_types::MapRegion> for MapRegionEdges {
    fn from(region: map_types::MapRegion) -> Self {
        Self {
            north: region.north,
            south: region.south,
            west: region.west,
            east: region.east,
        }
    }
}

impl From<MapRegionEdges> for map_types::MapRegion {
    fn from(edges: MapRegionEdges) -> Self {
        Self::new(edges.west, edges.south, edges.east, edges.north)
    }
}

#[component]
fn Coordinate(
    label: &'static str,
    value: Memo<f64>,
    limit: f64,
    on_change: block_editor_beui::beui::reactive::Callback<f64>,
) -> NodeId {
    view! {
        <NumberInput
            value={Binding::new(value, move |next: f64| on_change.call(next))}
            min={-limit}
            max={limit}
            label={label.to_owned()}
        />
    }
}

#[component]
fn PointList(state: MapState) -> NodeId {
    let points = state.points;
    let ids = create_memo(move || {
        points
            .get()
            .iter()
            .map(|point| point.id)
            .collect::<Vec<Uuid>>()
    });
    let empty = create_memo(move || ids.get().is_empty());
    let any = create_memo(move || !empty.get());
    let hint = TextColor::Muted;
    view! {
        <List spacing=6.0>
            <Text>"Points of interest"</Text>
            <Show condition={empty}>
                <Text style=TextStyle::CAPTION color={hint}>
                    "Add blocks with the + button, by dragging them in from Files, or by pasting an image."
                </Text>
            </Show>
            <Show condition={any}>
                <Text style=TextStyle::CAPTION>"Select a marker to edit it."</Text>
            </Show>
            <ForEach keys={ids}>|id: Uuid|
                <PointRow state id />
            </ForEach>
        </List>
    }
}

#[component]
fn PointRow(state: MapState, id: Uuid) -> NodeId {
    let point = create_memo(move || state.points.get().into_iter().find(|point| point.id == id));
    let label = create_memo(move || point_name(&state, point.get()));
    let swatch = create_memo(move || {
        point
            .get()
            .map_or(Color32::TRANSPARENT, |point| marker_color(point.color))
    });
    let element_test_id = format!("map.point.{id}");
    view! {
        <ListRow
            @test_id={element_test_id}
            on_click={move || {
                state.select(Some(id));
                if let Some(point) = state.points.get_untracked().iter().find(|point| point.id == id) {
                    state.centre_on(point.position);
                }
            }}
            on_activate={move || {}}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>{label}</Text>
                <Swatch color={swatch} />
            </List>
        </ListRow>
    }
}

#[component]
fn Swatch(color: Memo<Color32>) -> NodeId {
    let color = create_memo(move || TextColor::Custom(color.get()));
    view! {
        <Text size=TextSize::Xs color={color} align=TextAlign::Center>
            <Icon glyph=ICON_CIRCLE />
        </Text>
    }
}

#[component]
fn PointDetails(state: MapState, selected: Memo<Option<Uuid>>) -> NodeId {
    let point = create_memo(move || {
        let id = selected.get()?;
        state.points.get().into_iter().find(|point| point.id == id)
    });
    let label = create_memo(move || point_name(&state, point.get()));
    let block = create_memo(move || {
        let point = point.get()?;
        let label = state.label_of(point.block_id).ok()?;
        Some(ChildTarget::new(point.block_id, label.block_type))
    });
    let openable = create_memo(move || block.get().is_none());
    let editor = state.editor();
    let latitude = create_memo(move || point.get().map_or(0.0, |point| point.position.latitude));
    let longitude = create_memo(move || point.get().map_or(0.0, |point| point.position.longitude));
    let custom = create_memo(move || {
        point
            .get()
            .map_or(Color32::TRANSPARENT, |point| marker_color(point.color))
    });

    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph=ICON_ARROW_LEFT
                    label="Back to all points"
                    @test_id={"map.point.back"}
                    on_click={move || state.select(None)}
                />
                <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>{label}</Text>
            </List>
            <Frame height=PREVIEW_HEIGHT radius=Radius::Sm>
                <ChildBlock
                    editor={editor}
                    block={block}
                    mode=ChildMode::Preview
                    on_state={move |_| {}}
                />
            </Frame>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Button
                    label="Edit"
                    variant=ButtonVariant::Secondary
                    disabled={openable}
                    on_click={move || {
                        if let Some(target) = block.get_untracked() {
                            state.editor().host().open_block(target.id, target.block_type);
                        }
                    }}
                />
                <IconButton
                    glyph=ICON_LOCATE_FIXED
                    label="Centre the map here"
                    on_click={move || {
                        if let Some(point) = point.get_untracked() {
                            state.centre_on(point.position);
                        }
                    }}
                />
                <IconButton
                    glyph=ICON_TRASH
                    label="Remove this point of interest"
                    @test_id={"map.point.remove"}
                    on_click={move || {
                        if let Some(point) = point.get_untracked() {
                            state.remove_point(point.id);
                        }
                    }}
                />
            </List>
            <Text>"Position"</Text>
            <NumberInput
                value={Binding::new(latitude, move |value: f64| {
                    let Some(mut point) = point.get_untracked() else {
                        return;
                    };
                    point.position.latitude = value;
                    state.record(Map::update(&[point]));
                })}
                min={-MAX_LATITUDE}
                max=MAX_LATITUDE
                label="Latitude"
            />
            <NumberInput
                value={Binding::new(longitude, move |value: f64| {
                    let Some(mut point) = point.get_untracked() else {
                        return;
                    };
                    point.position.longitude = value;
                    state.record(Map::update(&[point]));
                })}
                min=-180.0
                max=180.0
                label="Longitude"
            />
            <Text>"Colour"</Text>
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <ForEach
                    keys={COLOR_PRESETS.map(|(name, _)| name.to_owned()).to_vec()}
                >|name: String|
                    <ColorPreset state point={point} name />
                </ForEach>
            </List>
            <ColorInput
                value={Binding::new(custom, move |color: Color32| {
                    let Some(mut point) = point.get_untracked() else {
                        return;
                    };
                    let [red, green, blue, _] = color.to_array();
                    point.color = MapColor::Rgb { red, green, blue };
                    state.record(Map::update(&[point]));
                })}
                label="Custom colour"
            />
        </List>
    }
}

#[component]
fn ColorPreset(state: MapState, point: Memo<Option<MapPoint>>, name: String) -> NodeId {
    let Some((_, color)) = COLOR_PRESETS.into_iter().find(|(label, _)| *label == name) else {
        unreachable!("the preset was named by the list it came from");
    };
    let swatch = marker_color(color);
    let described = name.clone();
    view! {
        <Tooltip label={name}>
            <Pressable
                accessibility={preset_role(&described)}
                on_click={move || {
                    let Some(mut chosen) = point.get_untracked() else {
                        return;
                    };
                    chosen.color = color;
                    state.record(Map::update(&[chosen]));
                }}
            >
                <Text size=TextSize::Sm color={TextColor::Custom(swatch)} align=TextAlign::Center>
                    <Icon glyph=ICON_CIRCLE />
                </Text>
            </Pressable>
        </Tooltip>
    }
}

fn preset_role(name: &str) -> Prop<Node> {
    let mut node = Node::new(Role::Button);
    node.set_label(name.to_owned());
    Prop::Static(node)
}

fn point_name(state: &MapState, point: Option<MapPoint>) -> String {
    point.map_or_else(String::new, |point| state.name_of(point.block_id))
}
