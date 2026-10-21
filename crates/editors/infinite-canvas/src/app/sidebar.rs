use block_editor_beui::be_block::canvas::{
    CanvasColor, CanvasEntity, CanvasEntityKind, CanvasLayerMove, CanvasTextAlign,
    CanvasTextVerticalAlign, CanvasTextWeight,
};
use block_editor_beui::beui::Color32;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_FORMAT_COLOR_RESET;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{ResizeMode, Sidebar, narrow_chrome};

use crate::geometry::*;

use super::components::CanvasComponents;
use super::layers::LayersSection;
use super::paint::resolve_color;
use super::selection_bar::Swatch;
use super::state::{Alignment, CanvasCommand, CanvasState, CommonValue, common_value};

const SPACING: f32 = 10.0;

pub(crate) const PRESETS: [(&str, CanvasColor); 5] = [
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
    state: CanvasState,
    shown: block_editor_beui::beui::reactive::Prop<bool>,
) -> NodeId {
    view! {
        <Sidebar shown={shown}>
            <Inspector state={state} />
        </Sidebar>
    }
}

#[component]
fn Inspector(state: CanvasState) -> NodeId {
    let empty = create_memo(move || state.selection.get().is_empty());
    let chosen = create_memo(move || !empty.get());
    let narrow = narrow_chrome();
    let roomy = create_memo(move || !narrow.get());
    let styled = create_memo(move || {
        state
            .selected_entities()
            .iter()
            .any(|entity| !entity.is_artboard())
    });
    view! {
        <List spacing=SPACING>
            <SelectionSummary state={state} />
            <Show condition={empty}>
                <Text
                    style=TextStyle::CAPTION
                    string="Select an object to edit its appearance."
                    wrap=true
                />
            </Show>
            <Show condition={chosen}>
                <List spacing=SPACING>
                    <Show condition={styled}>
                        <AppearanceSection state={state} />
                    </Show>
                    <TransformSection state={state} />
                    <ArrangeSection state={state} />
                    <ArtboardSection state={state} />
                    <BlockSection state={state} />
                    <CanvasComponents state={state} />
                </List>
            </Show>
            <LayersSection state={state} />
            <Show condition={roomy}>
                <List spacing=SPACING>
                    <Separator />
                    <ShortcutsSection />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn ArtboardSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let artboard = create_memo(move || {
        let selected = state.selected_entities();
        let [entity] = selected.as_slice() else {
            return None;
        };
        entity.is_artboard().then(|| entity.clone())
    });
    let shown = create_memo(move || artboard.get().is_some());
    let name = create_memo(move || match artboard.get().map(|entity| entity.kind) {
        Some(CanvasEntityKind::Artboard { name }) => name,
        _ => String::new(),
    });
    let locked = create_memo(move || artboard.get().is_none_or(|entity| entity.locked));
    let first = create_memo(move || {
        artboard.get().is_some_and(|entity| {
            state
                .first_artboard()
                .is_some_and(|first| first.id == entity.id)
        })
    });
    let renamed = move |name: String| {
        let Some(held) = artboard.get_untracked() else {
            return;
        };
        let mut updated = held.clone();
        updated.kind = CanvasEntityKind::Artboard { name };
        state.record_update(vec![held], vec![updated], true);
    };
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <Accordion title="Artboard" open={open}>
                    <List spacing=6.0>
                        <TextInput
                            label="Name"
                            value={Binding::new(name, renamed)}
                            disabled={locked}
                            @test_id={"infinite-canvas.artboard.name"}
                        />
                        <Show condition={first}>
                            <Text
                                style=TextStyle::CAPTION
                                string="The first artboard is what other editors show when they embed this canvas."
                                wrap=true
                            />
                        </Show>
                        <ArtboardBackground artboard={artboard} locked={locked} state={state} />
                    </List>
                </Accordion>
            </Show>
        </List>
    }
}

