use std::{cell::Cell, rc::Rc, sync::Arc};

use beui::reactive::{
    BackHandler, Canvas, CanvasItem, Drawing, Embed, EmbedPlacement, EmbedSlot, ForEach, Frame,
    Interactive, Layers, List, Memo, Prop, Show, clone, component, create_effect, create_memo,
    draw_gpu, on_cleanup, view,
};
use beui::{Align, CursorIcon, ForwardedInput, NodeId, Pos2, Rect};
use block_plugin_api::{
    BlockTypeDescriptor, ChildId, EditorInstanceId, EditorRegion, FrameSpec, PluginManifest,
};
use uuid::Uuid;

use crate::host::HostItem;
use crate::plugin_host::{
    self, EditorView, HostChild, InstanceRole, RegionPlacement, RegionSlot, RegionView,
};
use crate::surfaces::HostItemFace;

#[derive(Clone)]
pub(crate) struct RegionEditor {
    pub(crate) plugin: Arc<PluginManifest>,
    pub(crate) role: InstanceRole,
    pub(crate) instance: EditorInstanceId,
    pub(crate) block_types: Arc<Vec<BlockTypeDescriptor>>,
    pub(crate) client_id: Uuid,
}

impl RegionEditor {
    pub(crate) fn plugin_id(&self) -> &str {
        &self.plugin.identity.id
    }
}

pub(crate) type ChildView = Rc<dyn Fn(ChildId, Memo<Option<HostChild>>, Memo<Rect>) -> NodeId>;

#[component]
pub(crate) fn PluginRegion(
    editor: RegionEditor,
    region: EditorRegion,
    #[prop(default = None)] frame: Prop<Option<FrameSpec>>,
    #[prop(default = None)] view: Prop<Option<EditorView>>,
    #[prop(default = false)] passive: Prop<bool>,
    #[prop(default = 0.0)] rotation: Prop<f32>,
    #[prop(default = 1.0)] opacity: Prop<f32>,
    child_view: ChildView,
) -> NodeId {
    let instance = editor.instance;
    let plugin_id = editor.plugin_id().to_owned();
    plugin_host::mount_region(RegionSlot {
        plugin: &editor.plugin,
        block_types: &editor.block_types,
        client_id: editor.client_id,
        role: editor.role,
        instance,
        region,
    });
    on_cleanup(clone!(plugin_id -> move || {
        plugin_host::unmount_region(&plugin_id, instance, region)
    }));
    let revision = super::listen(&plugin_id);
    let placed: Rc<Cell<Option<EmbedPlacement>>> = Rc::new(Cell::new(None));
    let place: Rc<dyn Fn(Option<EmbedPlacement>)> = Rc::new(clone!(plugin_id frame view -> move |placement: Option<EmbedPlacement>| {
        if let Some(placement) = placement {
            plugin_host::place_region(
                &plugin_id,
                instance,
                region,
                RegionPlacement {
                    rect: placement.rect,
                    clip: placement.clip,
                },
                frame.peek(),
                view.peek(),
            );
        }
    }));
    let slot = EmbedSlot::new();
    slot.on_place(clone!(placed place -> move |placement| {
        placed.set(placement);
        place(placement);
    }));
    create_effect(clone!(placed place frame view -> move || {
        let _ = frame.get();
        let _ = view.get();
        place(placed.get());
    }));
    let state = create_memo(clone!(revision -> move || {
        revision.get();
        plugin_host::region_view(&editor.plugin, instance, region)
    }));
    create_effect(clone!(revision plugin_id -> move || {
        revision.get();
        for action in plugin_host::take_region_actions(&plugin_id, instance) {
            super::act(action);
        }
    }));
    let frames = create_memo(clone!(revision plugin_id -> move || {
        revision.get();
        plugin_host::frames(&plugin_id)
    }));
    let drawing = |floating: bool| {
        let shown = create_memo(clone!(state frames rotation opacity -> move || {
            let pieces = state.with(|view| match floating {
                true => view.floating.clone(),
                false => view.base.clone(),
            });
            (frames.get(), rotation.get(), opacity.get(), pieces)
        }));
        let drawn = create_memo(clone!(plugin_id state -> move || {
            let (_, rotation, opacity, pieces) = shown.get();
            state.with_untracked(|view| {
                plugin_host::region_drawing(
                    &plugin_id, instance, region, view, &pieces, rotation, opacity,
                )
            })
        }));
        Prop::Dynamic(Rc::new(move || draw_gpu(drawn.get())))
    };
    let base = drawing(false);
    let floating = drawing(true);
    let cursor = create_memo(clone!(state -> move || {
        state.with(|view| view.cursor.unwrap_or(CursorIcon::Default))
    }));
    let handles_back = create_memo(clone!(state -> move || state.with(|view| view.handles_back)));
    let loading = create_memo(clone!(state -> move || state.with(|view| view.loading)));
    let failure = create_memo(clone!(state plugin_id -> move || {
        state.with(|view| {
            view.error.as_ref().map(|(text, restart)| HostItem::Error {
                text: text.clone(),
                restart: restart.then(|| plugin_id.clone()),
            })
        })
    }));
    let rect = create_memo(clone!(state -> move || state.with(|view| view.rect)));
    let (reports, tick) = super::editors::reports();
    super::editors::provide_reports(reports.clone());
    create_effect(clone!(plugin_id -> move || {
        tick.get();
        plugin_host::report_children(&plugin_id, instance, region, super::editors::statuses(&reports));
    }));
    let forward = clone!(plugin_id -> move |input: ForwardedInput| {
        plugin_host::forward_region(&plugin_id, instance, region, &input);
    });
    let back = clone!(plugin_id -> move || {
        plugin_host::back_region(&plugin_id, instance, region, beui::BackGesture::Invoked);
    });
    let takes = clone!(state passive -> move |local: Pos2| {
        !passive.peek() && state.with_untracked(|view| view.takes(local))
    });
    let below = Rc::clone(&child_view);
    let above = child_view;
    view! {
        <Layers>
            <RegionChildren state={state.clone()} rect={rect.clone()} child_view={below} below=true />
            <BackHandler enabled={handles_back} on_back={back}>
                <Interactive
                    focusable=true
                    cursor={cursor}
                    on_forward={forward}
                    forward_at={takes}
                >
                    <Embed slot={slot} punch=false>
                        <Drawing draw={base} />
                    </Embed>
                </Interactive>
            </BackHandler>
            <Notices loading failure />
            <RegionChildren state rect child_view={above} below=false />
            <Drawing draw={floating} />
        </Layers>
    }
}

