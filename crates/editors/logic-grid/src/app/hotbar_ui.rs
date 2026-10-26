use beui::Overflow;
use beui::icons::{ICON_ARROW_UP, ICON_PLUS};
use beui::reactive::*;
use beui::styled::*;
use beui::{CursorIcon, ImageFit, NodeId, TextAlign};

use super::glyph::{GLYPH_HEIGHT, GLYPH_WIDTH, Glyph};
use super::session::Session;
use super::*;

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
    let top = create_memo(move || !nested.get());
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
    let up = move || {
        session.update(|model| {
            model.active_hotbar_folder.pop();
        });
    };
    let chose = move |path: Vec<usize>| match path.first() {
        Some(0) => session.update(LogicGridEditor::new_hotbar_folder),
        Some(1) => session.update(|model| model.confirm_hotbar_reset = true),
        _ => {}
    };
    let parent = create_memo(move || {
        session.read(|model| {
            let mut parent = model.active_hotbar_folder.clone();
            parent.pop();
            parent
        })
    });
    view! {
        <List spacing=SPACING>
            <HotbarHeader session={session}>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                    <SlotDrop session={session} target={parent} folder=true>
                        <Button
                            icon_only=true
                            glyph=ICON_ARROW_UP
                            label="Up to the parent folder"
                            disabled={top}
                            @test_id={"logic-grid.hotbar-up"}
                            on_click={up}
                        />
                    </SlotDrop>
                    <Text
                        overflow=Overflow::Ellipsis
                        style=TextStyle::CAPTION
                        @sizing=ItemSize::Percent(100.0)
                    >
                        {title}
                    </Text>
                    <MenuButton
                        label="Hotbar actions"
                        glyph=ICON_PLUS
                        icon_only=true
                        arrow=false
                        @test_id={"logic-grid.hotbar-actions"}
                        items={view! {
                            <MenuItem label="New folder" />
                            <MenuItem label="Reset hotbar" />
                        }}
                        on_select={chose}
                    />
                </List>
            </HotbarHeader>
            <List direction=Direction::Horizontal spacing=COLUMN_GAP>
                <ForEach keys={columns}>|column: usize|
                    <HotbarColumn
                        @sizing=ItemSize::Fixed(COLUMN_WIDTH)
                        session={session}
                        column={column}
                    />
                </ForEach>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <ResetDialog session={session} />
        </List>
    }
}

#[component]
fn SlotDrop(session: Session, target: Memo<Vec<usize>>, folder: bool, children: Child) -> NodeId {
    let accepts = Func::new(move |source: Vec<usize>| {
        let target = target.get_untracked();
        source != target && !target.starts_with(&source)
    });
    let dropped = move |(source, _): (Vec<usize>, DragPoint)| {
        let target = target.get_untracked();
        let target = match folder {
            true => HotbarDropTarget::Folder(target),
            false => HotbarDropTarget::Slot(target),
        };
        session.update(|model| {
            model.hotbar_drag = Some(source);
            model.drop_hotbar_slot(Some(target));
        });
    };
    view! {
        <DropTarget accepts={accepts} on_drop={dropped}>|handle: DropHandle| {
            let DropHandle { over, .. } = handle;
            view! {
                <Frame
                    border={create_memo(move || over.get().then_some(Border::new(ColorRole::Accent).width(Stroke::Focus).offset(Stroke::Focus)))}
                    radius=Radius::Sm
                >
                    {children}
                </Frame>
            }
        }</DropTarget>
    }
}

#[component]
fn HotbarHeader(session: Session, children: Child) -> NodeId {
    let folder = create_memo(move || session.read(|model| model.active_hotbar_folder.clone()));
    view! {
        <SlotDrop session={session} target={folder} folder=true>{children}</SlotDrop>
    }
}

#[component]
fn HotbarColumn(session: Session, column: usize) -> NodeId {
    let folder = create_memo(move || session.read(|model| model.column_folder(column)));
    let shown = create_memo(move || session.read(|model| model.column_view(column)));
    let title = create_memo(move || shown.get().title);
    let entries = create_memo(move || shown.get().entries);
    let empty = create_memo(move || entries.get().is_empty());
    let fill = create_memo(move || match shown.get().active {
        true => ColorRole::AccentSoft,
        false => ColorRole::SurfaceRaised,
    });

    view! {
        <SlotDrop session={session} target={folder} folder=true>
            <Frame
                @test_id={format!("logic-grid.column.{column}")}
                fill={fill}
                radius=Radius::Sm
                padding=COLUMN_PADDING
            >
                <List align=Align::Center spacing=Space::Xs>
                    <Text style=TextStyle::CAPTION align=TextAlign::Center>{title}</Text>
                    <Show condition={empty}>
                        <Frame width={Size::Custom(SLOT_SIZE)} height={Size::Custom(SLOT_SIZE)} />
                    </Show>
                    <ForEach keys={entries}>|path: Vec<usize>|
                        <HotbarSlotButton session={session} path={path} />
                    </ForEach>
                </List>
            </Frame>
        </SlotDrop>
    }
}

