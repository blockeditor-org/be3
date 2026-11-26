use beui::Overflow;
use beui::icons::{ICON_ARROW_UP, ICON_PLUS};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, ImageFit, NodeId, TextAlign};

use super::glyph::{GLYPH_HEIGHT, GLYPH_WIDTH, Glyph};
use super::session::Session;
use super::*;
use beui::{Align, Direction, PointerPress, drag};
use std::cell::RefCell;

const SLOT_SIZE: f32 = 64.0;
const SLOT_INSET: f32 = 4.0;
const BADGE_SIZE: f32 = 15.0;
const COLUMN_WIDTH: f32 = 84.0;
const COLUMN_GAP: Space = Space::Md;
const COLUMN_PADDING: Space = Space::Sm;
const SPACING: Space = Space::Md;
pub(super) const HOTBAR_WIDTH: f32 = COLUMN_WIDTH * 2.0 + COLUMN_GAP.px() + 28.0;

#[derive(Clone, Debug, PartialEq)]
struct ColumnView {
    title: String,
    active: bool,
    entries: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq)]
struct SlotView {
    label: String,
    glyph: Glyph,
    disabled: bool,
    selected: bool,
    open_folder: bool,
    dragging: bool,
    droppable: bool,
    hotkey: Option<&'static str>,
    component: bool,
    folder: bool,
    removable: bool,
}

impl LogicGridEditor {
    fn column_view(&self, column: usize) -> ColumnView {
        let rows = visible_hotbar_rows(&self.hotbar, &self.active_hotbar_folder);
        let row = &rows[column.min(1)];
        ColumnView {
            title: row.title.clone(),
            active: row.active,
            entries: row.entries.iter().map(|(path, _)| path.clone()).collect(),
        }
    }

    fn column_folder(&self, column: usize) -> Vec<usize> {
        visible_hotbar_rows(&self.hotbar, &self.active_hotbar_folder)[column.min(1)]
            .folder_path
            .clone()
    }

    fn slot_view(&self, path: &[usize]) -> Option<SlotView> {
        let slot = get_hotbar_slot(&self.hotbar, path)?;
        let hotkey = self
            .hotbar_key_entries()
            .iter()
            .position(|(key_path, _)| key_path == path)
            .and_then(hotbar_key_label);
        Some(SlotView {
            label: self.hotbar_slot_label(slot),
            glyph: Glyph::of(slot),
            disabled: self.hotbar_slot_disabled(slot),
            selected: self.active_hotbar_slot.as_deref() == Some(path),
            open_folder: self.active_hotbar_folder == path,
            dragging: self.hotbar_drag.as_deref() == Some(path),
            droppable: self.hotbar_drag.as_ref().is_some_and(|source| {
                source != path
                    && !path.starts_with(source)
                    && hotbar_slot_drop_target(&self.hotbar, path).is_some()
            }),
            hotkey,
            component: matches!(slot, HotbarSlot::Component { .. }),
            folder: matches!(slot, HotbarSlot::Folder { .. }),
            removable: matches!(slot, HotbarSlot::Folder { .. })
                && !hotbar_slot_contains_unremovable(slot),
        })
    }
}

