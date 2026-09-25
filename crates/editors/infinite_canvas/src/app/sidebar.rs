use std::rc::Rc;

use block_editor_beui::be_block::canvas::{
    CanvasColor, CanvasEntity, CanvasEntityKind, CanvasLayerMove, CanvasPoint, CanvasPreviewRegion,
    CanvasTextAlign, CanvasTextWeight,
};
use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_CIRCLE, ICON_FORMAT_COLOR_RESET};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Memo, Show, Spacer, clone, component, create_memo,
    create_signal, view,
};
use block_editor_beui::beui::styled::{
    Accordion, Button, ButtonVariant, Caption, Checkbox, ColorInput, Heading, NumberDrag,
    NumberInput, Separator, Shortcut, Slider, TextInput, ToggleButton, use_theme,
};
use block_editor_beui::{ResizeMode, Sidebar};

use crate::geometry::*;

use super::components::CanvasComponents;
use super::paint::resolve_color;
use super::state::{Alignment, CanvasCommand, CanvasState, CommonValue, common_value};

const SPACING: f32 = 10.0;

const PRESETS: [(&str, CanvasColor); 5] = [
    ("Default", CanvasColor::Auto),
    (
        "Red",
        CanvasColor::Rgba {
            red: 224,
            green: 49,
            blue: 49,
            alpha: 255,
        },
    ),
    (
        "Orange",
        CanvasColor::Rgba {
            red: 240,
            green: 140,
            blue: 0,
            alpha: 255,
        },
    ),
    (
        "Green",
        CanvasColor::Rgba {
            red: 47,
            green: 158,
            blue: 68,
            alpha: 255,
        },
    ),
    (
        "Blue",
        CanvasColor::Rgba {
            red: 25,
            green: 113,
            blue: 194,
            alpha: 255,
        },
    ),
];

const ALIGNMENTS: [(&str, Alignment); 6] = [
    ("Left", Alignment::Left),
    ("Center", Alignment::HorizontalCenter),
    ("Right", Alignment::Right),
    ("Top", Alignment::Top),
    ("Middle", Alignment::VerticalCenter),
    ("Bottom", Alignment::Bottom),
];

const LAYERS: [(&str, CanvasLayerMove); 4] = [
    ("Back", CanvasLayerMove::SendToBack),
    ("-1", CanvasLayerMove::BackOne),
    ("+1", CanvasLayerMove::ForwardOne),
    ("Front", CanvasLayerMove::BringToFront),
];

#[component]
pub(crate) fn CanvasSidebar(
    state: Rc<CanvasState>,
    shown: block_editor_beui::beui::reactive::Prop<bool>,
) -> NodeId {
    let region = Rc::clone(&state);
    let summary = Rc::clone(&state);
    let transform = Rc::clone(&state);
    let block = Rc::clone(&state);
    let components = Rc::clone(&state);
    let appearance = Rc::clone(&state);
    let arrange = Rc::clone(&state);
    let empty = create_memo(clone!(state -> move || state.selection.get().is_empty()));
    let chosen = create_memo(clone!(empty -> move || !empty.get()));
    view! {
        <Sidebar shown={shown}>
            <List spacing=SPACING>
                <Heading content="Inspector" />
                <PreviewRegionSection state={region} />
                <Separator />
                <Show condition={empty}>
                    <Caption content="Select an object to edit its appearance." wrap=true />
                </Show>
                <Show condition={chosen.clone()}>
                    <List spacing=SPACING>
                        <SelectionSummary state={summary} />
                        <TransformSection state={transform} />
                        <BlockSection state={block} />
                        <CanvasComponents state={components} />
                        <AppearanceSection state={appearance} />
                        <ArrangeSection state={arrange} />
                    </List>
                </Show>
                <Separator />
                <ShortcutsSection />
            </List>
        </Sidebar>
    }
}

#[component]
fn PreviewRegionSection(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let enabled = create_memo(clone!(state -> move || state.preview_region.get().is_some()));
    let absent = create_memo(clone!(enabled -> move || !enabled.get()));
    let toggled = clone!(state -> move |wanted: bool| {
        let region = wanted.then(|| preview_region_for_entities(&state.entities.get_untracked()));
        state.set_preview_region(region);
    });
    let fields = Rc::clone(&state);
    let fit = clone!(state -> move || {
        state.set_preview_region(Some(preview_region_for_entities(
            &state.entities.get_untracked(),
        )));
    });
    view! {
        <Accordion title="Canvas preview" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=SPACING>
                <Checkbox
                    label="Use preview region"
                    checked={enabled.clone()}
                    @test_id={"infinite-canvas.preview-region"}
                    on_change={toggled}
                />
                <Show condition={absent}>
                    <Caption
                        content="Without a region, previews use the canvas content extents."
                        wrap=true
                    />
                </Show>
                <Show condition={enabled}>
                    <List spacing=6.0>
                        <PreviewRegionFields state={fields} />
                        <Button
                            label="Fit region to content"
                            variant=ButtonVariant::Secondary
                            @test_id={"infinite-canvas.fit-region"}
                            on_click={fit}
                        />
                    </List>
                </Show>
            </List>
        </Accordion>
    }
}