const BACKGROUNDS: [(&str, CanvasColor); 2] = [
    (
        "White",
        CanvasColor::Rgba {
            red: 255,
            green: 255,
            blue: 255,
            alpha: 255,
        },
    ),
    (
        "Black",
        CanvasColor::Rgba {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 255,
        },
    ),
];

#[component]
fn ArtboardBackground(
    state: CanvasState,
    artboard: Memo<Option<CanvasEntity>>,
    locked: Memo<bool>,
) -> NodeId {
    let fill = create_memo(move || artboard.get().and_then(|entity| entity.style.fill));
    let theme = use_theme();
    let custom =
        create_memo(move || resolve_color(fill.get().unwrap_or_default(), theme.background.get()));
    let following = create_memo(move || fill.get().is_none());
    let value = create_memo(move || match fill.get() {
        Some(color) => CommonValue::Uniform(color),
        None => CommonValue::None,
    });
    let set = move |fill: Option<CanvasColor>| {
        let Some(held) = artboard.get_untracked().filter(|entity| !entity.locked) else {
            return;
        };
        let mut updated = held.clone();
        updated.style.fill = fill;
        state.record_update(vec![held], vec![updated], true);
    };
    let typed = move |color: Color32| {
        let [red, green, blue, alpha] = color.to_array();
        set(Some(CanvasColor::Rgba {
            red,
            green,
            blue,
            alpha,
        }));
    };
    let swatches = BACKGROUNDS
        .iter()
        .chain(&PRESETS[1..])
        .copied()
        .collect::<Vec<_>>();
    let count = swatches.len();
    view! {
        <List spacing=6.0>
            <Text style=TextStyle::CAPTION string="Background" />
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                <ToggleButton
                    label="Theme background"
                    glyph={ICON_FORMAT_COLOR_RESET.to_owned()}
                    pressed={Binding::new(following, move |_: bool| set(None))}
                    disabled={locked}
                    @test_id={"infinite-canvas.background.theme"}
                />
                <ForEach keys={(0..count).collect::<Vec<usize>>()}>|index: usize| {
                    let (name, color) = swatches[index];
                    view! {
                        <Swatch
                            kind="background"
                            name
                            color
                            value
                            on_pick={move |color| set(Some(color))}
                        />
                    }
                }</ForEach>
            </List>
            <ColorInput
                label="Background color"
                value={Binding::new(custom, typed)}
                disabled={locked}
                @test_id={"infinite-canvas.background"}
            />
        </List>
    }
}

#[component]
fn SelectionSummary(state: CanvasState) -> NodeId {
    let label = create_memo(move || {
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
            [] => "Canvas".to_owned(),
            _ => format!("{} objects selected", selected.len()),
        }
    });
    view! {
        <Text style=TextStyle::HEADING string={label} @test_id={"infinite-canvas.selection"} />
    }
}

#[component]
fn TransformSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let single = create_memo(move || state.selected_entities().len() == 1);
    let many = create_memo(move || !single.get());
    view! {
        <Accordion title="Transform" open={open}>
            <List spacing=6.0>
                <Show condition={many}>
                    <Text
                        style=TextStyle::CAPTION
                        string="Select one object to edit exact transform values."
                        wrap=true
                    />
                </Show>
                <Show condition={single}>
                    <TransformFields state={state} />
                </Show>
            </List>
        </Accordion>
    }
}