#[component]
pub(super) fn Hotbar(session: Session) -> NodeId {
    let nested = create_memo(move || session.read(|model| !model.active_hotbar_folder.is_empty()));
    let title = create_memo(move || {
        session.read(|model| match model.active_hotbar_folder.is_empty() {
            true => "Hotbar".to_owned(),
            false => hotbar_folder_name(&model.hotbar, &model.active_hotbar_folder).to_owned(),
        })
    });
    let columns = create_memo(move || match nested.get() {
        true => vec![0_usize, 1],
        false => vec![0_usize],
    });
    let go_up = move || {
        session.update(|model| {
            model.active_hotbar_folder.pop();
        });
    };
    let new_folder = move || session.update(LogicGridEditor::new_hotbar_folder);
    let ask_reset = move || session.update(|model| model.confirm_hotbar_reset = true);
    let parent = create_memo(move || {
        session.read(|model| {
            let mut parent = model.active_hotbar_folder.clone();
            parent.pop();
            parent
        })
    });
    let up = NodeRef::new();
    let header_over = create_signal(None::<HotbarDropTarget>);
    let up_border = landing(create_memo(move || {
        header_over
            .with(|over| matches!(over, Some(HotbarDropTarget::Folder(at)) if *at == parent.get()))
    }));
    view! {
        <List spacing=SPACING>
            <HotbarHeader session={session} up={up} parent={parent} over={header_over}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <Frame @node_ref=&up border={up_border} radius=Radius::Sm>
                        <Button
                            icon_only=true
                            glyph=ICON_ARROW_UP
                            label="Up to the parent folder"
                            disabled={move || !nested.get()}
                            @test_id={"logic-grid.hotbar-up"}
                            on_click={go_up}
                        />
                    </Frame>
                    <ListItem grow=1.0>
                        <Text overflow=Overflow::Ellipsis style=TextStyle::CAPTION>{title}</Text>
                    </ListItem>
                    <MenuButton
                        label="Hotbar actions"
                        glyph=ICON_PLUS
                        icon_only=true
                        arrow=false
                        @test_id={"logic-grid.hotbar-actions"}
                        items={view! {
                            <MenuItem label="New folder" on_click={new_folder} />
                            <MenuItem label="Reset hotbar" on_click={ask_reset} />
                        }}
                    />
                </List>
            </HotbarHeader>
            <List direction=Direction::Horizontal spacing=COLUMN_GAP>
                <ForEach value={columns}>|column: usize|
                    <ListItem length=COLUMN_WIDTH>
                        <HotbarColumn session={session} column={column} />
                    </ListItem>
                </ForEach>
                <Spacer />
            </List>
            <ResetDialog session={session} />
        </List>
    }
}

/// The slots a column has built, by path, so its drop site can offer each as a place.
type Slots = Stored<RefCell<Vec<(Vec<usize>, NodeRef)>>>;

/// Whether `source` can move to `target`: not onto itself or into itself.
fn takes(source: &[usize], target: &[usize]) -> bool {
    source != target && !target.starts_with(source)
}

/// A drop site whose places are `slots` (each chosen while the pointer is on it), then each
/// folder's area in `folders`, inner ones first: dropped on a slot the item moves there,
/// elsewhere in an area into its folder.
fn use_hotbar_site(
    session: Session,
    slots: Option<Slots>,
    folders: Vec<(NodeRef, Memo<Vec<usize>>)>,
) -> DropSiteHandle<HotbarDropTarget> {
    use_drop_site(
        Func::new(|_: Vec<usize>| true),
        Func::new(move |source: Vec<usize>| {
            let slots = slots
                .map(|slots| slots.value().borrow().clone())
                .unwrap_or_default();
            let slots = slots
                .into_iter()
                .filter(|(path, _)| takes(&source, path))
                .filter_map(|(path, node)| place_on(node, HotbarDropTarget::Slot(path)));
            let folders = folders.iter().filter_map(|(area, folder)| {
                let folder = folder.get_untracked();
                takes(&source, &folder)
                    .then(|| place_on(*area, HotbarDropTarget::Folder(folder)))
                    .flatten()
            });
            drag::closest(slots.chain(folders).collect::<Vec<_>>()).when_far(drag::floating())
        }),
        Callback::new(move |(source, target): (Vec<usize>, HotbarDropTarget)| {
            session.update(|model| {
                model.hotbar_drag = Some(source);
                model.drop_hotbar_slot(Some(target));
            });
        }),
    )
}

/// A ring around whatever a carried slot would land on.
fn landing(over: Memo<bool>) -> Memo<Option<Border>> {
    create_memo(move || {
        over.get().then_some(
            Border::new(ColorRole::Accent)
                .width(Stroke::Focus)
                .offset(Stroke::Focus),
        )
    })
}

#[component]
fn HotbarHeader(
    session: Session,
    up: NodeRef,
    parent: Memo<Vec<usize>>,
    over: Signal<Option<HotbarDropTarget>>,
    children: Child,
) -> NodeId {
    let folder = create_memo(move || session.read(|model| model.active_hotbar_folder.clone()));
    let area = NodeRef::new();
    // The up button, inside the header, takes an item to the parent folder before the header
    // takes it into the open one.
    let site = use_hotbar_site(session, None, vec![(up, parent), (area, folder)]);
    create_effect(move || over.set(site.over.get()));
    let border = landing(create_memo(move || {
        site.over
            .with(|over| matches!(over, Some(HotbarDropTarget::Folder(at)) if *at == folder.get()))
    }));
    view! {
        <Frame @node_ref=&area border={border} radius=Radius::Sm>{children}</Frame>
    }
}