#[component]
fn PreviewRegionFields(state: Rc<CanvasState>) -> NodeId {
    let region = create_memo(
        clone!(state -> move || state.preview_region.get().unwrap_or(
            CanvasPreviewRegion::new(CanvasPoint::default(), CanvasPoint::new(100.0, 100.0))
        )),
    );
    let edit = move |state: &Rc<CanvasState>, region: CanvasPreviewRegion| {
        state.set_preview_region_grouped(region);
    };
    let x = clone!(state region -> move |value: f64| {
        let mut next = region.get_untracked();
        next.center.x = value as f32;
        edit(&state, next);
    });
    let y = clone!(state region -> move |value: f64| {
        let mut next = region.get_untracked();
        next.center.y = value as f32;
        edit(&state, next);
    });
    let width = clone!(state region -> move |value: f64| {
        let mut next = region.get_untracked();
        next.size.x = (value as f32).max(MIN_SIZE);
        edit(&state, next);
    });
    let height = clone!(state region -> move |value: f64| {
        let mut next = region.get_untracked();
        next.size.y = (value as f32).max(MIN_SIZE);
        edit(&state, next);
    });
    let center_x = create_memo(clone!(region -> move || region.get().center.x as f64));
    let center_y = create_memo(clone!(region -> move || region.get().center.y as f64));
    let size_x = create_memo(clone!(region -> move || region.get().size.x as f64));
    let size_y = create_memo(clone!(region -> move || region.get().size.y as f64));
    view! {
        <List spacing=6.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Caption content="X" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="X"
                    value={center_x}
                    @test_id={"infinite-canvas.region.x"}
                    on_change={x}
                />
                <Caption content="Y" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="Y"
                    value={center_y}
                    @test_id={"infinite-canvas.region.y"}
                    on_change={y}
                />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Caption content="W" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="W"
                    min={MIN_SIZE as f64}
                    value={size_x}
                    @test_id={"infinite-canvas.region.width"}
                    on_change={width}
                />
                <Caption content="H" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="H"
                    min={MIN_SIZE as f64}
                    value={size_y}
                    @test_id={"infinite-canvas.region.height"}
                    on_change={height}
                />
            </List>
        </List>
    }
}

#[component]
fn SelectionSummary(state: Rc<CanvasState>) -> NodeId {
    let label = create_memo(clone!(state -> move || {
        let selected = state.selected_entities();
        match selected.as_slice() {
            [entity] => format!(
                "{} selected{}",
                entity_kind_label(&entity.kind),
                match entity.locked {
                    true => " · Locked",
                    false => "",
                }
            ),
            _ if selected
                .first()
                .and_then(|entity| entity.group_id)
                .is_some_and(|group| {
                    selected.iter().all(|entity| entity.group_id == Some(group))
                }) =>
            {
                format!("Group · {} objects", selected.len())
            }
            _ => format!("{} objects selected", selected.len()),
        }
    }));
    view! {
        <Caption content={label} @test_id={"infinite-canvas.selection"} />
    }
}

#[component]
fn TransformSection(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let single = create_memo(clone!(state -> move || state.selected_entities().len() == 1));
    let many = create_memo(clone!(single -> move || !single.get()));
    let fields = Rc::clone(&state);
    view! {
        <Accordion title="Transform" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=6.0>
                <Show condition={many}>
                    <Caption
                        content="Select one object to edit exact transform values."
                        wrap=true
                    />
                </Show>
                <Show condition={single}>
                    <TransformFields state={fields} />
                </Show>
            </List>
        </Accordion>
    }
}