#[component]
fn TransformFields(state: CanvasState) -> NodeId {
    let entity = create_memo(move || state.selected_entities().pop());
    let locked = create_memo(move || entity.get().is_none_or(|entity| entity.locked));
    let resize = create_memo(move || match entity.get() {
        Some(entity) => state.resize_mode(&entity),
        None => ResizeMode::None,
    });
    let no_width = create_memo(move || locked.get() || !resize.get().horizontal());
    let no_height = create_memo(move || locked.get() || !resize.get().vertical());
    let no_rotation = create_memo(move || locked.get() || !state.selection_allows_rotation());
    let field = |apply: fn(&mut CanvasEntity, f64)| {
        let change = move |value: f64| {
            let Some(held) = entity.get_untracked() else {
                return;
            };
            let mut updated = held.clone();
            apply(&mut updated, value);
            state.record_update(vec![held], vec![updated], true);
        };
        let preview = move |value: Option<f64>| {
            let typed = value.zip(entity.get_untracked()).map(|(value, mut held)| {
                apply(&mut held, value);
                held
            });
            state.preview_typed(typed);
        };
        (change, preview)
    };
    let (x, x_preview) = field(|entity, value| entity.transform.center.x = value as f32);
    let (y, y_preview) = field(|entity, value| entity.transform.center.y = value as f32);
    let (width, width_preview) =
        field(|entity, value| entity.transform.size.x = (value as f32).max(MIN_SIZE));
    let (height, height_preview) =
        field(|entity, value| entity.transform.size.y = (value as f32).max(MIN_SIZE));
    let (rotation, rotation_preview) =
        field(|entity, value| entity.transform.rotation = (value as f32).to_radians());
    let at = |entity: Memo<Option<CanvasEntity>>, read: fn(&CanvasEntity) -> f32| {
        create_memo(move || entity.get().map_or(0.0, |entity| read(&entity)) as f64)
    };
    let center_x = at(entity, |entity| entity.transform.center.x);
    let center_y = at(entity, |entity| entity.transform.center.y);
    let size_x = at(entity, |entity| entity.transform.size.x);
    let size_y = at(entity, |entity| entity.transform.size.y);
    let angle = at(entity, |entity| entity.transform.rotation.to_degrees());
    view! {
        <List spacing=6.0>
            <Grid
                columns={[Track::Intrinsic, Track::Fraction(1.0)].repeat(2)}
                column_spacing=6.0
                row_spacing=6.0
            >
                <Frame align={Align2::vertical(Align::Center)}>
                    <Text style=TextStyle::CAPTION string="X" />
                </Frame>
                <NumberInput
                    label="X"
                    value={Binding::new(center_x, x)}
                    disabled={locked}
                    decimals={Some(2)}
                    @test_id={"infinite-canvas.transform.x"}
                    on_preview={x_preview}
                />
                <Frame align={Align2::vertical(Align::Center)}>
                    <Text style=TextStyle::CAPTION string="Y" />
                </Frame>
                <NumberInput
                    label="Y"
                    value={Binding::new(center_y, y)}
                    disabled={locked}
                    decimals={Some(2)}
                    @test_id={"infinite-canvas.transform.y"}
                    on_preview={y_preview}
                />
                <Frame align={Align2::vertical(Align::Center)}>
                    <Text style=TextStyle::CAPTION string="W" />
                </Frame>
                <NumberInput
                    label="W"
                    min={MIN_SIZE as f64}
                    value={Binding::new(size_x, width)}
                    disabled={no_width}
                    decimals={Some(2)}
                    @test_id={"infinite-canvas.transform.width"}
                    on_preview={width_preview}
                />
                <Frame align={Align2::vertical(Align::Center)}>
                    <Text style=TextStyle::CAPTION string="H" />
                </Frame>
                <NumberInput
                    label="H"
                    min={MIN_SIZE as f64}
                    value={Binding::new(size_y, height)}
                    disabled={no_height}
                    decimals={Some(2)}
                    @test_id={"infinite-canvas.transform.height"}
                    on_preview={height_preview}
                />
            </Grid>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Text style=TextStyle::CAPTION string="Rotation" />
                <NumberInput
                    @sizing=ItemSize::Percent(100.0)
                    label="Rotation"
                    value={Binding::new(angle, rotation)}
                    disabled={no_rotation}
                    decimals={Some(2)}
                    @test_id={"infinite-canvas.transform.rotation"}
                    on_preview={rotation_preview}
                />
            </List>
        </List>
    }
}