#[component]
fn HotbarColumn(session: Session, column: usize) -> NodeId {
    let folder = create_memo(move || session.read(|model| model.column_folder(column)));
    let shown = create_memo(move || session.read(|model| model.column_view(column)));
    let title = create_memo(move || shown.get().title);
    let entries = create_memo(move || shown.get().entries);
    let fill = create_memo(move || match shown.get().active {
        true => ColorRole::AccentSoft,
        false => ColorRole::Control,
    });

    let slots: Slots = store(RefCell::default());
    let area = NodeRef::new();
    let site = use_hotbar_site(session, Some(slots), vec![(area, folder)]);
    let over = site.over;
    let border = landing(create_memo(move || {
        over.with(|over| matches!(over, Some(HotbarDropTarget::Folder(_))))
    }));
    view! {
        <Frame @node_ref=&area border={border} radius=Radius::Sm>
            <Frame
                @test_id={format!("logic-grid.column.{column}")}
                fill={fill}
                radius=Radius::Sm
                padding=COLUMN_PADDING
            >
                <List align=Align::Center spacing=Space::Xs>
                    <Text style=TextStyle::CAPTION align=TextAlign::Center>{title}</Text>
                    <ForEach
                        value={entries}
                        fallback={|| view! {
                            <Frame
                                width={Size::Custom(SLOT_SIZE)}
                                height={Size::Custom(SLOT_SIZE)}
                            />
                        }}
                    >|path: Vec<usize>|
                        <HotbarSlotButton session={session} path={path} slots={slots} over={over} />
                    </ForEach>
                </List>
            </Frame>
        </Frame>
    }
}

#[component]
fn HotbarSlotButton(
    session: Session,
    path: Vec<usize>,
    slots: Slots,
    over: ReadSignal<Option<HotbarDropTarget>>,
) -> NodeId {
    let path = store(path);
    let face = NodeRef::new();
    slots
        .value()
        .borrow_mut()
        .push((path.value().to_vec(), face));
    on_cleanup(move || {
        let gone = path.value().to_vec();
        slots.try_with(|slots| slots.borrow_mut().retain(|(path, _)| *path != gone));
    });
    let shown = create_memo(move || session.read(|model| model.slot_view(&path.value())));
    let label = create_memo(move || shown.get().map(|slot| slot.label).unwrap_or_default());
    let menu_disabled = create_memo(move || {
        !shown
            .get()
            .is_some_and(|slot| slot.component || slot.folder)
    });
    let remove_slot = move || session.update(|model| model.remove_hotbar_slot(&path.value()));
    let open_folder =
        move || session.update(|model| model.toggle_hotbar_folder(path.value().to_vec()));
    let remove_folder = move || session.update(|model| model.remove_hotbar_folder(&path.value()));
    let hovered = create_signal(false);
    let disabled = create_memo(move || shown.get().is_none_or(|slot| slot.disabled));
    let payload = create_memo(move || (!disabled.get()).then(|| path.value().to_vec()));
    let clicked = move || {
        if !disabled.get_untracked() {
            session.update(|model| model.click_hotbar_slot(path.value().to_vec()));
        }
    };
    let carried = is_carried(path.value().to_vec());
    create_effect_on_change(
        move || carried.get(),
        move |carried, _| {
            session.update(|model| model.hotbar_drag = carried.then(|| path.value().to_vec()));
        },
    );
    let ghost = move |dragged: Vec<usize>| {
        let shown = create_memo(move || {
            session
                .read(|model| model.slot_view(&dragged))
                .map(|slot| SlotView {
                    dragging: false,
                    droppable: false,
                    ..slot
                })
        });
        let hovered = create_signal(false);
        view! {
            <Frame @test_id="logic-grid.slot-ghost">
                <SlotFace shown={shown} hovered={hovered.read_only()} />
            </Frame>
        }
    };
    let border = landing(create_memo(move || {
        over.with(
            |over| matches!(over, Some(HotbarDropTarget::Slot(target)) if *target == *path.value()),
        )
    }));
    let drag = Func::new(move |_: PointerPress| {
        let carried = payload.get_untracked()?;
        let shown = carried.clone();
        Some(drag::Carry::new(carried).preview(move || ghost(shown.clone())))
    });
    let element_test_id = format!(
        "logic-grid.slot.{}",
        path.value()
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(".")
    );
    view! {
        <Frame @node_ref=&face border={border} radius=Radius::Sm>
            <ContextMenu
                items={view! {
                    <MenuItem
                        label="Remove from hotbar"
                        disabled={move || !shown.with(|slot| slot.as_ref().is_some_and(|slot| slot.component))}
                        on_click={remove_slot}
                    />
                    <MenuItem
                        label="Open folder"
                        disabled={move || !shown.with(|slot| slot.as_ref().is_some_and(|slot| slot.folder))}
                        on_click={open_folder}
                    />
                    <MenuItem
                        label="Remove folder"
                        disabled={move || !shown.with(|slot| slot.as_ref().is_some_and(|slot| slot.removable))}
                        on_click={remove_folder}
                    />
                }}
                disabled={menu_disabled}
            >
                <Tooltip label={label}>
                    <Interactive
                        @test_id={element_test_id}
                        focusable=true
                        cursor=CursorIcon::PointingHand
                        drag={Some(drag)}
                        on_activate={clicked}
                        on_click={clicked}
                        on_hover_change={hovered}
                    >
                        <SlotFace shown={shown} hovered={hovered.read_only()} />
                    </Interactive>
                </Tooltip>
            </ContextMenu>
        </Frame>
    }
}

