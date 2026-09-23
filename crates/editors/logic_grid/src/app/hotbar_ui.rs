use std::cell::{Cell, RefCell};

use beui::icons::{ICON_ADD, ICON_ARROW_UPWARD};
use beui::reactive::{
    Align, Canvas, CanvasItem, Child, ClickCatcher, Direction, Focusable, ForEach, Frame, ItemSize,
    List, Memo, Picture, ReadSignal, Show, Spacer, Text, clone, component, component_rect,
    create_memo, create_signal, on_cleanup, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Caption, ContextMenu, Dialog, IconButton, MenuButton, Tabs,
    TextInput, Tooltip, use_theme,
};
use beui::unstyled::{ChoiceOption, MenuItem};
use beui::{CursorIcon, ImageFit, NodeId, PointerPress, TextAlign};

use super::glyph::{GLYPH_HEIGHT, GLYPH_WIDTH, Glyph};
use super::session::Session;
use super::*;

const SLOT_SIZE: f32 = 64.0;
const SLOT_INSET: f32 = 4.0;
const BADGE_SIZE: f32 = 15.0;
const SLOT_FONT: f32 = 9.0;
const COLUMN_WIDTH: f32 = 84.0;
const COLUMN_GAP: f32 = 8.0;
const COLUMN_PADDING: f32 = 6.0;
const DRAG_DISTANCE: f32 = 4.0;
const SPACING: f32 = 8.0;
const DIALOG_WIDTH: f32 = 300.0;
pub(super) const HOTBAR_WIDTH: f32 = COLUMN_WIDTH * 2.0 + COLUMN_GAP + 28.0;

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

struct Zone {
    id: u64,
    rect: ReadSignal<Rect>,
    priority: u8,
    target: Rc<dyn Fn() -> Option<HotbarDropTarget>>,
}

#[derive(Default)]
pub(super) struct DropZones {
    zones: RefCell<Vec<Zone>>,
    next: Cell<u64>,
}

impl DropZones {
    fn register(
        self: &Rc<Self>,
        priority: u8,
        target: impl Fn() -> Option<HotbarDropTarget> + 'static,
    ) {
        let id = self.next.get();
        self.next.set(id + 1);
        self.zones.borrow_mut().push(Zone {
            id,
            rect: component_rect(),
            priority,
            target: Rc::new(target),
        });
        let zones = Rc::downgrade(self);
        on_cleanup(move || {
            if let Some(zones) = zones.upgrade() {
                zones.zones.borrow_mut().retain(|zone| zone.id != id);
            }
        });
    }

    fn target_at(&self, at: Pos2) -> Option<HotbarDropTarget> {
        let target = self
            .zones
            .borrow()
            .iter()
            .filter(|zone| zone.rect.get_untracked().contains(at))
            .max_by_key(|zone| zone.priority)
            .map(|zone| Rc::clone(&zone.target))?;
        target()
    }
}

#[component]
pub(super) fn Hotbar(session: Rc<Session>) -> NodeId {
    let zones = Rc::new(DropZones::default());
    let header_zones = Rc::clone(&zones);
    let nested = create_memo(clone!(session -> move || {
        session.read(|model| !model.active_hotbar_folder.is_empty())
    }));
    let top = create_memo(clone!(nested -> move || !nested.get()));
    let title = create_memo(clone!(session -> move || {
        session.read(|model| match model.active_hotbar_folder.is_empty() {
            true => "Hotbar".to_owned(),
            false => hotbar_folder_name(&model.hotbar, &model.active_hotbar_folder).to_owned(),
        })
    }));
    let columns = create_memo(clone!(nested -> move || match nested.get() {
        true => vec![0_usize, 1],
        false => vec![0_usize],
    }));
    let up = clone!(session -> move || {
        session.update(|model| {
            model.active_hotbar_folder.pop();
        });
    });
    let chose = clone!(session -> move |path: Vec<usize>| match path.first() {
        Some(0) => session.update(LogicGridEditor::new_hotbar_folder),
        Some(1) => session.update(|model| model.confirm_hotbar_reset = true),
        _ => {}
    });
    let column_session = Rc::clone(&session);
    let column_zones = Rc::clone(&zones);
    view! {
        <List spacing=SPACING>
            <HotbarHeader session={Rc::clone(&session)} zones={header_zones}>
                <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                    <IconButton
                        glyph={ICON_ARROW_UPWARD.to_owned()}
                        label="Up to the parent folder"
                        disabled={top}
                        @test_id={"logic-grid.hotbar-up"}
                        on_click={up}
                    />
                    <Caption @sizing=ItemSize::Percent(100.0) content={title} />
                    <MenuButton
                        label="Hotbar actions"
                        glyph={ICON_ADD.to_owned()}
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
                <ForEach keys={columns}>
                    {move |column: usize| {
                        let session = Rc::clone(&column_session);
                        let zones = Rc::clone(&column_zones);
                        view! {
                            <HotbarColumn
                                @sizing=ItemSize::Fixed(COLUMN_WIDTH)
                                session={session}
                                zones={zones}
                                column={column}
                            />
                        }
                    }}
                </ForEach>
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <ResetDialog session={session} />
        </List>
    }
}