#[component]
fn BlockSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let entity = create_memo(move || {
        let selected = state.selected_entities();
        let [entity] = selected.as_slice() else {
            return None;
        };
        matches!(
            entity.kind,
            CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
        )
        .then(|| entity.clone())
    });
    let shown = create_memo(move || entity.get().is_some());
    let direct = create_memo(move || {
        entity
            .get()
            .is_some_and(|entity| matches!(entity.kind, CanvasEntityKind::DirectEditor { .. }))
    });
    let label = create_memo(move || match direct.get() {
        true => "Show preview only".to_owned(),
        false => "Use direct editor".to_owned(),
    });
    let unavailable = create_memo(move || {
        !direct.get()
            && entity
                .get()
                .is_none_or(|entity| state.child_state(entity.id).intrinsic_size.is_none())
    });
    let toggled = move || state.toggle_selected_block_mode();
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <Accordion title="Block" open={open}>
                    <List spacing=6.0>
                        <Button
                            label={label}
                            variant=ButtonVariant::Secondary
                            disabled={unavailable}
                            @test_id={"infinite-canvas.block-mode"}
                            on_click={toggled}
                        />
                        <Show condition={direct}>
                            <EditorScale state={state} />
                        </Show>
                    </List>
                </Accordion>
            </Show>
        </List>
    }
}

#[component]
fn EditorScale(state: CanvasState) -> NodeId {
    let entity = create_memo(move || state.selected_entities().pop());
    let scale = create_memo(move || match entity.get() {
        Some(entity) => match entity.kind {
            CanvasEntityKind::DirectEditor { scale, .. } => scale as f64 * 100.0,
            _ => 100.0,
        },
        None => 100.0,
    });
    let locked = create_memo(move || entity.get().is_none_or(|entity| entity.locked));
    let changed = move |value: f64| {
        let Some(held) = entity.get_untracked() else {
            return;
        };
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
    };
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
            <Text style=TextStyle::CAPTION string="Scale" />
            <NumberInput
                @sizing=ItemSize::Percent(100.0)
                label="Scale"
                min=10.0
                max=800.0
                value={Binding::new(scale, changed)}
                disabled={locked}
                drag={NumberDrag::Linear { speed: 1.0 }}
                @test_id={"infinite-canvas.editor-scale"}
            />
        </List>
    }
}

#[component]
fn ArrangeSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    let movable = create_memo(move || state.selected_unlocked().len());
    let cannot_align = create_memo(move || movable.get() < 2);
    let cannot_distribute = create_memo(move || movable.get() < 3);
    let cannot_group = create_memo(move || !state.selection_can_group());
    let cannot_ungroup = create_memo(move || {
        !state
            .selected_entities()
            .iter()
            .any(|entity| entity.group_id.is_some())
    });
    let cannot_lock = create_memo(move || {
        !state
            .selected_entities()
            .iter()
            .any(|entity| !entity.locked)
    });
    let cannot_unlock =
        create_memo(move || !state.selected_entities().iter().any(|entity| entity.locked));
    let horizontal = move || state.distribute_selection(true);
    let vertical = move || state.distribute_selection(false);
    let group = move || state.run(CanvasCommand::Group);
    let ungroup = move || state.run(CanvasCommand::Ungroup);
    let lock = move || state.run(CanvasCommand::Lock);
    let unlock = move || state.run(CanvasCommand::Unlock);
    view! {
        <Accordion title="Arrange" open={open}>
            <List spacing=6.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                    <ForEach keys={(0..ALIGNMENTS.len()).collect::<Vec<usize>>()}>|index: usize| {
                        let (label, alignment) = ALIGNMENTS[index];
                        view! {
                            <Button
                                label={label}
                                variant=ButtonVariant::Secondary
                                disabled={cannot_align}
                                @test_id={format!("infinite-canvas.align.{label}")}
                                on_click={move || state.align_selection(alignment)}
                            />
                        }
                    }</ForEach>
                </List>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                    <Button
                        label="Distribute horizontally"
                        variant=ButtonVariant::Secondary
                        disabled={cannot_distribute}
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
                    <ForEach keys={(0..LAYERS.len()).collect::<Vec<usize>>()}>|index: usize| {
                        let (label, movement) = LAYERS[index];
                        let blocked = create_memo(move || {
                            !state.can_reorder(movement)
                        });
                        view! {
                            <Button
                                label={label}
                                variant=ButtonVariant::Secondary
                                disabled={blocked}
                                @test_id={format!("infinite-canvas.layer.{label}")}
                                on_click={move || state.reorder(movement)}
                            />
                        }
                    }</ForEach>
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
            </List>
        </Accordion>
    }
}