#[component]
fn SlotFace(shown: Memo<Option<SlotView>>, hovered: ReadSignal<bool>) -> NodeId {
    let fill = create_memo(move || {
        let Some(slot) = shown.get() else {
            return ColorRole::Surface;
        };
        if slot.selected {
            ColorRole::AccentSoft
        } else if slot.open_folder {
            ColorRole::Pressed
        } else if slot.disabled || slot.dragging {
            ColorRole::Surface
        } else if hovered.get() {
            ColorRole::Hover
        } else {
            ColorRole::Control
        }
    });
    let outline = create_memo(move || match shown.get() {
        Some(slot) if slot.selected || slot.open_folder || slot.dragging || slot.droppable => {
            ColorRole::Accent
        }
        _ => ColorRole::Border,
    });
    let text = create_memo(move || {
        match shown
            .get()
            .is_none_or(|slot| slot.disabled || slot.dragging)
        {
            true => TextColor::Muted,
            false => TextColor::Default,
        }
    });
    let image = create_memo(move || shown.get().map(|slot| slot.glyph.image()));
    let tint = create_memo(move || match shown.get().is_none_or(|slot| slot.disabled) {
        true => Color32::from_rgba_unmultiplied(255, 255, 255, 120),
        false => Color32::WHITE,
    });
    let label = create_memo(move || shown.get().map(|slot| slot.label).unwrap_or_default());
    let hotkey = create_memo(move || {
        shown
            .get()
            .and_then(|slot| slot.hotkey)
            .unwrap_or_default()
            .to_owned()
    });
    view! {
        <Frame
            width={Size::Custom(SLOT_SIZE)}
            height={Size::Custom(SLOT_SIZE)}
            fill={fill}
            border={move || Some(Border::new(outline.get()).width(Stroke::Focus))}
            radius=Radius::Sm
        >
            <Canvas width=SLOT_SIZE height=SLOT_SIZE>
                <CanvasItem x=SLOT_INSET y=SLOT_INSET width=GLYPH_WIDTH height=GLYPH_HEIGHT>
                    <Picture image={image} fit=ImageFit::Fill tint={tint} />
                </CanvasItem>
                <CanvasItem
                    x=SLOT_INSET
                    y={SLOT_INSET + GLYPH_HEIGHT}
                    width=GLYPH_WIDTH
                    height={SLOT_SIZE - GLYPH_HEIGHT - SLOT_INSET * 2.0}
                >
                    <Text
                        size=TextSize::Xs
                        color={text}
                        align=TextAlign::Center
                        vertical_align=TextAlign::Center
                        overflow=Overflow::Ellipsis
                    >
                        {label}
                    </Text>
                </CanvasItem>
                <Show value={move || hotkey.with(|hotkey| !hotkey.is_empty())}>
                    <CanvasItem x=SLOT_INSET y=SLOT_INSET width=BADGE_SIZE height=BADGE_SIZE>
                        <Frame fill=ColorRole::Background radius=Radius::Xs>
                            <Text
                                size=TextSize::Xs
                                color={text}
                                align=TextAlign::Center
                                vertical_align=TextAlign::Center
                            >
                                {hotkey}
                            </Text>
                        </Frame>
                    </CanvasItem>
                </Show>
            </Canvas>
        </Frame>
    }
}