#[component]
fn TransformFields(state: Rc<CanvasState>) -> NodeId {
    let entity = create_memo(clone!(state -> move || state.selected_entities().pop()));
    let locked = create_memo(clone!(entity -> move || {
        entity.get().is_none_or(|entity| entity.locked)
    }));
    let resize = create_memo(clone!(state entity -> move || match entity.get() {
        Some(entity) => state.resize_mode(&entity),
        None => ResizeMode::None,
    }));
    let no_width = create_memo(clone!(locked resize -> move || {
        locked.get() || !resize.get().horizontal()
    }));
    let no_height = create_memo(clone!(locked resize -> move || {
        locked.get() || !resize.get().vertical()
    }));
    let no_rotation = create_memo(clone!(state locked -> move || {
        locked.get() || !state.selection_allows_rotation()
    }));
    let edit = |state: &Rc<CanvasState>, entity: &CanvasEntity, updated: CanvasEntity| {
        state.record_update(vec![entity.clone()], vec![updated], true);
    };
    let x = clone!(state entity -> move |value: f64| {
        let Some(held) = entity.get_untracked() else { return };
        let mut updated = held.clone();
        updated.transform.center.x = value as f32;
        edit(&state, &held, updated);
    });
    let y = clone!(state entity -> move |value: f64| {
        let Some(held) = entity.get_untracked() else { return };
        let mut updated = held.clone();
        updated.transform.center.y = value as f32;
        edit(&state, &held, updated);
    });
    let width = clone!(state entity -> move |value: f64| {
        let Some(held) = entity.get_untracked() else { return };
        let mut updated = held.clone();
        updated.transform.size.x = (value as f32).max(MIN_SIZE);
        edit(&state, &held, updated);
    });
    let height = clone!(state entity -> move |value: f64| {
        let Some(held) = entity.get_untracked() else { return };
        let mut updated = held.clone();
        updated.transform.size.y = (value as f32).max(MIN_SIZE);
        edit(&state, &held, updated);
    });
    let rotation = clone!(state entity -> move |value: f64| {
        let Some(held) = entity.get_untracked() else { return };
        let mut updated = held.clone();
        updated.transform.rotation = (value as f32).to_radians();
        edit(&state, &held, updated);
    });
    let at = |entity: &Memo<Option<CanvasEntity>>, read: fn(&CanvasEntity) -> f32| {
        let entity = entity.clone();
        create_memo(move || entity.get().map_or(0.0, |entity| read(&entity)) as f64)
    };
    let center_x = at(&entity, |entity| entity.transform.center.x);
    let center_y = at(&entity, |entity| entity.transform.center.y);
    let size_x = at(&entity, |entity| entity.transform.size.x);
    let size_y = at(&entity, |entity| entity.transform.size.y);
    let angle = at(&entity, |entity| entity.transform.rotation.to_degrees());
    view! {
        <List spacing=6.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Caption content="X" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="X"
                    value={center_x}
                    disabled={locked.clone()}
                    @test_id={"infinite-canvas.transform.x"}
                    on_change={x}
                />
                <Caption content="Y" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="Y"
                    value={center_y}
                    disabled={locked.clone()}
                    @test_id={"infinite-canvas.transform.y"}
                    on_change={y}
                />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Caption content="W" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="W"
                    min={MIN_SIZE as f64}
                    value={size_x}
                    disabled={no_width}
                    @test_id={"infinite-canvas.transform.width"}
                    on_change={width}
                />
                <Caption content="H" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="H"
                    min={MIN_SIZE as f64}
                    value={size_y}
                    disabled={no_height}
                    @test_id={"infinite-canvas.transform.height"}
                    on_change={height}
                />
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Caption content="Rotation" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="Rotation"
                    value={angle}
                    disabled={no_rotation}
                    @test_id={"infinite-canvas.transform.rotation"}
                    on_change={rotation}
                />
            </List>
        </List>
    }
}

#[component]
fn BlockSection(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let entity = create_memo(clone!(state -> move || {
        let selected = state.selected_entities();
        let [entity] = selected.as_slice() else {
            return None;
        };
        matches!(
            entity.kind,
            CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
        )
        .then(|| entity.clone())
    }));
    let shown = create_memo(clone!(entity -> move || entity.get().is_some()));
    let direct = create_memo(clone!(entity -> move || {
        entity
            .get()
            .is_some_and(|entity| matches!(entity.kind, CanvasEntityKind::DirectEditor { .. }))
    }));
    let label = create_memo(clone!(direct -> move || match direct.get() {
        true => "Show preview only".to_owned(),
        false => "Use direct editor".to_owned(),
    }));
    let unavailable = create_memo(clone!(state entity direct -> move || {
        !direct.get()
            && entity
                .get()
                .is_none_or(|entity| state.child_state(entity.id).intrinsic_size.is_none())
    }));
    let toggled = clone!(state -> move || state.toggle_selected_block_mode());
    let scaled = Rc::clone(&state);
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <Accordion title="Block" open={open} on_toggle={move |open| set_open.set(open)}>
                    <List spacing=6.0>
                        <Button
                            label={label}
                            variant=ButtonVariant::Secondary
                            disabled={unavailable}
                            @test_id={"infinite-canvas.block-mode"}
                            on_click={toggled}
                        />
                        <Show condition={direct}>
                            <EditorScale state={scaled} />
                        </Show>
                    </List>
                </Accordion>
            </Show>
        </List>
    }
}