#[component]
fn HotbarSlotButton(session: Session, path: Vec<usize>) -> NodeId {
    let path = store(path);
    let shown = create_memo(move || session.read(|model| model.slot_view(&path.value())));
    let label = create_memo(move || shown.get().map(|slot| slot.label).unwrap_or_default());
    let menu_disabled = create_memo(move || {
        !shown
            .get()
            .is_some_and(|slot| slot.component || slot.folder)
    });
    let not_component = create_memo(move || !shown.get().is_some_and(|slot| slot.component));
    let not_folder = create_memo(move || !shown.get().is_some_and(|slot| slot.folder));
    let not_removable = create_memo(move || !shown.get().is_some_and(|slot| slot.removable));
    let chose = move |chosen: Vec<usize>| match chosen.first() {
        Some(0) => session.update(|model| model.remove_hotbar_slot(&path.value())),
        Some(1) => session.update(|model| model.toggle_hotbar_folder(path.value().to_vec())),
        Some(2) => session.update(|model| model.remove_hotbar_folder(&path.value())),
        _ => {}
    };
    let hovered = create_signal(false);
    let disabled = create_memo(move || shown.get().is_none_or(|slot| slot.disabled));
    let payload = create_memo(move || (!disabled.get()).then(|| path.value().to_vec()));
    let clicked = move || {
        if !disabled.get_untracked() {
            session.update(|model| model.click_hotbar_slot(path.value().to_vec()));
        }
    };
    let carried = move |dragging: bool| {
        session.update(|model| model.hotbar_drag = dragging.then(|| path.value().to_vec()));
    };
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
    let target = create_memo(move || path.value().to_vec());
    let element_test_id = format!(
        "logic-grid.slot.{}",
        path.value()
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(".")
    );
    view! {
        <SlotDrop session={session} target={target} folder=false>
            <ContextMenu
                items={view! {
                    <MenuItem label="Remove from hotbar" disabled={not_component} />
                    <MenuItem label="Open folder" disabled={not_folder} />
                    <MenuItem label="Remove folder" disabled={not_removable} />
                }}
                disabled={menu_disabled}
                on_select={chose}
            >
                <Tooltip label={label}>
                    <Interactive focusable=true on_activate={clicked} on_hover_change={hovered}>
                        <Draggable
                            @test_id={element_test_id}
                            payload={payload}
                            cursor=CursorIcon::PointingHand
                            preview={ghost}
                            on_click={clicked}
                            on_drag_change={carried}
                        >|_: DragHandle|
                            <SlotFace shown={shown} hovered={hovered.read_only()} />
                        </Draggable>
                    </Interactive>
                </Tooltip>
            </ContextMenu>
        </SlotDrop>
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
            ColorRole::SurfaceRaised
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
    let badged = create_memo(move || !hotkey.get().is_empty());
    view! {
        <Frame
            width={Size::Custom(SLOT_SIZE)}
            height={Size::Custom(SLOT_SIZE)}
            fill={fill}
            border={create_memo(move || Some(Border::new(outline.get()).width(Stroke::Focus)))}
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
                <Show condition={badged}>
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
    let single = create_memo(move || !merger.get());
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
            <Show condition={single}>
                <List spacing=Space::Xs>
                    <Text style=TextStyle::CAPTION>"Scale"</Text>
                    <ScalePicker session={session} output=false />
                </List>
            </Show>
            <Show condition={merger}>
                <List spacing=Space::Xs>
                    <Text style=TextStyle::CAPTION>"Input scale"</Text>
                    <ScalePicker session={session} output=false />
                    <Text style=TextStyle::CAPTION>"Output scale"</Text>
                    <ScalePicker session={session} output=true />
                </List>
            </Show>
            <Show condition={labelled}>
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
                .iter()
                .position(|value| i64::from(*value) == scale.get())
        })
    });
    let label = match output {
        true => "Output scale",
        false => "Scale",
    }
    .to_owned();
    let chose = move |index: Option<usize>| {
        let Some(scale) = index
            .and_then(|index| SCALES.get(index))
            .and_then(|value| Scale::new(*value).ok())
        else {
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
                <ForEach keys={SCALES.to_vec()}>|value: u8|
                    <ChoiceOption label={format!("{value}x")} />
                </ForEach>
            }}
            selected={Binding::new(selected, chose)}
        />
    }
}