#[component]
fn AppearanceSection(state: CanvasState) -> NodeId {
    let open = create_signal(true);
    view! {
        <Accordion title="Appearance" open={open}>
            <List spacing=SPACING>
                <ForegroundRow state={state} />
                <LineWidthRow state={state} />
                <LineOptions state={state} />
                <RectangleOptions state={state} />
                <TextOptions state={state} />
                <OpacityRow state={state} />
            </List>
        </Accordion>
    }
}

pub(crate) fn styled_entities(state: CanvasState) -> Vec<CanvasEntity> {
    state
        .selected_entities()
        .into_iter()
        .filter(|entity| {
            !matches!(
                entity.kind,
                CanvasEntityKind::Block { .. }
                    | CanvasEntityKind::DirectEditor { .. }
                    | CanvasEntityKind::Artboard { .. }
            )
        })
        .collect()
}

#[component]
fn ForegroundRow(state: CanvasState) -> NodeId {
    let value = create_memo(move || {
        common_value(
            styled_entities(state)
                .iter()
                .map(|entity| entity.style.foreground),
        )
    });
    let shown = create_memo(move || !value.get().absent());
    let theme = use_theme();
    let custom =
        create_memo(move || resolve_color(value.get().or(CanvasColor::Auto), theme.text.get()));
    let typed = move |color: Color32| {
        let [red, green, blue, alpha] = color.to_array();
        let color = CanvasColor::Rgba {
            red,
            green,
            blue,
            alpha,
        };
        set_foreground(state, color);
    };
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Text style=TextStyle::CAPTION string="Color" />
                    <ColorInput
                        label="Custom color"
                        value={Binding::new(custom, typed)}
                        @test_id={"infinite-canvas.color"}
                    />
                </List>
            </Show>
        </List>
    }
}

pub(crate) fn set_foreground(state: CanvasState, color: CanvasColor) {
    state.remember_foreground(color);
    state.update_selected(
        |kind| {
            !matches!(
                kind,
                CanvasEntityKind::Block { .. }
                    | CanvasEntityKind::DirectEditor { .. }
                    | CanvasEntityKind::Artboard { .. }
            )
        },
        |style| style.foreground = color,
    );
}

#[component]
fn LineWidthRow(state: CanvasState) -> NodeId {
    let value = create_memo(move || {
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
    });
    let shown = create_memo(move || !value.get().absent());
    let mixed = create_memo(move || value.get().mixed());
    let width = create_memo(move || value.get().or(2.0));
    let changed = move |width: f32| {
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
    };
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=4.0>
                    <MixedLabel label="Line width" mixed={mixed} />
                    <Slider
                        label="Line width"
                        min=0.5
                        max=20.0
                        value={Binding::new(width, changed)}
                        @test_id={"infinite-canvas.line-width"}
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
            <Text style=TextStyle::CAPTION string={label} />
            <Show condition={mixed}>
                <Text style=TextStyle::CAPTION string="Mixed" />
            </Show>
            <Spacer @sizing=ItemSize::Percent(100.0) />
        </List>
    }
}