#[component]
fn EditorScale(state: Rc<CanvasState>) -> NodeId {
    let entity = create_memo(clone!(state -> move || state.selected_entities().pop()));
    let scale = create_memo(clone!(entity -> move || match entity.get() {
        Some(entity) => match entity.kind {
            CanvasEntityKind::DirectEditor { scale, .. } => scale as f64 * 100.0,
            _ => 100.0,
        },
        None => 100.0,
    }));
    let locked = create_memo(clone!(entity -> move || {
        entity.get().is_none_or(|entity| entity.locked)
    }));
    let changed = clone!(state entity -> move |value: f64| {
        let Some(held) = entity.get_untracked() else { return };
        let CanvasEntityKind::DirectEditor { block_id, scale } = held.kind else {
            return;
        };
        let wanted = ((value / 100.0) as f32).clamp(0.1, 8.0);
        let factor = wanted / scale.max(f32::EPSILON);
        let mut updated = held.clone();
        updated.kind = CanvasEntityKind::DirectEditor {
            block_id,
            scale: wanted,
        };
        updated.transform.size.x *= factor;
        updated.transform.size.y *= factor;
        state.record_update(vec![held], vec![updated], true);
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <Caption content="Scale" />
            <NumberInput
                @sizing=ItemSize::Percent(100.0)
                label="Scale"
                min=10.0
                max=800.0
                value={scale}
                disabled={locked}
                drag={NumberDrag::Linear { speed: 1.0 }}
                @test_id={"infinite-canvas.editor-scale"}
                on_change={changed}
            />
        </List>
    }
}

#[component]
fn ArrangeSection(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let movable = create_memo(clone!(state -> move || state.selected_unlocked().len()));
    let cannot_align = create_memo(clone!(movable -> move || movable.get() < 2));
    let cannot_distribute = create_memo(clone!(movable -> move || movable.get() < 3));
    let cannot_group = create_memo(clone!(state -> move || !state.selection_can_group()));
    let cannot_ungroup = create_memo(clone!(state -> move || {
        !state
            .selected_entities()
            .iter()
            .any(|entity| entity.group_id.is_some())
    }));
    let cannot_lock = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| !entity.locked)
    }));
    let cannot_delete = cannot_lock.clone();
    let cannot_unlock = create_memo(clone!(state -> move || {
        !state.selected_entities().iter().any(|entity| entity.locked)
    }));
    let aligning = Rc::clone(&state);
    let layering = Rc::clone(&state);
    let horizontal = clone!(state -> move || state.distribute_selection(true));
    let vertical = clone!(state -> move || state.distribute_selection(false));
    let group = clone!(state -> move || state.run(CanvasCommand::Group));
    let ungroup = clone!(state -> move || state.run(CanvasCommand::Ungroup));
    let lock = clone!(state -> move || state.run(CanvasCommand::Lock));
    let unlock = clone!(state -> move || state.run(CanvasCommand::Unlock));
    let delete = clone!(state -> move || state.run(CanvasCommand::Delete));
    view! {
        <Accordion title="Arrange" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                    <ForEach keys={(0..ALIGNMENTS.len()).collect::<Vec<usize>>()}>
                        {move |index: usize| {
                            let (label, alignment) = ALIGNMENTS[index];
                            let state = Rc::clone(&aligning);
                            view! {
                                <Button
                                    label={label}
                                    variant=ButtonVariant::Secondary
                                    disabled={cannot_align.clone()}
                                    @test_id={format!("infinite-canvas.align.{label}")}
                                    on_click={move || state.align_selection(alignment)}
                                />
                            }
                        }}
                    </ForEach>
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                    <Button
                        label="Distribute horizontally"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_distribute.clone()}
                        @test_id={"infinite-canvas.distribute-horizontally"}
                        on_click={horizontal}
                    />
                    <Button
                        label="Distribute vertically"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_distribute}
                        @test_id={"infinite-canvas.distribute-vertically"}
                        on_click={vertical}
                    />
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                    <ForEach keys={(0..LAYERS.len()).collect::<Vec<usize>>()}>
                        {move |index: usize| {
                            let (label, movement) = LAYERS[index];
                            let state = Rc::clone(&layering);
                            let blocked = create_memo(clone!(state -> move || {
                                !state.can_reorder(movement)
                            }));
                            view! {
                                <Button
                                    label={label}
                                    variant=ButtonVariant::Secondary
                                    disabled={blocked}
                                    @test_id={format!("infinite-canvas.layer.{label}")}
                                    on_click={move || state.reorder(movement)}
                                />
                            }
                        }}
                    </ForEach>
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                    <Button
                        label="Group"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_group}
                        @test_id={"infinite-canvas.group"}
                        on_click={group}
                    />
                    <Button
                        label="Ungroup"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_ungroup}
                        @test_id={"infinite-canvas.ungroup"}
                        on_click={ungroup}
                    />
                    <Button
                        label="Lock"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_lock}
                        @test_id={"infinite-canvas.lock"}
                        on_click={lock}
                    />
                    <Button
                        label="Unlock"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_unlock}
                        @test_id={"infinite-canvas.unlock"}
                        on_click={unlock}
                    />
                </List>
                <Button
                    label="Delete"
                    variant=ButtonVariant::Secondary
                    disabled={cannot_delete}
                    @test_id={"infinite-canvas.delete"}
                    on_click={delete}
                />
            </List>
        </Accordion>
    }
}

