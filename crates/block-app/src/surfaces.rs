use std::{cell::RefCell, rc::Rc};

use beui::reactive::{
    Canvas, CanvasItem, Embed, EmbedSlot, ForEach, Frame, KeyedStore, List, Memo, ReadSignal, Show,
    Text, Viewport, WriteSignal, component, create_memo, create_signal, view,
};
use beui::styled::{Button, ButtonVariant, Caption, Heading, Icon, Spinner, use_theme};
use beui::{Align, Color32, Drawing, NodeId, Rect, TextAlign, Vec2};

use crate::host::{self, HostCommand, HostItem, PlacedItem, SurfaceOutput, Ui};
use crate::plugin_host::{Blit, PluginDrawing};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SurfaceId {
    Main,
    Presenting,
    Creation,
    ArtifactSettings,
}

impl SurfaceId {
    const ALL: [Self; 4] = [
        Self::Main,
        Self::Presenting,
        Self::Creation,
        Self::ArtifactSettings,
    ];

    fn index(self) -> usize {
        match self {
            Self::Main => 0,
            Self::Presenting => 1,
            Self::Creation => 2,
            Self::ArtifactSettings => 3,
        }
    }
}

#[derive(Default)]
struct State {
    placement: Option<(Rect, Rect)>,
    output: SurfaceOutput,
    origin: Option<Rect>,
    used: bool,
    height: Option<f32>,
    blits: Vec<Blit>,
}

#[derive(Clone)]
pub(crate) struct SurfaceHandle {
    slot: EmbedSlot,
    drawing: ReadSignal<Option<Drawing>>,
    set_drawing: WriteSignal<Option<Drawing>>,
    items: KeyedStore<u64, PlacedItem>,
    size: ReadSignal<Vec2>,
    set_size: WriteSignal<Vec2>,
    shown: ReadSignal<bool>,
    set_shown: WriteSignal<bool>,
    height: ReadSignal<Option<f32>>,
    set_height: WriteSignal<Option<f32>>,
}

impl SurfaceHandle {
    fn new() -> Self {
        let (drawing, set_drawing) = create_signal(None);
        let (size, set_size) = create_signal(Vec2::ZERO);
        let (shown, set_shown) = create_signal(false);
        let (height, set_height) = create_signal(None);
        Self {
            slot: EmbedSlot::new(),
            drawing,
            set_drawing,
            items: KeyedStore::new(),
            size,
            set_size,
            shown,
            set_shown,
            height,
            set_height,
        }
    }

    pub(crate) fn shown(&self) -> ReadSignal<bool> {
        self.shown.clone()
    }

    pub(crate) fn height(&self) -> ReadSignal<Option<f32>> {
        self.height.clone()
    }
}

thread_local! {
    static STATES: [RefCell<State>; 4] = Default::default();
    static HANDLES: RefCell<Option<Rc<[SurfaceHandle; 4]>>> = const { RefCell::new(None) };
}

pub(crate) fn create_handles() {
    let handles = Rc::new(SurfaceId::ALL.map(|_| SurfaceHandle::new()));
    HANDLES.with(|slot| *slot.borrow_mut() = Some(handles));
}

pub(crate) fn handle(id: SurfaceId) -> SurfaceHandle {
    HANDLES.with(|handles| {
        handles
            .borrow()
            .as_ref()
            .expect("the surface handles are created before the view is built")[id.index()]
        .clone()
    })
}

fn with_state<R>(id: SurfaceId, act: impl FnOnce(&mut State) -> R) -> R {
    STATES.with(|states| act(&mut states[id.index()].borrow_mut()))
}

pub(crate) fn with<R>(id: SurfaceId, act: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let (placement, mut output) = with_state(id, |state| {
        state.used = true;
        (state.placement, std::mem::take(&mut state.output))
    });
    let layer = match id {
        SurfaceId::Main if !placement.is_some_and(|(rect, _)| host::floats(rect)) => 0,
        _ => 1,
    };
    let result = placement.map(|(rect, clip)| {
        let mut ui = Ui::new(&mut output, rect, clip, layer);
        act(&mut ui)
    });
    with_state(id, |state| {
        output.items.append(&mut state.output.items);
        output.blits.append(&mut state.output.blits);
        state.output = output;
        if let Some((rect, _)) = placement {
            state.origin = Some(rect);
        }
    });
    result
}

pub(crate) fn set_height(id: SurfaceId, height: Option<f32>) {
    with_state(id, |state| state.height = height);
}

pub(crate) fn commit() {
    for id in SurfaceId::ALL {
        let handle = handle(id);
        let (output, used, origin, height, placement, changed) = with_state(id, |state| {
            let output = std::mem::take(&mut state.output);
            let changed = output.blits != state.blits || output.blits.iter().any(Blit::pending);
            state.blits.clone_from(&output.blits);
            let used = std::mem::take(&mut state.used);
            (
                output,
                used,
                state.origin,
                state.height,
                state.placement,
                changed,
            )
        });
        handle.set_shown.set(used);
        handle.set_height.set(height);
        let size = placement.map_or(Vec2::ZERO, |(rect, _)| rect.size());
        handle.set_size.set(size);
        if changed {
            let drawing = match output.blits.is_empty() {
                true => None,
                false => Some(Drawing::new(PluginDrawing::new(output.blits))),
            };
            handle.set_drawing.set(drawing);
        }
        let origin = origin.map_or(beui::Pos2::ZERO, |rect| rect.min);
        handle
            .items
            .reconcile_owned(output.items.into_iter().map(|(key, placed)| {
                (
                    key,
                    PlacedItem {
                        rect: placed.rect.translate(beui::Pos2::ZERO - origin),
                        item: placed.item,
                    },
                )
            }));
    }
}