#[component]
fn Notices(loading: Memo<bool>, failure: Memo<Option<HostItem>>) -> NodeId {
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let still_loading = create_memo(clone!(failure loading -> move || {
        loading.get() && failure.get().is_none()
    }));
    let failure_item = create_memo(move || {
        failure.get().unwrap_or(HostItem::Error {
            text: String::new(),
            restart: None,
        })
    });
    let loading_item = create_memo(|| HostItem::Notice {
        text: "Loading plugin…".to_owned(),
        spinner: true,
    });
    view! {
        <Frame align_horizontal=Align::Center align_vertical=Align::Center>
            <List spacing=0.0>
                <Show condition={failed}>
                    <HostItemFace content={failure_item.clone()} />
                </Show>
                <Show condition={still_loading}>
                    <HostItemFace content={loading_item.clone()} />
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn RegionChildren(
    state: Memo<RegionView>,
    rect: Memo<Rect>,
    child_view: ChildView,
    below: bool,
) -> NodeId {
    let keys = create_memo(clone!(state -> move || {
        state.with(|view| {
            view.children
                .iter()
                .filter(|child| (child.is_below() && !child.frame_owner) == below)
                .map(|child| child.child)
                .collect::<Vec<ChildId>>()
        })
    }));
    view! {
        <Canvas>
            <ForEach keys={keys}>
                {move |key: ChildId| {
                    let child = create_memo(clone!(state -> move || {
                        state.with(|view| {
                            view.children.iter().find(|child| child.child == key).cloned()
                        })
                    }));
                    let clip = create_memo(clone!(child rect -> move || {
                        let origin = rect.get().min;
                        child
                            .get()
                            .map_or(Rect::ZERO, |child| child.clip.translate(-origin.to_vec2()))
                    }));
                    let x = create_memo(clone!(clip -> move || clip.get().min.x));
                    let y = create_memo(clone!(clip -> move || clip.get().min.y));
                    let width = create_memo(clone!(clip -> move || clip.get().width()));
                    let height = create_memo(clone!(clip -> move || clip.get().height()));
                    let built = child_view(key, child, rect.clone());
                    view! {
                        <CanvasItem x={x} y={y} width={width} height={height}>
                            {built}
                        </CanvasItem>
                    }
                }}
            </ForEach>
        </Canvas>
    }
}