#[component]
fn AppearanceSection(state: Rc<CanvasState>) -> NodeId {
    let (open, set_open) = create_signal(true);
    let color = Rc::clone(&state);
    let stroke = Rc::clone(&state);
    let line = Rc::clone(&state);
    let rectangle = Rc::clone(&state);
    let text = Rc::clone(&state);
    let opacity = Rc::clone(&state);
    view! {
        <Accordion title="Appearance" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=SPACING>
                <ForegroundRow state={color} />
                <LineWidthRow state={stroke} />
                <LineOptions state={line} />
                <RectangleOptions state={rectangle} />
                <TextOptions state={text} />
                <OpacityRow state={opacity} />
            </List>
        </Accordion>
    }
}

fn styled_entities(state: &Rc<CanvasState>) -> Vec<CanvasEntity> {
    state
        .selected_entities()
        .into_iter()
        .filter(|entity| {
            !matches!(
                entity.kind,
                CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
            )
        })
        .collect()
}

#[component]
fn ForegroundRow(state: Rc<CanvasState>) -> NodeId {
    let value = create_memo(clone!(state -> move || {
        common_value(
            styled_entities(&state)
                .iter()
                .map(|entity| entity.style.foreground),
        )
    }));
    let shown = create_memo(clone!(value -> move || !value.get().absent()));
    let chosen = Rc::clone(&state);
    let theme = use_theme();
    let custom = create_memo(clone!(value theme -> move || {
        resolve_color(value.get().or(CanvasColor::Auto), theme.text.get())
    }));
    let typed = clone!(state -> move |color: Color32| {
        let [red, green, blue, alpha] = color.to_array();
        let color = CanvasColor::Rgba {
            red,
            green,
            blue,
            alpha,
        };
        set_foreground(&state, color);
    });
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Caption content="Color" />
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                        <ForEach keys={(0..PRESETS.len()).collect::<Vec<usize>>()}>
                            {move |index: usize| {
                                let (name, color) = PRESETS[index];
                                let state = Rc::clone(&chosen);
                                let value = value.clone();
                                view! {
                                    <ColorPreset
                                        name
                                        color
                                        value
                                        on_pick={move |color| set_foreground(&state, color)}
                                    />
                                }
                            }}
                        </ForEach>
                    </List>
                    <ColorInput
                        label="Custom color"
                        value={custom}
                        @test_id={"infinite-canvas.color"}
                        on_change={typed}
                    />
                </List>
            </Show>
        </List>
    }
}

fn set_foreground(state: &Rc<CanvasState>, color: CanvasColor) {
    state.remember_foreground(color);
    state.update_selected(
        |kind| {
            !matches!(
                kind,
                CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
            )
        },
        |style| style.foreground = color,
    );
}

#[component]
fn ColorPreset(
    name: &'static str,
    color: CanvasColor,
    value: Memo<CommonValue<CanvasColor>>,
    on_pick: block_editor_beui::beui::reactive::Callback<CanvasColor>,
) -> NodeId {
    let pressed = create_memo(clone!(value -> move || value.get() == CommonValue::Uniform(color)));
    view! {
        <ToggleButton
            label={name}
            glyph={ICON_CIRCLE.to_owned()}
            pressed={pressed}
            @test_id={format!("infinite-canvas.color.{name}")}
            on_change={move |_: bool| on_pick.call(color)}
        />
    }
}