#[component]
fn LineOptions(state: CanvasState) -> NodeId {
    let lines = create_memo(move || {
        state
            .selected_entities()
            .into_iter()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Line))
            .collect::<Vec<_>>()
    });
    let shown = create_memo(move || !lines.with(Vec::is_empty));
    let flag = |lines: Memo<Vec<CanvasEntity>>, read: fn(&CanvasEntity) -> bool| {
        create_memo(move || {
            common_value(lines.with(|lines| lines.iter().map(read).collect::<Vec<bool>>()))
                .or(false)
        })
    };
    let dashed = flag(lines, |entity| entity.style.dashed);
    let start = flag(lines, |entity| entity.style.arrow_start);
    let end = flag(lines, |entity| entity.style.arrow_end);
    let set = |state: CanvasState,
               apply: fn(&mut block_editor_beui::be_block::canvas::CanvasEntityStyle, bool),
               value: bool| {
        state.update_selected(
            |kind| matches!(kind, CanvasEntityKind::Line),
            move |style| apply(style, value),
        );
    };
    let dash = move |value: bool| set(state, |style, value| style.dashed = value, value);
    let arrow_start =
        move |value: bool| set(state, |style, value| style.arrow_start = value, value);
    let arrow_end = move |value: bool| set(state, |style, value| style.arrow_end = value, value);
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Separator />
                    <Text style=TextStyle::CAPTION string="Line" />
                    <Checkbox
                        label="Dashed"
                        checked={Binding::new(dashed, dash)}
                        @test_id={"infinite-canvas.dashed"}
                    />
                    <Checkbox
                        label="Start arrow"
                        checked={Binding::new(start, arrow_start)}
                        @test_id={"infinite-canvas.arrow-start"}
                    />
                    <Checkbox
                        label="End arrow"
                        checked={Binding::new(end, arrow_end)}
                        @test_id={"infinite-canvas.arrow-end"}
                    />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn RectangleOptions(state: CanvasState) -> NodeId {
    let rectangles = create_memo(move || {
        state
            .selected_entities()
            .into_iter()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Rectangle))
            .collect::<Vec<_>>()
    });
    let shown = create_memo(move || !rectangles.with(Vec::is_empty));
    let fill = create_memo(move || {
        common_value(rectangles.with(|rectangles| {
            rectangles
                .iter()
                .map(|entity| entity.style.fill)
                .collect::<Vec<_>>()
        }))
    });
    let radius = create_memo(move || {
        common_value(rectangles.with(|rectangles| {
            rectangles
                .iter()
                .map(|entity| entity.style.corner_radius)
                .collect::<Vec<f32>>()
        }))
    });
    let mixed = create_memo(move || radius.get().mixed());
    let corner = create_memo(move || radius.get().or(0.0));
    let rounded = move |value: f32| {
        state.update_selected(
            |kind| matches!(kind, CanvasEntityKind::Rectangle),
            |style| style.corner_radius = value,
        );
    };
    let clearing = move |_: bool| set_fill(state, None);
    let theme = use_theme();
    let custom = create_memo(move || {
        resolve_color(
            fill.get().or(None).unwrap_or(CanvasColor::Auto),
            theme.text.get(),
        )
    });
    let typed = move |color: Color32| {
        let [red, green, blue, alpha] = color.to_array();
        set_fill(
            state,
            Some(CanvasColor::Rgba {
                red,
                green,
                blue,
                alpha,
            }),
        );
    };
    let none = create_memo(move || fill.get() == CommonValue::Uniform(None));
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Separator />
                    <Text style=TextStyle::CAPTION string="Rectangle" />
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                        <ToggleButton
                            label="No fill"
                            glyph={ICON_FORMAT_COLOR_RESET.to_owned()}
                            pressed={Binding::new(none, clearing)}
                            @test_id={"infinite-canvas.fill.none"}
                        />
                        <ForEach keys={(0..PRESETS.len()).collect::<Vec<usize>>()}>|index: usize| {
                            let (name, color) = PRESETS[index];
                            let value = create_memo(move || match fill.get() {
                                CommonValue::Uniform(Some(color)) => CommonValue::Uniform(color),
                                CommonValue::Uniform(None) | CommonValue::None => {
                                    CommonValue::None
                                }
                                CommonValue::Mixed => CommonValue::Mixed,
                            });
                            view! {
                                <Swatch
                                    kind="fill"
                                    name
                                    color
                                    value
                                    on_pick={move |color| set_fill(state, Some(color))}
                                />
                            }
                        }</ForEach>
                    </List>
                    <ColorInput
                        label="Fill color"
                        value={Binding::new(custom, typed)}
                        @test_id={"infinite-canvas.fill"}
                    />
                    <MixedLabel label="Corner radius" mixed={mixed} />
                    <Slider
                        label="Corner radius"
                        min=0.0
                        max=100.0
                        value={Binding::new(corner, rounded)}
                        @test_id={"infinite-canvas.corner-radius"}
                    />
                </List>
            </Show>
        </List>
    }
}