#[component]
fn HotbarHeader(session: Rc<Session>, zones: Rc<DropZones>, children: Child) -> NodeId {
    zones.register(
        0,
        clone!(session -> move || {
            Some(HotbarDropTarget::Folder(
                session.peek(|model| model.active_hotbar_folder.clone()),
            ))
        }),
    );
    view! {
        <Frame>{children}</Frame>
    }
}

#[component]
fn HotbarColumn(session: Rc<Session>, zones: Rc<DropZones>, column: usize) -> NodeId {
    zones.register(
        1,
        clone!(session -> move || {
            Some(HotbarDropTarget::Folder(
                session.peek(|model| model.column_folder(column)),
            ))
        }),
    );
    let shown = create_memo(clone!(session -> move || {
        session.read(|model| model.column_view(column))
    }));
    let title = create_memo(clone!(shown -> move || shown.get().title));
    let entries = create_memo(clone!(shown -> move || shown.get().entries));
    let empty = create_memo(clone!(entries -> move || entries.get().is_empty()));
    let theme = use_theme();
    let fill = create_memo(clone!(theme shown -> move || match shown.get().active {
        true => theme.accent_soft.get(),
        false => theme.surface_raised.get(),
    }));
    view! {
        <Frame
            color={fill}
            radius=4
            padding_horizontal=COLUMN_PADDING
            padding_vertical=COLUMN_PADDING
        >
            <List align=Align::Center spacing=4.0>
                <Caption content={title} align=TextAlign::Center />
                <Show condition={empty}>
                    <Frame width=SLOT_SIZE height=SLOT_SIZE />
                </Show>
                <ForEach keys={entries}>
                    {move |path: Vec<usize>| {
                        let session = Rc::clone(&session);
                        let zones = Rc::clone(&zones);
                        view! {
                            <HotbarSlotButton session={session} zones={zones} path={path} />
                        }
                    }}
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn HotbarSlotButton(session: Rc<Session>, zones: Rc<DropZones>, path: Vec<usize>) -> NodeId {
    zones.register(
        2,
        clone!(session path -> move || {
            session.peek(|model| hotbar_slot_drop_target(&model.hotbar, &path))
        }),
    );
    let shown = create_memo(clone!(session path -> move || {
        session.read(|model| model.slot_view(&path))
    }));
    let label = create_memo(clone!(shown -> move || {
        shown.get().map(|slot| slot.label).unwrap_or_default()
    }));
    let menu_disabled = create_memo(clone!(shown -> move || {
        !shown.get().is_some_and(|slot| slot.component || slot.folder)
    }));
    let not_component = create_memo(clone!(shown -> move || {
        !shown.get().is_some_and(|slot| slot.component)
    }));
    let not_folder = create_memo(clone!(shown -> move || {
        !shown.get().is_some_and(|slot| slot.folder)
    }));
    let not_removable = create_memo(clone!(shown -> move || {
        !shown.get().is_some_and(|slot| slot.removable)
    }));
    let chose = clone!(session path -> move |chosen: Vec<usize>| match chosen.first() {
        Some(0) => session.update(|model| model.remove_hotbar_slot(&path)),
        Some(1) => session.update(|model| model.toggle_hotbar_folder(path.clone())),
        Some(2) => session.update(|model| model.remove_hotbar_folder(&path)),
        _ => {}
    });
    let items = view! {
        <MenuItem label="Remove from hotbar" disabled={not_component} />
        <MenuItem label="Open folder" disabled={not_folder} />
        <MenuItem label="Remove folder" disabled={not_removable} />
    };

    let (hovered, set_hovered) = create_signal(false);
    let on_hover_change = move |over: bool| set_hovered.set(over);
    let pressed_at = Rc::new(Cell::new(None::<Pos2>));
    let disabled = create_memo(clone!(shown -> move || {
        shown.get().is_none_or(|slot| slot.disabled)
    }));
    let on_press = clone!(pressed_at -> move |press: PointerPress| pressed_at.set(Some(press.pos)));
    let on_drag = clone!(session path pressed_at disabled -> move |press: PointerPress| {
        let Some(start) = pressed_at.get() else {
            return;
        };
        if disabled.get_untracked()
            || (press.pos - start).length() < DRAG_DISTANCE
            || session.peek(|model| model.hotbar_drag.is_some())
        {
            return;
        }
        session.update(|model| model.hotbar_drag = Some(path.clone()));
    });
    let on_click_at = clone!(session path zones disabled -> move |press: PointerPress| {
        if session.peek(|model| model.hotbar_drag.is_some()) {
            let target = zones.target_at(press.pos);
            session.update(|model| model.drop_hotbar_slot(target));
        } else if !disabled.get_untracked() {
            session.update(|model| model.click_hotbar_slot(path.clone()));
        }
    });
    let on_active_change = clone!(session pressed_at -> move |active: bool| {
        if active {
            return;
        }
        pressed_at.set(None);
        if session.peek(|model| model.hotbar_drag.is_some()) {
            session.update(|model| model.hotbar_drag = None);
        }
    });
    let on_activate = clone!(session path disabled -> move || {
        if !disabled.get_untracked() {
            session.update(|model| model.click_hotbar_slot(path.clone()));
        }
    });
    let test_id = format!(
        "logic-grid.slot.{}",
        path.iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(".")
    );
    view! {
        <ContextMenu items={items} disabled={menu_disabled} on_select={chose}>
            <Tooltip label={label}>
                <Focusable on_activate={on_activate}>
                    <ClickCatcher
                        cursor=CursorIcon::PointingHand
                        @test_id={test_id}
                        on_press={on_press}
                        on_drag={on_drag}
                        on_click_at={on_click_at}
                        on_active_change={on_active_change}
                        on_hover_change={on_hover_change}
                    >
                        <SlotFace shown={shown} hovered={hovered} />
                    </ClickCatcher>
                </Focusable>
            </Tooltip>
        </ContextMenu>
    }
}

#[component]
fn SlotFace(shown: Memo<Option<SlotView>>, hovered: ReadSignal<bool>) -> NodeId {
    let theme = use_theme();
    let fill = create_memo(clone!(theme shown hovered -> move || {
        let Some(slot) = shown.get() else {
            return theme.surface.get();
        };
        if slot.selected {
            theme.accent_soft.get()
        } else if slot.open_folder {
            theme.pressed.get()
        } else if slot.disabled || slot.dragging {
            theme.surface.get()
        } else if hovered.get() {
            theme.hover.get()
        } else {
            theme.surface_raised.get()
        }
    }));
    let outline = create_memo(clone!(theme shown -> move || {
        match shown.get() {
            Some(slot) if slot.selected || slot.open_folder || slot.dragging || slot.droppable => {
                theme.accent.get()
            }
            _ => theme.border.get(),
        }
    }));
    let text = create_memo(clone!(theme shown -> move || {
        match shown.get().is_none_or(|slot| slot.disabled || slot.dragging) {
            true => theme.text_muted.get(),
            false => theme.text.get(),
        }
    }));
    let image = create_memo(clone!(shown -> move || shown.get().map(|slot| slot.glyph.image())));
    let tint = create_memo(clone!(shown -> move || {
        match shown.get().is_none_or(|slot| slot.disabled) {
            true => Color32::from_rgba_unmultiplied(255, 255, 255, 120),
            false => Color32::WHITE,
        }
    }));
    let label = create_memo(clone!(shown -> move || {
        shown.get().map(|slot| slot.label).unwrap_or_default()
    }));
    let hotkey = create_memo(clone!(shown -> move || {
        shown.get().and_then(|slot| slot.hotkey).unwrap_or_default().to_owned()
    }));
    let badged = create_memo(clone!(hotkey -> move || !hotkey.get().is_empty()));
    let badge_text = text.clone();
    view! {
        <Frame
            width=SLOT_SIZE
            height=SLOT_SIZE
            color={fill}
            outline={outline}
            outline_width=1.5
            outline_visible=true
            radius=4
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
                        string={label}
                        font_size=SLOT_FONT
                        color={text}
                        align=TextAlign::Center
                        vertical_align=TextAlign::Center
                        clip=true
                    />
                </CanvasItem>
                <CanvasItem x=SLOT_INSET y=SLOT_INSET width=BADGE_SIZE height=BADGE_SIZE>
                    <Frame visible={badged} color={theme.background.clone()} radius=3>
                        <Text
                            string={hotkey}
                            font_size=SLOT_FONT
                            color={badge_text}
                            align=TextAlign::Center
                            vertical_align=TextAlign::Center
                        />
                    </Frame>
                </CanvasItem>
            </Canvas>
        </Frame>
    }
}

#[component]
fn ResetDialog(session: Rc<Session>) -> NodeId {
    let open = create_memo(clone!(session -> move || {
        session.read(|model| model.confirm_hotbar_reset)
    }));
    let reset = clone!(session -> move || {
        session.update(|model| {
            model.confirm_hotbar_reset = false;
            model.reset_hotbar();
        });
    });
    let cancel = clone!(session -> move || {
        session.update(|model| model.confirm_hotbar_reset = false);
    });
    let dismiss = clone!(session -> move || {
        session.update(|model| model.confirm_hotbar_reset = false);
    });
    view! {
        <Dialog open={open} title="Reset hotbar?" width=DIALOG_WIDTH on_dismiss={dismiss}>
            <List spacing=SPACING>
                <Body content="This will restore the default hotbar layout." />
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Button
                        label="Reset"
                        variant=ButtonVariant::Primary
                        @test_id={"logic-grid.hotbar-reset"}
                        on_click={reset}
                    />
                    <Button label="Cancel" variant=ButtonVariant::Secondary on_click={cancel} />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
pub(super) fn ToolSettings(session: Rc<Session>) -> NodeId {
    let merger = create_memo(clone!(session -> move || {
        session.read(|model| model.tool.kind == ToolKind::MergerSplitter)
    }));
    let single = create_memo(clone!(merger -> move || !merger.get()));
    let labelled = create_memo(clone!(session -> move || {
        session.read(|model| {
            matches!(model.tool.kind, ToolKind::Input | ToolKind::Output)
                && model.challenge.is_none()
        })
    }));
    let label =
        create_memo(clone!(session -> move || session.read(|model| model.io_label.clone())));
    let typed =
        clone!(session -> move |text: String| session.update(|model| model.io_label = text));
    let input_session = Rc::clone(&session);
    let output_session = Rc::clone(&session);
    view! {
        <List spacing=SPACING>
            <Show condition={single}>
                <List spacing=4.0>
                    <Caption content="Scale" />
                    <ScalePicker session={Rc::clone(&session)} output=false />
                </List>
            </Show>
            <Show condition={merger}>
                <List spacing=4.0>
                    <Caption content="Input scale" />
                    <ScalePicker session={input_session} output=false />
                    <Caption content="Output scale" />
                    <ScalePicker session={output_session} output=true />
                </List>
            </Show>
            <Show condition={labelled}>
                <TextInput
                    value={label}
                    label="Label"
                    placeholder="Label"
                    @test_id={"logic-grid.io-label"}
                    on_change={typed}
                />
            </Show>
            <Caption content="Middle drag or wheel: pan" />
            <Caption content="Ctrl+wheel: zoom" />
            <Caption content="Shift: add or remove" />
            <Caption content="Delete: remove" />
            <Caption content="Esc: cancel" />
        </List>
    }
}

#[component]
fn ScalePicker(session: Rc<Session>, output: bool) -> NodeId {
    let selected = create_memo(clone!(session -> move || {
        session.read(|model| {
            let scale = match output {
                true => model.tool.merger_out_scale,
                false => model.tool.scale,
            };
            SCALES
                .iter()
                .position(|value| i64::from(*value) == scale.get())
                .unwrap_or_default()
        })
    }));
    let chose = move |index: usize| {
        let Some(scale) = SCALES.get(index).and_then(|value| Scale::new(*value).ok()) else {
            return;
        };
        session.update(|model| match output {
            true => model.tool.merger_out_scale = scale,
            false => model.tool.scale = scale,
        });
    };
    let options = view! {
        <ForEach keys={SCALES.to_vec()}>
            {move |value: u8| view! {
                <ChoiceOption label={format!("{value}x")} />
            }}
        </ForEach>
    };
    view! {
        <Tabs options={options} selected={selected} on_change={chose} />
    }
}