#[component]
fn LineWidthRow(state: Rc<CanvasState>) -> NodeId {
    let value = create_memo(clone!(state -> move || {
        common_value(
            state
                .selected_entities()
                .iter()
                .filter(|entity| {
                    matches!(
                        entity.kind,
                        CanvasEntityKind::Line
                            | CanvasEntityKind::Rectangle
                            | CanvasEntityKind::Pen { .. }
                    )
                })
                .map(|entity| entity.style.line_width),
        )
    }));
    let shown = create_memo(clone!(value -> move || !value.get().absent()));
    let mixed = create_memo(clone!(value -> move || value.get().mixed()));
    let width = create_memo(clone!(value -> move || value.get().or(2.0)));
    let changed = clone!(state -> move |width: f32| {
        state.update_selected(
            |kind| {
                matches!(
                    kind,
                    CanvasEntityKind::Line
                        | CanvasEntityKind::Rectangle
                        | CanvasEntityKind::Pen { .. }
                )
            },
            |style| style.line_width = width,
        );
    });
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=4.0>
                    <MixedLabel label="Line width" mixed={mixed} />
                    <Slider
                        label="Line width"
                        min=0.5
                        max=20.0
                        value={width}
                        @test_id={"infinite-canvas.line-width"}
                        on_change={changed}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn MixedLabel(label: &'static str, mixed: Memo<bool>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <Caption content={label} />
            <Show condition={mixed}>
                <Caption content="Mixed" />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn LineOptions(state: Rc<CanvasState>) -> NodeId {
    let lines = create_memo(clone!(state -> move || {
        state
            .selected_entities()
            .into_iter()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Line))
            .collect::<Vec<_>>()
    }));
    let shown = create_memo(clone!(lines -> move || !lines.with(Vec::is_empty)));
    let flag = |lines: &Memo<Vec<CanvasEntity>>, read: fn(&CanvasEntity) -> bool| {
        let lines = lines.clone();
        create_memo(move || {
            common_value(lines.with(|lines| lines.iter().map(read).collect::<Vec<bool>>()))
                .or(false)
        })
    };
    let dashed = flag(&lines, |entity| entity.style.dashed);
    let start = flag(&lines, |entity| entity.style.arrow_start);
    let end = flag(&lines, |entity| entity.style.arrow_end);
    let set = |state: &Rc<CanvasState>,
               apply: fn(&mut block_editor_beui::be_block::canvas::CanvasEntityStyle, bool),
               value: bool| {
        state.update_selected(
            |kind| matches!(kind, CanvasEntityKind::Line),
            move |style| apply(style, value),
        );
    };
    let dash = clone!(state -> move |value: bool| {
        set(&state, |style, value| style.dashed = value, value)
    });
    let arrow_start = clone!(state -> move |value: bool| {
        set(&state, |style, value| style.arrow_start = value, value)
    });
    let arrow_end = clone!(state -> move |value: bool| {
        set(&state, |style, value| style.arrow_end = value, value)
    });
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Separator />
                    <Caption content="Line" />
                    <Checkbox
                        label="Dashed"
                        checked={dashed}
                        @test_id={"infinite-canvas.dashed"}
                        on_change={dash}
                    />
                    <Checkbox
                        label="Start arrow"
                        checked={start}
                        @test_id={"infinite-canvas.arrow-start"}
                        on_change={arrow_start}
                    />
                    <Checkbox
                        label="End arrow"
                        checked={end}
                        @test_id={"infinite-canvas.arrow-end"}
                        on_change={arrow_end}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RectangleOptions(state: Rc<CanvasState>) -> NodeId {
    let rectangles = create_memo(clone!(state -> move || {
        state
            .selected_entities()
            .into_iter()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Rectangle))
            .collect::<Vec<_>>()
    }));
    let shown = create_memo(clone!(rectangles -> move || !rectangles.with(Vec::is_empty)));
    let fill = create_memo(clone!(rectangles -> move || {
        common_value(
            rectangles.with(|rectangles| {
                rectangles
                    .iter()
                    .map(|entity| entity.style.fill)
                    .collect::<Vec<_>>()
            }),
        )
    }));
    let radius = create_memo(clone!(rectangles -> move || {
        common_value(rectangles.with(|rectangles| {
            rectangles
                .iter()
                .map(|entity| entity.style.corner_radius)
                .collect::<Vec<f32>>()
        }))
    }));
    let mixed = create_memo(clone!(radius -> move || radius.get().mixed()));
    let corner = create_memo(clone!(radius -> move || radius.get().or(0.0)));
    let rounded = clone!(state -> move |value: f32| {
        state.update_selected(
            |kind| matches!(kind, CanvasEntityKind::Rectangle),
            |style| style.corner_radius = value,
        );
    });
    let filling = Rc::clone(&state);
    let clearing = clone!(state -> move |_: bool| set_fill(&state, None));
    let theme = use_theme();
    let custom = create_memo(clone!(fill theme -> move || {
        resolve_color(fill.get().or(None).unwrap_or(CanvasColor::Auto), theme.text.get())
    }));
    let typed = clone!(filling -> move |color: Color32| {
        let [red, green, blue, alpha] = color.to_array();
        set_fill(
            &filling,
            Some(CanvasColor::Rgba {
                red,
                green,
                blue,
                alpha,
            }),
        );
    });
    let none = create_memo(clone!(fill -> move || fill.get() == CommonValue::Uniform(None)));
    let picking = Rc::clone(&state);
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Separator />
                    <Caption content="Rectangle" />
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                        <ToggleButton
                            label="No fill"
                            glyph={ICON_FORMAT_COLOR_RESET.to_owned()}
                            pressed={none}
                            @test_id={"infinite-canvas.fill.none"}
                            on_change={clearing}
                        />
                        <ForEach keys={(0..PRESETS.len()).collect::<Vec<usize>>()}>
                            {move |index: usize| {
                                let (name, color) = PRESETS[index];
                                let state = Rc::clone(&picking);
                                let value = create_memo(clone!(fill -> move || match fill.get() {
                                    CommonValue::Uniform(Some(color)) => CommonValue::Uniform(color),
                                    CommonValue::Uniform(None) | CommonValue::None => {
                                        CommonValue::None
                                    }
                                    CommonValue::Mixed => CommonValue::Mixed,
                                }));
                                view! {
                                    <ColorPreset
                                        name
                                        color
                                        value
                                        on_pick={move |color| set_fill(&state, Some(color))}
                                    />
                                }
                            }}
                        </ForEach>
                    </List>
                    <ColorInput
                        label="Fill color"
                        value={custom}
                        @test_id={"infinite-canvas.fill"}
                        on_change={typed}
                    />
                    <MixedLabel label="Corner radius" mixed={mixed} />
                    <Slider
                        label="Corner radius"
                        min=0.0
                        max=100.0
                        value={corner}
                        @test_id={"infinite-canvas.corner-radius"}
                        on_change={rounded}
                    />
                </List>
            </Show>
        </List>
    }
}