#[component]
fn ResetDialog(session: Session) -> NodeId {
    let open = create_memo(move || session.read(|model| model.confirm_hotbar_reset));
    let reset = move || {
        session.update(|model| {
            model.confirm_hotbar_reset = false;
            model.reset_hotbar();
        });
    };
    let dismiss = move || {
        session.update(|model| model.confirm_hotbar_reset = false);
    };
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title="Reset hotbar?"
            id="logic-grid.hotbar-reset"
            actions={vec![DialogAction::destructive("Reset", reset), DialogAction::cancel("Cancel")]}
            on_dismiss={dismiss}
        >
            <Text>"This will restore the default hotbar layout."</Text>
        </Dialog>
    }
}

#[component]
pub(super) fn ToolSettings(session: Session) -> NodeId {
    let merger =
        create_memo(move || session.read(|model| model.tool.kind == ToolKind::MergerSplitter));
    let labelled = create_memo(move || {
        session.read(|model| {
            matches!(model.tool.kind, ToolKind::Input | ToolKind::Output)
                && model.challenge.is_none()
        })
    });
    let label = create_memo(move || session.read(|model| model.io_label.clone()));
    let typed = move |text: String| session.update(|model| model.io_label = text);
    view! {
        <List spacing=SPACING>
            <Show
                value={merger}
                fallback={move || view! {
                    <List spacing=Space::Xs>
                        <Text style=TextStyle::CAPTION>"Scale"</Text>
                        <ScalePicker session={session} output=false />
                    </List>
                }}
            >
                <List spacing=Space::Xs>
                    <Text style=TextStyle::CAPTION>"Input scale"</Text>
                    <ScalePicker session={session} output=false />
                    <Text style=TextStyle::CAPTION>"Output scale"</Text>
                    <ScalePicker session={session} output=true />
                </List>
            </Show>
            <Show value={labelled}>
                <TextInput
                    value={Binding::new(label, typed)}
                    label="Label"
                    placeholder="Label"
                    @test_id={"logic-grid.io-label"}
                />
            </Show>
            <Text style=TextStyle::CAPTION>"Middle drag or wheel: pan"</Text>
            <Text style=TextStyle::CAPTION>"Ctrl+wheel: zoom"</Text>
            <Text style=TextStyle::CAPTION>"Shift: add or remove"</Text>
            <Text style=TextStyle::CAPTION>"Delete: remove"</Text>
            <Text style=TextStyle::CAPTION>"Esc: cancel"</Text>
        </List>
    }
}

#[component]
fn ScalePicker(session: Session, output: bool) -> NodeId {
    let selected = create_memo(move || {
        session.read(|model| {
            let scale = match output {
                true => model.tool.merger_out_scale,
                false => model.tool.scale,
            };
            SCALES
                .into_iter()
                .find(|value| i64::from(*value) == scale.get())
        })
    });
    let label = match output {
        true => "Output scale",
        false => "Scale",
    }
    .to_owned();
    let chose = move |value: Option<u8>| {
        let Some(scale) = value.and_then(|value| Scale::new(value).ok()) else {
            return;
        };
        session.update(|model| match output {
            true => model.tool.merger_out_scale = scale,
            false => model.tool.scale = scale,
        });
    };
    view! {
        <Select
            label
            options={view! {
                <ForEach value={SCALES.to_vec()}>|value: u8|
                    <ChoiceOption value label={format!("{value}x")} />
                </ForEach>
            }}
            selected={Binding::new(selected, chose)}
        />
    }
}