fn set_fill(state: CanvasState, fill: Option<CanvasColor>) {
    state.remember_fill(fill);
    state.update_selected(
        |kind| matches!(kind, CanvasEntityKind::Rectangle),
        |style| style.fill = fill,
    );
}

#[component]
fn OpacityRow(state: CanvasState) -> NodeId {
    let value = create_memo(move || {
        common_value(
            state
                .selected_entities()
                .iter()
                .map(|entity| entity.style.opacity),
        )
    });
    let mixed = create_memo(move || value.get().mixed());
    let opacity = create_memo(move || value.get().or(1.0));
    let changed = move |value: f32| {
        state.update_selected(|_| true, |style| style.opacity = value);
    };
    view! {
        <List spacing=4.0>
            <Separator />
            <MixedLabel label="Opacity" mixed={mixed} />
            <Slider
                label="Opacity"
                min=0.0
                max=1.0
                value={Binding::new(opacity, changed)}
                @test_id={"infinite-canvas.opacity"}
            />
        </List>
    }
}

const ALIGN_OPTIONS: [(&str, CanvasTextAlign); 3] = [
    ("Left", CanvasTextAlign::Left),
    ("Center", CanvasTextAlign::Center),
    ("Right", CanvasTextAlign::Right),
];

const VERTICAL_ALIGN_OPTIONS: [(&str, CanvasTextVerticalAlign); 3] = [
    ("Top", CanvasTextVerticalAlign::Top),
    ("Middle", CanvasTextVerticalAlign::Middle),
    ("Bottom", CanvasTextVerticalAlign::Bottom),
];