pub(crate) fn read_placements() {
    for id in SurfaceId::ALL {
        let placement = handle(id)
            .slot
            .placement()
            .filter(|placement| placement.rect.is_positive())
            .map(|placement| (placement.rect, placement.clip));
        let shown = handle(id).shown.get_untracked();
        with_state(id, |state| {
            state.placement = if shown || id == SurfaceId::Main {
                placement
            } else {
                None
            };
        });
    }
}

#[component]
pub(crate) fn HostSurface(id: SurfaceId) -> NodeId {
    let handle = handle(id);
    let items = handle.items.clone();
    let width = create_memo({
        let size = handle.size.clone();
        move || size.get().x
    });
    let height = create_memo({
        let size = handle.size.clone();
        move || size.get().y
    });
    view! {
        <Embed slot={handle.slot.clone()} punch=false>
            <Canvas>
                <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                    <Viewport drawing={handle.drawing.clone()} />
                </CanvasItem>
                <ForEach keys={items.keys()}>
                    {move |key: u64| {
                        let item = items.get(&key);
                        view! {
                            <HostItemView item />
                        }
                    }}
                </ForEach>
            </Canvas>
        </Embed>
    }
}

#[component]
fn HostItemView(item: ReadSignal<PlacedItem>) -> CanvasItem {
    let x = create_memo({
        let item = item.clone();
        move || item.get().rect.min.x
    });
    let y = create_memo({
        let item = item.clone();
        move || item.get().rect.min.y
    });
    let width = create_memo({
        let item = item.clone();
        move || item.get().rect.width()
    });
    let height = create_memo({
        let item = item.clone();
        move || item.get().rect.height()
    });
    let content = create_memo(move || item.get().item);
    view! {
        <CanvasItem x={x} y={y} width={width} height={height}>
            <HostItemFace content />
        </CanvasItem>
    }
}

#[component]
fn HostItemFace(content: Memo<HostItem>) -> NodeId {
    let kind = content.get_untracked();
    let text = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Notice { text, .. } | HostItem::Error { text, .. } => text,
            HostItem::Fallback { name, .. } => name,
            HostItem::Unsupported { block, .. } => format!("Block: {block}"),
        }
    });
    let detail = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Unsupported { block_type, .. } => format!("Type: {block_type}"),
            _ => String::new(),
        }
    });
    let italic = create_memo({
        let content = content.clone();
        move || {
            matches!(
                content.get(),
                HostItem::Fallback {
                    automatic: true,
                    ..
                }
            )
        }
    });
    let spinning = create_memo({
        let content = content.clone();
        move || matches!(content.get(), HostItem::Notice { spinner: true, .. })
    });
    let restart = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Error { restart, .. } => restart,
            _ => None,
        }
    });
    let restartable = create_memo({
        let restart = restart.clone();
        move || restart.get().is_some()
    });
    let glyph = create_memo({
        let content = content.clone();
        move || match content.get() {
            HostItem::Fallback { icon, .. } => icon.unwrap_or_default(),
            _ => String::new(),
        }
    });
    let has_glyph = create_memo({
        let glyph = glyph.clone();
        move || !glyph.get().is_empty()
    });
    let theme = use_theme();
    let (surface, outline, color) = match kind {
        HostItem::Fallback { .. } => (
            Color32::from_gray(28),
            Color32::from_gray(75),
            Color32::from_gray(211),
        ),
        HostItem::Error { .. } => (
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            theme.danger.get_untracked(),
        ),
        _ => (
            Color32::TRANSPARENT,
            Color32::TRANSPARENT,
            theme.text_muted.get_untracked(),
        ),
    };
    let unsupported = matches!(kind, HostItem::Unsupported { .. });
    view! {
        <Frame
            color=surface
            outline=outline
            outline_width=1.0
            outline_visible=true
            radius=5
            padding_horizontal=12.0
            padding_vertical=12.0
        >
            <List spacing=8.0 align=Align::Center>
                <Show condition={unsupported}>
                    <Heading content="Unsupported block type" align=TextAlign::Center />
                </Show>
                <Show condition={has_glyph}>
                    <Icon glyph={glyph} text_size=28.0 color=color />
                </Show>
                <Text string={text} font_size=14.0 align=TextAlign::Center color=color italic />
                <Show condition={unsupported}>
                    <Caption content={detail} align=TextAlign::Center />
                </Show>
                <Show condition={spinning}>
                    <Spinner />
                </Show>
                <Show condition={restartable}>
                    <Button
                        label="Restart plugin"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            if let Some(plugin) = restart.get_untracked() {
                                host::push_command(HostCommand::RestartPlugin(plugin));
                            }
                        }}
                    />
                </Show>
            </List>
        </Frame>
    }
}
