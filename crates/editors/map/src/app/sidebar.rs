use block_editor_beui::be_block::Map;
use block_editor_beui::be_block::map::{MAX_LATITUDE, MapColor, MapPoint};
use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::accesskit::{Node, Role};
use block_editor_beui::beui::icons::{ICON_ARROW_BACK, ICON_CIRCLE, ICON_DELETE, ICON_MY_LOCATION};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Prop, Show, component, create_memo,
    view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Checkbox, ColorInput, Heading, IconButton, IconSized,
    ListRow, NumberInput, Separator, Tooltip, use_theme,
};
use block_editor_beui::beui::unstyled::Pressable;
use block_editor_beui::{ChildBlock, ChildMode, ChildTarget};
use uuid::Uuid;

use crate::points::marker_color;

use super::state::MapState;

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
    let theme = use_theme();

    view! {
        <List spacing=10.0>
            <Heading content="Map" />
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
                {move || view! {
                    <List spacing=4.0>
                        <Caption content={reason} color={theme.danger} />
                        <Button
                            label="Dismiss"
                            variant=ButtonVariant::Secondary
                            on_click={move || state.dismiss_import_error()}
                        />
                    </List>
                }}
            </Show>
        </List>
    }
}

#[component]
fn RegionInspector(state: MapState) -> NodeId {
    let region = state.preview_region;
    let fixed = create_memo(move || region.get().is_some());
    let loose = create_memo(move || !fixed.get());
    let theme = use_theme();
    let loose_color = theme.text_muted;
    view! {
        <List spacing=6.0>
            <Body content="Preview region" />
            <Checkbox
                label="Show a fixed region"
                checked={fixed}
                @test_id={"map.preview-region"}
                on_change={move |on: bool| {
                    let region = on.then(|| state.visible_region.get_untracked());
                    state.record(Map::set_preview_region(region));
                }}
            />
            <Show condition={loose}>
                <Caption content="Previews show the whole world." color={loose_color} />
            </Show>
            <Show condition={fixed}>
                {move || view! {
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
                }}
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

impl From<block_editor_beui::be_block::map::MapRegion> for MapRegionEdges {
    fn from(region: block_editor_beui::be_block::map::MapRegion) -> Self {
        Self {
            north: region.north,
            south: region.south,
            west: region.west,
            east: region.east,
        }
    }
}

impl From<MapRegionEdges> for block_editor_beui::be_block::map::MapRegion {
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
            value={value}
            min={-limit}
            max={limit}
            label={label.to_owned()}
            on_change={move |next: f64| on_change.call(next)}
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
    let theme = use_theme();
    let hint = theme.text_muted;
    view! {
        <List spacing=6.0>
            <Body content="Points of interest" />
            <Show condition={empty}>
                <Caption
                    content="Add blocks with the + button, by dragging them in from Files, or by pasting an image."
                    wrap=true
                    color={hint}
                />
            </Show>
            <Show condition={any}>
                <Caption content="Select a marker to edit it." color={theme.text_muted} />
            </Show>
            <ForEach keys={ids}>
                {move |id: Uuid| {
                    view! {
                        <PointRow state id />
                    }
                }}
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
    let test_id = format!("map.point.{id}");
    view! {
        <ListRow
            @test_id={test_id}
            on_click={move || {
                state.select(Some(id));
                if let Some(point) = state.points.get_untracked().iter().find(|point| point.id == id) {
                    state.centre_on(point.position);
                }
            }}
            on_activate={move || {}}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Body @sizing=ItemSize::Percent(100.0) content={label} />
                <Swatch color={swatch} />
            </List>
        </ListRow>
    }
}

#[component]
fn Swatch(color: Memo<Color32>) -> NodeId {
    view! {
        <IconSized glyph={ICON_CIRCLE.to_owned()} font_size=12.0 color={color} />
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
        let label = state.label_of(point.block_id)?;
        Some(ChildTarget::new(point.block_id, label.block_type))
    });
    let openable = create_memo(move || block.get().is_none());
    let opened_block = block;
    let centred_point = point;
    let editor = state.editor();
    let latitude = create_memo(move || point.get().map_or(0.0, |point| point.position.latitude));
    let longitude = create_memo(move || point.get().map_or(0.0, |point| point.position.longitude));
    let moved_point = point;
    let slid_point = point;
    let tinted_point = point;
    let coloured_point = point;
    let custom = create_memo(move || {
        point
            .get()
            .map_or(Color32::TRANSPARENT, |point| marker_color(point.color))
    });

    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_ARROW_BACK.to_owned()}
                    label="Back to all points"
                    @test_id={"map.point.back"}
                    on_click={move || state.select(None)}
                />
                <Body @sizing=ItemSize::Percent(100.0) content={label} />
            </List>
            <Frame height=PREVIEW_HEIGHT radius=4>
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
                        if let Some(target) = opened_block.get_untracked() {
                            state.editor().host().open_block(target.id, target.block_type);
                        }
                    }}
                />
                <IconButton
                    glyph={ICON_MY_LOCATION.to_owned()}
                    label="Centre the map here"
                    on_click={move || {
                        if let Some(point) = centred_point.get_untracked() {
                            state.centre_on(point.position);
                        }
                    }}
                />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Remove this point of interest"
                    @test_id={"map.point.remove"}
                    on_click={move || {
                        if let Some(point) = point.get_untracked() {
                            state.remove_point(point.id);
                        }
                    }}
                />
            </List>
            <Body content="Position" />
            <NumberInput
                value={latitude}
                min={-MAX_LATITUDE}
                max=MAX_LATITUDE
                label="Latitude"
                on_change={move |value: f64| {
                    let Some(mut point) = moved_point.get_untracked() else {
                        return;
                    };
                    point.position.latitude = value;
                    state.record(Map::update(&[point]));
                }}
            />
            <NumberInput
                value={longitude}
                min=-180.0
                max=180.0
                label="Longitude"
                on_change={move |value: f64| {
                    let Some(mut point) = slid_point.get_untracked() else {
                        return;
                    };
                    point.position.longitude = value;
                    state.record(Map::update(&[point]));
                }}
            />
            <Body content="Colour" />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <ForEach keys={COLOR_PRESETS.map(|(name, _)| name.to_owned()).to_vec()}>
                    {move |name: String| {
                        let point = coloured_point;
                        view! {
                            <ColorPreset state point name />
                        }
                    }}
                </ForEach>
            </List>
            <ColorInput
                value={custom}
                label="Custom colour"
                on_change={move |color: Color32| {
                    let Some(mut point) = tinted_point.get_untracked() else {
                        return;
                    };
                    let [red, green, blue, _] = color.to_array();
                    point.color = MapColor::Rgb { red, green, blue };
                    state.record(Map::update(&[point]));
                }}
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
                <IconSized glyph={ICON_CIRCLE.to_owned()} font_size=16.0 color={swatch} />
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
    let Some(point) = point else {
        return String::new();
    };
    match state.label_of(point.block_id) {
        Some(label) => label.name,
        None => "Loading…".to_owned(),
    }
}
