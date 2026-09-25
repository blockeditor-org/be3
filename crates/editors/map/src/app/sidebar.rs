use std::rc::Rc;

use block_editor_plugin::be_block::Map;
use block_editor_plugin::be_block::map::{MAX_LATITUDE, MapColor, MapPoint};
use block_editor_plugin::beui::Color32;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::accesskit::{Node, Role};
use block_editor_plugin::beui::icons::{
    ICON_ARROW_BACK, ICON_CIRCLE, ICON_DELETE, ICON_MY_LOCATION,
};
use block_editor_plugin::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Prop, Show, clone, component,
    create_memo, view,
};
use block_editor_plugin::beui::styled::{
    Body, Button, ButtonVariant, Caption, Checkbox, ColorInput, Heading, IconButton, IconSized,
    ListRow, NumberInput, Separator, Tooltip, use_theme,
};
use block_editor_plugin::beui::unstyled::Pressable;
use block_editor_plugin::{ChildBlock, ChildMode, ChildTarget};
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
pub(crate) fn MapSidebar(state: Rc<MapState>) -> NodeId {
    let selected = create_memo(clone!(state -> move || state.selected.get()));
    let listing = create_memo(clone!(selected -> move || selected.get().is_none()));
    let chosen = create_memo(clone!(selected -> move || selected.get().is_some()));
    let region = Rc::clone(&state);
    let list = Rc::clone(&state);
    let details = Rc::clone(&state);
    let dismiss = Rc::clone(&state);
    let error = state.import_error.clone();
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    let reason = create_memo(clone!(error -> move || error.get().unwrap_or_default()));
    let theme = use_theme();

    view! {
        <List spacing=10.0>
            <Heading content="Map" />
            <Separator />
            <RegionInspector state={region} />
            <Separator />
            <Show condition={listing}>
                <PointList state={list} />
            </Show>
            <Show condition={chosen}>
                <PointDetails state={details} selected={selected} />
            </Show>
            <Show condition={failed}>
                <List spacing=4.0>
                    <Caption content={reason} color={theme.danger.clone()} />
                    <Button
                        label="Dismiss"
                        variant=ButtonVariant::Secondary
                        on_click={move || dismiss.dismiss_import_error()}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RegionInspector(state: Rc<MapState>) -> NodeId {
    let region = state.preview_region.clone();
    let fixed = create_memo(clone!(region -> move || region.get().is_some()));
    let loose = create_memo(clone!(fixed -> move || !fixed.get()));
    let toggled = Rc::clone(&state);
    let captured = Rc::clone(&state);
    let zoomed = Rc::clone(&state);
    let theme = use_theme();
    let loose_color = theme.text_muted.clone();
    view! {
        <List spacing=6.0>
            <Body content="Preview region" />
            <Checkbox
                label="Show a fixed region"
                checked={fixed.clone()}
                @test_id={"map.preview-region"}
                on_change={move |on: bool| {
                    let region = on.then(|| toggled.visible_region.get_untracked());
                    toggled.record(Map::set_preview_region(region));
                }}
            />
            <Show condition={loose}>
                <Caption content="Previews show the whole world." color={loose_color} />
            </Show>
            <Show condition={fixed}>
                <List spacing=6.0>
                    <RegionEdges state={Rc::clone(&state)} />
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Button
                            label="Use current view"
                            variant=ButtonVariant::Secondary
                            @test_id={"map.capture-region"}
                            on_click={move || {
                                let region = Some(captured.visible_region.get_untracked());
                                captured.record(Map::set_preview_region(region));
                            }}
                        />
                        <Button
                            label="Zoom to region"
                            variant=ButtonVariant::Secondary
                            on_click={move || zoomed.request_fit()}
                        />
                    </List>
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RegionEdges(state: Rc<MapState>) -> NodeId {
    let edge = |state: &Rc<MapState>,
                label: &'static str,
                read: fn(&MapRegionEdges) -> f64,
                write: fn(&mut MapRegionEdges, f64)| {
        let read_state = Rc::clone(state);
        let value = create_memo(move || {
            read_state
                .preview_region
                .get()
                .map_or(0.0, |region| read(&MapRegionEdges::from(region)))
        });
        let write_state = Rc::clone(state);
        (label, value, move |next: f64| {
            let Some(region) = write_state.preview_region.get_untracked() else {
                return;
            };
            let mut edges = MapRegionEdges::from(region);
            write(&mut edges, next);
            write_state.record(Map::set_preview_region(Some(edges.into())));
        })
    };
    let (north_label, north, set_north) = edge(
        &state,
        "North",
        |edges| edges.north,
        |edges, value| edges.north = value,
    );
    let (south_label, south, set_south) = edge(
        &state,
        "South",
        |edges| edges.south,
        |edges, value| edges.south = value,
    );
    let (west_label, west, set_west) = edge(
        &state,
        "West",
        |edges| edges.west,
        |edges, value| edges.west = value,
    );
    let (east_label, east, set_east) = edge(
        &state,
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

impl From<block_editor_plugin::be_block::map::MapRegion> for MapRegionEdges {
    fn from(region: block_editor_plugin::be_block::map::MapRegion) -> Self {
        Self {
            north: region.north,
            south: region.south,
            west: region.west,
            east: region.east,
        }
    }
}

impl From<MapRegionEdges> for block_editor_plugin::be_block::map::MapRegion {
    fn from(edges: MapRegionEdges) -> Self {
        Self::new(edges.west, edges.south, edges.east, edges.north)
    }
}

#[component]
fn Coordinate(
    label: &'static str,
    value: Memo<f64>,
    limit: f64,
    on_change: block_editor_plugin::beui::reactive::Callback<f64>,
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
fn PointList(state: Rc<MapState>) -> NodeId {
    let points = state.points.clone();
    let ids = create_memo(clone!(points -> move || {
        points.get().iter().map(|point| point.id).collect::<Vec<Uuid>>()
    }));
    let empty = create_memo(clone!(ids -> move || ids.get().is_empty()));
    let any = create_memo(clone!(empty -> move || !empty.get()));
    let theme = use_theme();
    let hint = theme.text_muted.clone();
    view! {
        <List spacing=6.0>
            <Body content="Points of interest" />
            <Show condition={empty}>
                <Caption
                    content="Add blocks with the + button, by dragging them in from Files, or by pasting an image."
                    color={hint}
                />
            </Show>
            <Show condition={any}>
                <Caption content="Select a marker to edit it." color={theme.text_muted.clone()} />
            </Show>
            <ForEach keys={ids}>
                {move |id: Uuid| {
                    let state = Rc::clone(&state);
                    view! {
                        <PointRow state id />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn PointRow(state: Rc<MapState>, id: Uuid) -> NodeId {
    let named = Rc::clone(&state);
    let point = create_memo(clone!(named -> move || {
        named.points.get().into_iter().find(|point| point.id == id)
    }));
    let label = create_memo(clone!(point named -> move || point_name(&named, point.get())));
    let swatch = create_memo(clone!(point -> move || {
        point.get().map_or(Color32::TRANSPARENT, |point| marker_color(point.color))
    }));
    let chosen = Rc::clone(&state);
    let centred = Rc::clone(&state);
    let test_id = format!("map.point.{id}");
    view! {
        <ListRow
            @test_id={test_id}
            on_click={move || {
                chosen.select(Some(id));
                if let Some(point) = centred.points.get_untracked().iter().find(|point| point.id == id) {
                    centred.centre_on(point.position);
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
fn PointDetails(state: Rc<MapState>, selected: Memo<Option<Uuid>>) -> NodeId {
    let held = Rc::clone(&state);
    let point = create_memo(clone!(selected held -> move || {
        let id = selected.get()?;
        held.points.get().into_iter().find(|point| point.id == id)
    }));
    let named = Rc::clone(&state);
    let label = create_memo(clone!(point named -> move || point_name(&named, point.get())));
    let target = Rc::clone(&state);
    let block = create_memo(clone!(point target -> move || {
        let point = point.get()?;
        let label = target.label_of(point.block_id)?;
        Some(ChildTarget::new(point.block_id, label.block_type))
    }));
    let openable = create_memo(clone!(block -> move || block.get().is_none()));
    let back = Rc::clone(&state);
    let opened = Rc::clone(&state);
    let opened_block = block.clone();
    let centred = Rc::clone(&state);
    let centred_point = point.clone();
    let removed = Rc::clone(&state);
    let editor = state.editor().clone();
    let latitude = create_memo(clone!(point -> move || {
        point.get().map_or(0.0, |point| point.position.latitude)
    }));
    let longitude = create_memo(clone!(point -> move || {
        point.get().map_or(0.0, |point| point.position.longitude)
    }));
    let moved = Rc::clone(&state);
    let moved_point = point.clone();
    let slid = Rc::clone(&state);
    let slid_point = point.clone();
    let tinted = Rc::clone(&state);
    let tinted_point = point.clone();
    let coloured = Rc::clone(&state);
    let coloured_point = point.clone();
    let custom = create_memo(clone!(point -> move || {
        point.get().map_or(Color32::TRANSPARENT, |point| marker_color(point.color))
    }));

    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <IconButton
                    glyph={ICON_ARROW_BACK.to_owned()}
                    label="Back to all points"
                    @test_id={"map.point.back"}
                    on_click={move || back.select(None)}
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
                            opened.editor().host().open_block(target.id, target.block_type);
                        }
                    }}
                />
                <IconButton
                    glyph={ICON_MY_LOCATION.to_owned()}
                    label="Centre the map here"
                    on_click={move || {
                        if let Some(point) = centred_point.get_untracked() {
                            centred.centre_on(point.position);
                        }
                    }}
                />
                <IconButton
                    glyph={ICON_DELETE.to_owned()}
                    label="Remove this point of interest"
                    @test_id={"map.point.remove"}
                    on_click={move || {
                        if let Some(point) = point.get_untracked() {
                            removed.remove_point(point.id);
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
                    moved.record(Map::update(&[point]));
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
                    slid.record(Map::update(&[point]));
                }}
            />
            <Body content="Colour" />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <ForEach keys={COLOR_PRESETS.map(|(name, _)| name.to_owned()).to_vec()}>
                    {move |name: String| {
                        let state = Rc::clone(&coloured);
                        let point = coloured_point.clone();
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
                    tinted.record(Map::update(&[point]));
                }}
            />
        </List>
    }
}

#[component]
fn ColorPreset(state: Rc<MapState>, point: Memo<Option<MapPoint>>, name: String) -> NodeId {
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