#[component]
fn TextOptions(state: CanvasState) -> NodeId {
    let styles = create_memo(move || {
        state
            .selected_entities()
            .iter()
            .filter_map(|entity| match &entity.kind {
                CanvasEntityKind::Text { text_style, .. } => Some(*text_style),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    let shown = create_memo(move || !styles.with(Vec::is_empty));
    let font_size = create_memo(move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.font_size)
                .collect::<Vec<f32>>()
        }))
        .or(18.0) as f64
    });
    let line_height = create_memo(move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.line_height)
                .collect::<Vec<f32>>()
        }))
        .or(1.2) as f64
    });
    let bold = create_memo(move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.weight == CanvasTextWeight::Bold)
                .collect::<Vec<bool>>()
        }))
        .or(false)
    });
    let wrap = create_memo(move || {
        common_value(
            styles.with(|styles| styles.iter().map(|style| style.wrap).collect::<Vec<bool>>()),
        )
        .or(false)
    });
    let alignment = create_memo(move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.alignment)
                .collect::<Vec<CanvasTextAlign>>()
        }))
    });
    let vertical_alignment = create_memo(move || {
        common_value(styles.with(|styles| {
            styles
                .iter()
                .map(|style| style.vertical_alignment)
                .collect::<Vec<CanvasTextVerticalAlign>>()
        }))
    });
    let sized = move |value: f64| {
        let size = (value as f32).clamp(4.0, 256.0);
        state.update_selected_text(move |style| style.font_size = size);
    };
    let spaced = move |value: f64| {
        let height = (value as f32).clamp(0.5, 4.0);
        state.update_selected_text(move |style| style.line_height = height);
    };
    let weighted = move |bold: bool| {
        state.update_selected_text(move |style| {
            style.weight = match bold {
                true => CanvasTextWeight::Bold,
                false => CanvasTextWeight::Regular,
            };
        });
    };
    let wrapped = move |wrap: bool| {
        state.update_selected_text(move |style| style.wrap = wrap);
    };
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=6.0>
                    <Separator />
                    <Text style=TextStyle::CAPTION string="Text" />
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Text style=TextStyle::CAPTION string="Font size" />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            label="Font size"
                            min=4.0
                            max=256.0
                            value={Binding::new(font_size, sized)}
                            decimals={Some(2)}
                            @test_id={"infinite-canvas.font-size"}
                        />
                    </List>
                    <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                        <Text style=TextStyle::CAPTION string="Line height" />
                        <NumberInput
                            @sizing=ItemSize::Percent(100.0)
                            label="Line height"
                            min=0.5
                            max=4.0
                            value={Binding::new(line_height, spaced)}
                            decimals={Some(2)}
                            drag={NumberDrag::Linear { speed: 0.05 }}
                            @test_id={"infinite-canvas.line-height"}
                        />
                    </List>
                    <Checkbox
                        label="Bold"
                        checked={Binding::new(bold, weighted)}
                        @test_id={"infinite-canvas.bold"}
                    />
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                        <ForEach
                            keys={(0..ALIGN_OPTIONS.len()).collect::<Vec<usize>>()}
                        >|index: usize| {
                            let (label, wanted) = ALIGN_OPTIONS[index];
                            let pressed = create_memo(move || {
                                alignment.get() == CommonValue::Uniform(wanted)
                            });
                            view! {
                                <ToggleButton
                                    label={label}
                                    pressed={Binding::new(pressed, move |_: bool| {
                                        state.update_selected_text(move |style| {
                                            style.alignment = wanted;
                                        });
                                    })}
                                    @test_id={format!("infinite-canvas.align-text.{label}")}
                                />
                            }
                        }</ForEach>
                    </List>
                    <List direction=Direction::Horizontal align=Align::Center spacing=4.0 wrap=true>
                        <ForEach
                            keys={(0..VERTICAL_ALIGN_OPTIONS.len()).collect::<Vec<usize>>()}
                        >|index: usize| {
                            let (label, wanted) = VERTICAL_ALIGN_OPTIONS[index];
                            let pressed = create_memo(move || {
                                vertical_alignment.get() == CommonValue::Uniform(wanted)
                            });
                            view! {
                                <ToggleButton
                                    label={label}
                                    pressed={Binding::new(pressed, move |_: bool| {
                                        state.update_selected_text(move |style| {
                                            style.vertical_alignment = wanted;
                                        });
                                    })}
                                    @test_id={format!("infinite-canvas.align-text.{label}")}
                                />
                            }
                        }</ForEach>
                    </List>
                    <Checkbox
                        label="Wrap text"
                        checked={Binding::new(wrap, wrapped)}
                        @test_id={"infinite-canvas.wrap"}
                    />
                    <Text
                        style=TextStyle::CAPTION
                        string="Resize to wrap; hold Alt while resizing to scale text."
                        wrap=true
                    />
                </List>
            </Show>
        </List>
    }
}

const SHORTCUTS: [(&str, &str); 10] = [
    ("Pan / Select", "H / V"),
    ("Rectangle / Line", "R / L"),
    ("Text / Pen", "T / P"),
    ("Pan", "Drag empty space, space-drag, or two fingers"),
    ("Zoom", "Ctrl-scroll or pinch"),
    ("Select all", "Ctrl+A"),
    ("Nudge", "Arrow keys; Shift for 10×"),
    ("Duplicate", "Ctrl+D or Alt-drag"),
    ("Edit selected block", "Enter"),
    ("Exit tool or editor", "Escape"),
];

#[component]
fn ShortcutsSection() -> NodeId {
    let open = create_signal(false);
    view! {
        <Accordion title="Shortcuts" open={open}>
            <List spacing=4.0>
                <ForEach keys={(0..SHORTCUTS.len()).collect::<Vec<usize>>()}>|index: usize| {
                    let (action, keys) = SHORTCUTS[index];
                    view! {
                        <Shortcut keys={keys} description={action} />
                    }
                }</ForEach>
            </List>
        </Accordion>
    }
}