fn set_fill(state: &Rc<CanvasState>, fill: Option<CanvasColor>) {
    state.remember_fill(fill);
    state.update_selected(
        |kind| matches!(kind, CanvasEntityKind::Rectangle),
        |style| style.fill = fill,
    );
}

#[component]
fn OpacityRow(state: Rc<CanvasState>) -> NodeId {
    let value = create_memo(clone!(state -> move || {
        common_value(
            state
                .selected_entities()
                .iter()
                .map(|entity| entity.style.opacity),
        )
    }));
    let mixed = create_memo(clone!(value -> move || value.get().mixed()));
    let opacity = create_memo(clone!(value -> move || value.get().or(1.0)));
    let changed = clone!(state -> move |value: f32| {
        state.update_selected(|_| true, |style| style.opacity = value);
    });
    view! {
        <List spacing=4.0>
            <Separator />
            <MixedLabel label="Opacity" mixed={mixed} />
            <Slider
                label="Opacity"
                min=0.0
                max=1.0
                value={opacity}
                @test_id={"infinite-canvas.opacity"}
                on_change={changed}
            />
        </List>
    }
}

const ALIGN_OPTIONS: [(&str, CanvasTextAlign); 3] = [
    ("Left", CanvasTextAlign::Left),
    ("Center", CanvasTextAlign::Center),
    ("Right", CanvasTextAlign::Right),
];

#[component]
fn TextOptions(state: Rc<CanvasState>) -> NodeId {
    let styles = create_memo(clone!(state -> move || {
        state
            .selected_entities()
            .iter()
            .filter_map(|entity| match &entity.kind {
                CanvasEntityKind::Text { text_style, .. } => Some(*text_style),
                _ => None,
            })
            .collect::<Vec<_>>()
    }));
    let shown = create_memo(clone!(styles -> move || !styles.with(Vec::is_empty)));
    let single = create_memo(clone!(state -> move || {
        let selected = state.selected_entities();
        matches!(
            selected.as_slice(),
            [entity] if matches!(entity.kind, CanvasEntityKind::Text { .. })
        )
    }));
    let font_size = create_memo(clone!(styles -> move || {
        common_value(styles.with(|styles| {
            styles.iter().map(|style| style.font_size).collect::<Vec<f32>>()
        }))
        .or(18.0) as f64
    }));
    let line_height = create_memo(clone!(styles -> move || {
        common_value(styles.with(|styles| {
            styles.iter().map(|style| style.line_height).collect::<Vec<f32>>()
        }))
        .or(1.2) as f64
    }));
    let bold = create_memo(clone!(styles -> move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.weight == CanvasTextWeight::Bold)
                .collect::<Vec<bool>>()
        }))
        .or(false)
    }));
    let wrap = create_memo(clone!(styles -> move || {
        common_value(styles.with(|styles| {
            styles.iter().map(|style| style.wrap).collect::<Vec<bool>>()
        }))
        .or(false)
    }));
    let alignment = create_memo(clone!(styles -> move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.alignment)
                .collect::<Vec<CanvasTextAlign>>()
        }))
    }));
    let sized = clone!(state -> move |value: f64| {
        let size = (value as f32).clamp(4.0, 256.0);
        state.update_selected_text(move |style| style.font_size = size);
    });
    let spaced = clone!(state -> move |value: f64| {
        let height = (value as f32).clamp(0.5, 4.0);
        state.update_selected_text(move |style| style.line_height = height);
    });
    let weighted = clone!(state -> move |bold: bool| {
        state.update_selected_text(move |style| {
            style.weight = match bold {
                true => CanvasTextWeight::Bold,
                false => CanvasTextWeight::Regular,
            };
        });
    });
    let wrapped = clone!(state -> move |wrap: bool| {
        state.update_selected_text(move |style| style.wrap = wrap);
    });
    let aligning = Rc::clone(&state);
    let editing = Rc::clone(&state);
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Separator />
                    <Caption content="Text" />
                    <Show condition={single}>
                        <TextContent state={editing} />
                    </Show>
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Caption content="Font size" />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            label="Font size"
                            min=4.0
                            max=256.0
                            value={font_size}
                            @test_id={"infinite-canvas.font-size"}
                            on_change={sized}
                        />
                    </List>
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Caption content="Line height" />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            label="Line height"
                            min=0.5
                            max=4.0
                            value={line_height}
                            drag={NumberDrag::Linear { speed: 0.05 }}
                            @test_id={"infinite-canvas.line-height"}
                            on_change={spaced}
                        />
                    </List>
                    <Checkbox
                        label="Bold"
                        checked={bold}
                        @test_id={"infinite-canvas.bold"}
                        on_change={weighted}
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                        <ForEach keys={(0..ALIGN_OPTIONS.len()).collect::<Vec<usize>>()}>
                            {move |index: usize| {
                                let (label, wanted) = ALIGN_OPTIONS[index];
                                let state = Rc::clone(&aligning);
                                let pressed = create_memo(clone!(alignment -> move || {
                                    alignment.get() == CommonValue::Uniform(wanted)
                                }));
                                view! {
                                    <ToggleButton
                                        label={label}
                                        pressed={pressed}
                                        @test_id={format!("infinite-canvas.align-text.{label}")}
                                        on_change={move |_: bool| {
                                            state.update_selected_text(move |style| {
                                                style.alignment = wanted;
                                            });
                                        }}
                                    />
                                }
                            }}
                        </ForEach>
                    </List>
                    <Checkbox
                        label="Wrap text"
                        checked={wrap}
                        @test_id={"infinite-canvas.wrap"}
                        on_change={wrapped}
                    />
                    <Caption
                        content="Resize to wrap; hold Alt while resizing to scale text."
                        wrap=true
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn TextContent(state: Rc<CanvasState>) -> NodeId {
    let entity = create_memo(clone!(state -> move || state.selected_entities().pop()));
    let text = create_memo(clone!(entity -> move || match entity.get() {
        Some(entity) => match entity.kind {
            CanvasEntityKind::Text { text, .. } => text,
            _ => String::new(),
        },
        None => String::new(),
    }));
    let placeholder = create_memo(clone!(entity -> move || match entity.get() {
        Some(entity) => match entity.kind {
            CanvasEntityKind::Text { placeholder, .. } => placeholder,
            _ => String::new(),
        },
        None => String::new(),
    }));
    let locked = create_memo(clone!(entity -> move || {
        entity.get().is_none_or(|entity| entity.locked)
    }));
    let focused = create_memo(clone!(state entity -> move || {
        entity
            .get()
            .is_some_and(|entity| state.editing_text.get() == Some(entity.id))
    }));
    let changed = clone!(state entity -> move |typed: String| {
        let Some(held) = entity.get_untracked() else { return };
        let CanvasEntityKind::Text {
            text_style,
            placeholder,
            ..
        } = held.kind.clone() else {
            return;
        };
        let mut updated = held.clone();
        updated.kind = CanvasEntityKind::Text {
            text: typed,
            text_style,
            placeholder,
        };
        state.record_update(vec![held.clone()], vec![updated], true);
        if !text_style.wrap {
            state.request_text_measure(held.id);
        }
    });
    let left = clone!(state -> move |has_focus: bool| {
        if !has_focus {
            state.edit_text(None);
            state.finish_grouped_edit();
        }
    });
    view! {
        <TextInput
            label="Text"
            value={text}
            placeholder={placeholder}
            disabled={locked}
            focused={focused}
            @test_id={"infinite-canvas.text"}
            on_change={changed}
            on_focus_change={left}
        />
    }
}

const SHORTCUTS: [(&str, &str); 9] = [
    ("Select / Rectangle / Line", "V / R / L"),
    ("Text / Pen", "T / P"),
    ("Pan", "Space-drag or middle-drag"),
    ("Zoom", "Ctrl-scroll or pinch"),
    ("Select all", "Ctrl+A"),
    ("Nudge", "Arrow keys; Shift for 10×"),
    ("Duplicate", "Ctrl+D or Alt-drag"),
    ("Edit selected block", "Enter"),
    ("Exit tool or editor", "Escape"),
];

#[component]
fn ShortcutsSection() -> NodeId {
    let (open, set_open) = create_signal(false);
    view! {
        <Accordion title="Shortcuts" open={open} on_toggle={move |open| set_open.set(open)}>
            <List spacing=4.0>
                <ForEach keys={(0..SHORTCUTS.len()).collect::<Vec<usize>>()}>
                    {move |index: usize| {
                        let (action, keys) = SHORTCUTS[index];
                        view! {
                            <Shortcut keys={keys} description={action} />
                        }
                    }}
                </ForEach>
            </List>
        </Accordion>
    }
}
