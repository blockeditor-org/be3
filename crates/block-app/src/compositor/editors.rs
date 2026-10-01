use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use be_graph::Access;
use beui::reactive::{
    Canvas, CanvasItem, Dynamic, ForEach, Frame, Interactive, Layers, Memo, Portal, Prop,
    ReadSignal, Show, WriteSignal, clone, component, component_rect, create_effect, create_memo,
    create_signal, on_cleanup, provide_context, use_context, view,
};
use beui::{NodeId, Pos2, Rect, ScrollGesture, Vec2, ZoomGesture, vec2};
use block_plugin_api::{
    BarAction, ChildId, ChildMode, ChildRect, EditorInstanceId, EditorRegion, FrameChrome,
    FrameSpec, PaneId, TopBar, ViewChange,
};
use uuid::Uuid;

use super::region::{ChildView, PluginRegion, RegionEditor};
use crate::editors::{BlockLabel, Chrome, EditorRegistry, PluginEditor, editor_access_ceiling};
use crate::host::HostItem;
use crate::plugin_host::{
    self, EditorView, HostChild, HostChildStatus, RegionPlacement, RegionSlot,
};
use crate::surfaces::HostItemFace;

const MIN_ZOOM: f32 = 1.0 / 64.0;
const MAX_ZOOM: f32 = 32.0;
const CHILD_UNAVAILABLE: &str = "the block is already open above this editor";

pub(crate) struct State {
    registry: RefCell<Rc<EditorRegistry>>,
    client_id: Cell<Uuid>,
    open: RefCell<HashMap<Uuid, PluginEditor>>,
    simulated: RefCell<HashMap<Uuid, Access>>,
}

#[derive(Clone)]
pub(crate) struct Editors(Rc<State>);

thread_local! {
    static EDITORS: RefCell<Option<Editors>> = const { RefCell::new(None) };
}

pub(crate) fn editors() -> Editors {
    EDITORS.with(|editors| {
        editors
            .borrow()
            .clone()
            .expect("the editors are installed before the document is built")
    })
}

impl Editors {
    pub(crate) fn install(registry: Rc<EditorRegistry>, client_id: Uuid) -> Self {
        let editors = Self(Rc::new(State {
            registry: RefCell::new(registry),
            client_id: Cell::new(client_id),
            open: RefCell::new(HashMap::new()),
            simulated: RefCell::new(HashMap::new()),
        }));
        EDITORS.with(|held| *held.borrow_mut() = Some(editors.clone()));
        editors
    }

    pub(crate) fn reset(&self, registry: Rc<EditorRegistry>) {
        *self.0.registry.borrow_mut() = registry;
        let closed: Vec<PluginEditor> =
            self.with(|open| open.drain().map(|(_, editor)| editor).collect());
        drop(closed);
        self.0.simulated.borrow_mut().clear();
        super::changed();
    }

    pub(crate) fn simulated(&self, id: Uuid) -> Option<Access> {
        self.0.simulated.borrow().get(&id).copied()
    }

    pub(crate) fn registry(&self) -> Rc<EditorRegistry> {
        Rc::clone(&self.0.registry.borrow())
    }

    pub(crate) fn client_id(&self) -> Uuid {
        self.0.client_id.get()
    }

    pub(crate) fn with<R>(&self, act: impl FnOnce(&mut HashMap<Uuid, PluginEditor>) -> R) -> R {
        act(&mut self.0.open.borrow_mut())
    }

    pub(crate) fn with_simulated<R>(&self, act: impl FnOnce(&mut HashMap<Uuid, Access>) -> R) -> R {
        let result = act(&mut self.0.simulated.borrow_mut());
        super::changed();
        result
    }

    fn access(&self, id: Uuid, above: Access) -> Access {
        let simulated = self
            .0
            .simulated
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or(Access::Edit);
        above.min(editor_access_ceiling(id)).min(simulated)
    }

    fn ensure(&self, id: Uuid, block_type: Uuid, view_block: Option<Uuid>) {
        let registry = self.registry();
        self.with(|open| {
            open.entry(id)
                .or_insert_with(|| registry.open(id, block_type).viewed_by(view_block));
        });
    }

    fn handle(&self, id: Uuid) -> Option<Handle> {
        self.with(|open| open.get(&id).map(Handle::of))
    }

    fn region(&self, handle: &Handle) -> Option<RegionEditor> {
        Some(RegionEditor {
            plugin: handle.plugin.clone()?,
            role: handle.role,
            instance: handle.instance,
            block_types: Rc::clone(&self.registry()).plugin_block_types().clone(),
            client_id: self.client_id(),
        })
    }
}

#[derive(Clone)]
struct Handle {
    block_type: Uuid,
    plugin: Option<std::sync::Arc<block_plugin_api::PluginManifest>>,
    instance: block_plugin_api::EditorInstanceId,
    role: plugin_host::InstanceRole,
    pan_and_zoom: bool,
    max_zoom: Option<u32>,
    preview: bool,
}

impl Handle {
    fn of(editor: &PluginEditor) -> Self {
        let capabilities = editor.direct_editor_capabilities();
        Self {
            block_type: editor.block_type(),
            plugin: editor.plugin().cloned(),
            instance: editor.instance(),
            role: editor.role(),
            pan_and_zoom: capabilities.supports_pan_and_zoom,
            max_zoom: capabilities.max_zoom,
            preview: editor.has_region(EditorRegion::Preview),
        }
    }

    fn plugin_id(&self) -> Option<&str> {
        self.plugin
            .as_ref()
            .map(|plugin| plugin.identity.id.as_str())
    }
}

#[derive(Clone)]
struct Nesting {
    above: Rc<Vec<Uuid>>,
    access: Access,
}

impl Nesting {
    fn root() -> Self {
        Self {
            above: Rc::new(Vec::new()),
            access: Access::Edit,
        }
    }

    fn within(&self, id: Uuid, access: Access) -> Self {
        let mut above = (*self.above).clone();
        above.push(id);
        Self {
            above: Rc::new(above),
            access,
        }
    }
}

#[derive(Clone)]
struct TabContext {
    stack: Memo<Vec<Uuid>>,
    top_bar: Prop<TopBar>,
    portal: WriteSignal<Option<NodeId>>,
    exit: Rc<dyn Fn()>,
}

#[derive(Clone, Default)]
pub(crate) struct ChildReporter {
    view: Option<Rc<dyn Fn(ViewChange)>>,
    bar: Option<Rc<dyn Fn(BarAction)>>,
}

#[derive(Clone)]
pub(crate) struct ChildReports {
    statuses: Rc<RefCell<BTreeMap<u64, HostChildStatus>>>,
    tick: WriteSignal<u64>,
}

#[component]
pub(crate) fn ShellSurface(shell: Memo<Option<Uuid>>) -> NodeId {
    let keys = create_memo(move || shell.get().into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |block: Uuid| {
                    provide_context(Nesting::root());
                    view! {
                        <TabFrame block top_bar=TopBar::Hidden />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[component]
pub(crate) fn PaneSurface(shell: Memo<Option<Uuid>>, pane: PaneId) -> NodeId {
    let keys = create_memo(move || shell.get().into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |block: Uuid| {
                    provide_context(Nesting::root());
                    view! {
                        <BlockRegion block region=EditorRegion::Pane(pane) />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[component]
pub(crate) fn PresentingSurface() -> NodeId {
    let editors = editors();
    let any = super::any();
    let presenting = create_memo(clone!(editors -> move || {
        any.get();
        editors.with(|open| {
            open.values()
                .find(|editor| editor.presenting_now())
                .map(PluginEditor::id)
        })
    }));
    let keys = create_memo(move || presenting.get().into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |block: Uuid| {
                    provide_context(Nesting::root());
                    crate::host::set_fullscreen(true);
                    on_cleanup(|| crate::host::set_fullscreen(false));
                    view! {
                        <BlockFrame
                            block
                            chrome=Chrome::Drawn
                            content=None
                            top_bar=TopBar::Hidden
                            embedded=false
                            passive=false
                            presented=true
                        />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[component]
fn BlockRegion(block: Uuid, region: EditorRegion) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    provide_context(nesting.within(block, nesting.access));
    let any = super::any();
    let keys = create_memo(clone!(editors -> move || {
        any.get();
        region_key(&editors, block, false).into_iter().collect::<Vec<_>>()
    }));
    let child_view = child_view(block, region);
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |_key: RegionKey| {
                    let Some(region_editor) = editors.handle(block).and_then(|handle| editors.region(&handle)) else {
                        return view! {
                            <Frame />
                        };
                    };
                    view! {
                        <PluginRegion
                            editor={region_editor}
                            region
                            frame={Some(FrameSpec::default())}
                            child_view={Rc::clone(&child_view)}
                        />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[component]
fn TabFrame(block: Uuid, top_bar: Prop<TopBar>) -> NodeId {
    let editors = editors();
    let any = super::any();
    let stack = create_memo(clone!(editors any -> move || {
        any.get();
        let mut stack = Vec::new();
        let mut next = frame_child(&editors, block);
        while let Some(id) = next {
            if id == block || stack.contains(&id) {
                break;
            }
            stack.push(id);
            next = frame_child(&editors, id);
        }
        stack
    }));
    let (portal, set_portal) = create_signal(None::<NodeId>);
    let exit: Rc<dyn Fn()> = Rc::new(clone!(editors stack -> move || {
        let stack = stack.get_untracked();
        let parent = match stack.len() {
            0 | 1 => block,
            depth => stack[depth - 2],
        };
        revoke_frame_child(&editors, parent);
        crate::host::request_repaint();
    }));
    provide_context(TabContext {
        stack: stack.clone(),
        top_bar: top_bar.clone(),
        portal: set_portal,
        exit,
    });
    let chrome = create_memo(clone!(stack -> move || match stack.get().is_empty() {
        true => Chrome::Drawn,
        false => Chrome::None,
    }));
    view! {
        <Layers>
            <BlockFrame
                block
                chrome={chrome}
                content=None
                top_bar={top_bar}
                embedded=false
                passive=false
            />
            <Portal node={portal} />
        </Layers>
    }
}

fn frame_child(editors: &Editors, block: Uuid) -> Option<Uuid> {
    let handle = editors.handle(block)?;
    plugin_host::frame_child(handle.plugin_id()?, handle.instance)
}

fn chrome_owner(editors: &Editors, block: Uuid) -> Uuid {
    let mut stack = Vec::new();
    let mut next = frame_child(editors, block);
    while let Some(id) = next {
        if id == block || stack.contains(&id) {
            break;
        }
        stack.push(id);
        next = frame_child(editors, id);
    }
    stack.last().copied().unwrap_or(block)
}

fn frame_menu(editors: &Editors, block: Uuid) -> Vec<block_plugin_api::MenuEntry> {
    let owner = chrome_owner(editors, block);
    editors
        .with(|open| open.get(&owner).map(PluginEditor::menu))
        .unwrap_or_default()
}

pub(super) fn pick_frame_menu(block: Uuid, pick: String) {
    let editors = editors();
    let owner = chrome_owner(&editors, block);
    editors.with(|open| {
        if let Some(editor) = open.get(&owner) {
            editor.pick_menu(pick);
        }
    });
}

fn revoke_frame_child(editors: &Editors, block: Uuid) {
    if let Some(handle) = editors.handle(block)
        && let Some(plugin_id) = handle.plugin_id()
    {
        plugin_host::revoke_frame_child(plugin_id, handle.instance);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Viewport {
    zoom: f32,
    pan: Vec2,
    center: Option<Pos2>,
    auto_fit: Option<bool>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            center: None,
            auto_fit: None,
        }
    }
}

thread_local! {
    static VIEWPORTS: RefCell<HashMap<Uuid, Viewport>> = RefCell::new(HashMap::new());
}

fn fit(viewport: &mut Viewport, size: Vec2, content: Vec2) {
    viewport.zoom = (size.x / content.x.max(1.0))
        .min(size.y / content.y.max(1.0))
        .min(1.0)
        .clamp(MIN_ZOOM, MAX_ZOOM);
    viewport.pan = Vec2::ZERO;
}

#[derive(Clone, Copy)]
enum Command {
    Pan(Vec2),
    Zoom { factor: f32, anchor: Option<Pos2> },
    Fit,
    ResumeAutoFit,
}

fn settle(viewport: &mut Viewport, command: Command, rect: Rect, content: Vec2, max_zoom: f32) {
    let size = rect.size().max(Vec2::splat(1.0));
    match command {
        Command::Pan(delta) => {
            viewport.pan += delta;
            viewport.auto_fit = viewport.auto_fit.map(|_| false);
        }
        Command::Zoom { factor, anchor } => {
            let old = viewport.zoom;
            let new = (old * factor).clamp(MIN_ZOOM, max_zoom);
            if new != old {
                let anchor = anchor.unwrap_or_else(|| rect.center());
                let offset = anchor - rect.center();
                viewport.pan = offset - (offset - viewport.pan) * (new / old);
                viewport.zoom = new;
            }
            viewport.auto_fit = viewport.auto_fit.map(|_| false);
        }
        Command::Fit => {
            fit(viewport, size, content);
            viewport.auto_fit = viewport.auto_fit.map(|_| false);
        }
        Command::ResumeAutoFit => {
            if viewport.auto_fit.is_some() {
                viewport.auto_fit = Some(true);
                fit(viewport, size, content);
            }
        }
    }
}

#[component]
fn BlockFrame(
    block: Uuid,
    chrome: Prop<Chrome>,
    content: Prop<Option<Rect>>,
    top_bar: Prop<TopBar>,
    embedded: bool,
    passive: Prop<bool>,
    #[prop(default = false)] presented: bool,
    #[prop(default = ChildReporter::default())] reporter: ChildReporter,
) -> NodeId {
    let any = super::any();
    let shown = create_memo(move || {
        any.get();
        presented
            || editors()
                .with(|open| open.get(&block).map(|editor| !editor.presenting_now()))
                .unwrap_or(true)
    });
    let keys = create_memo(move || shown.get().then_some(block).into_iter().collect::<Vec<_>>());
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |block: Uuid| {
                    view! {
                        <BlockFrameBody
                            block
                            chrome={chrome.clone()}
                            content={content.clone()}
                            top_bar={top_bar.clone()}
                            embedded
                            passive={passive.clone()}
                            reporter={reporter.clone()}
                        />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RegionKey {
    plugin: String,
    instance: EditorInstanceId,
}

fn region_key(editors: &Editors, block: Uuid, preview: bool) -> Option<RegionKey> {
    let handle = editors.handle(block)?;
    if preview && !handle.preview {
        return None;
    }
    Some(RegionKey {
        plugin: handle.plugin_id()?.to_owned(),
        instance: handle.instance,
    })
}

#[derive(Clone, PartialEq)]
enum Resolution {
    NoAccess,
    Missing,
    Unsupported(Uuid),
    Ready { region: RegionKey, access: Access },
}

#[component]
fn Notice(item: HostItem) -> NodeId {
    let content = create_memo(move || item.clone());
    view! {
        <Frame align_horizontal=beui::Align::Center align_vertical=beui::Align::Center>
            <HostItemFace content />
        </Frame>
    }
}

#[component]
fn BlockFrameBody(
    block: Uuid,
    chrome: Prop<Chrome>,
    content: Prop<Option<Rect>>,
    top_bar: Prop<TopBar>,
    embedded: bool,
    passive: Prop<bool>,
    reporter: ChildReporter,
) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    let any = super::any();
    let resolution = create_memo(clone!(editors nesting -> move || {
        any.get();
        let access = editors.access(block, nesting.access);
        if !access.can_view() {
            return Resolution::NoAccess;
        }
        let Some(handle) = editors.handle(block) else {
            return Resolution::Missing;
        };
        match region_key(&editors, block, false) {
            Some(region) => Resolution::Ready { region, access },
            None => Resolution::Unsupported(handle.block_type),
        }
    }));
    view! {
        <Layers>
            <Dynamic value={resolution}>
                {move |resolution: Resolution| {
                    match resolution {
                        Resolution::NoAccess => {
                            let item = HostItem::Notice {
                                text: "No access".to_owned(),
                                spinner: false,
                            };
                            view! {
                                <Notice item />
                            }
                        }
                        Resolution::Missing => view! {
                            <Frame />
                        },
                        Resolution::Unsupported(block_type) => {
                            let item = HostItem::Unsupported { block, block_type };
                            view! {
                                <Notice item />
                            }
                        }
                        Resolution::Ready { access, .. } => {
                            let editors = self::editors();
                            let Some(handle) = editors.handle(block) else {
                                return view! {
                                    <Frame />
                                };
                            };
                            let Some(region_editor) = editors.region(&handle) else {
                                return view! {
                                    <Frame />
                                };
                            };
                            view! {
                                <BlockFrameView
                                    block
                                    access
                                    handle
                                    region_editor
                                    chrome={chrome.clone()}
                                    content={content.clone()}
                                    top_bar={top_bar.clone()}
                                    embedded
                                    passive={passive.clone()}
                                    reporter={reporter.clone()}
                                />
                            }
                        }
                    }
                }}
            </Dynamic>
        </Layers>
    }
}

#[component]
fn BlockFrameView(
    block: Uuid,
    access: Access,
    handle: Handle,
    region_editor: RegionEditor,
    chrome: Prop<Chrome>,
    content: Prop<Option<Rect>>,
    top_bar: Prop<TopBar>,
    embedded: bool,
    passive: Prop<bool>,
    reporter: ChildReporter,
) -> NodeId {
    let ChildReporter {
        view: on_view_change,
        bar: on_bar_action,
    } = reporter;
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    provide_context(nesting.within(block, access));
    let plugin_id = region_editor.plugin_id().to_owned();
    let instance = region_editor.instance;
    editors.with(|open| {
        if let Some(editor) = open.get_mut(&block) {
            editor.shown(true);
        }
    });
    on_cleanup(clone!(editors -> move || {
        editors.with(|open| {
            if let Some(editor) = open.get_mut(&block) {
                editor.shown(false);
            }
        });
    }));
    let framed = create_memo(clone!(content -> move || content.get().is_some()));
    create_effect(clone!(plugin_id framed -> move || {
        let _ = framed.get();
        plugin_host::hold(&plugin_id, instance, EditorRegion::Frame);
    }));
    let revision = super::listen(&plugin_id);
    let rect = component_rect();
    let read_only = !access.can_edit();
    let placed = embedded || content.peek().is_some();
    let pan_and_zoom = handle.pan_and_zoom && !placed;
    let max_zoom = handle.max_zoom.map_or(MAX_ZOOM, |zoom| zoom as f32);
    let (viewport, set_viewport) = create_signal(
        VIEWPORTS
            .with(|viewports| viewports.borrow().get(&block).copied())
            .unwrap_or_default(),
    );
    create_effect(clone!(viewport -> move || {
        let state = viewport.get();
        VIEWPORTS.with(|viewports| viewports.borrow_mut().insert(block, state));
    }));
    let viewport_rect = create_memo(clone!(plugin_id revision rect -> move || {
        revision.get();
        let rect = rect.get();
        plugin_host::frame_rects(&plugin_id, instance)
            .map(|rects| rects.content.translate(rect.min.to_vec2()))
            .filter(Rect::is_positive)
            .unwrap_or(rect)
    }));
    let content_size = create_memo(clone!(plugin_id revision viewport_rect -> move || {
        revision.get();
        let size = viewport_rect.get().size().max(Vec2::splat(1.0));
        let intrinsic = plugin_host::intrinsic_size(&plugin_id, instance)
            .unwrap_or(vec2(420.0, 240.0));
        vec2(size.x.max(intrinsic.x), size.y.max(intrinsic.y))
    }));
    let command: Rc<dyn Fn(Command)> = Rc::new(
        clone!(viewport set_viewport viewport_rect content_size on_view_change -> move |command: Command| {
            if !pan_and_zoom {
                if let Some(report) = &on_view_change {
                    report(match command {
                        Command::Pan(delta) => ViewChange::Pan { x: delta.x, y: delta.y },
                        Command::Zoom { factor, anchor } => ViewChange::Zoom {
                            factor,
                            anchor: anchor.map(|anchor| (anchor.x, anchor.y)),
                        },
                        Command::Fit => ViewChange::Fit,
                        Command::ResumeAutoFit => ViewChange::ResumeAutoFit,
                    });
                }
                return;
            }
            let mut state = viewport.get_untracked();
            settle(&mut state, command, viewport_rect.get_untracked(), content_size.get_untracked(), max_zoom);
            if state != viewport.get_untracked() {
                set_viewport.set(state);
            }
        }),
    );
    create_effect(
        clone!(viewport set_viewport viewport_rect content_size -> move || {
            if !pan_and_zoom {
                return;
            }
            let rect = viewport_rect.get();
            let content = content_size.get();
            let mut state = viewport.get_untracked();
            if let Some(previous) = state.center.replace(rect.center()) {
                state.pan += previous - rect.center();
            }
            if state.auto_fit.is_none() {
                state.auto_fit = Some(true);
            }
            if state.auto_fit == Some(true) {
                fit(&mut state, rect.size().max(Vec2::splat(1.0)), content);
            }
            if state != viewport.get_untracked() {
                set_viewport.set(state);
            }
        }),
    );
    create_effect(
        clone!(plugin_id revision command on_bar_action rect -> move || {
            revision.get();
            let rect = rect.get_untracked();
            for change in plugin_host::take_view_changes(&plugin_id, instance) {
                command(match change {
                    ViewChange::Pan { x, y } => Command::Pan(vec2(x, y)),
                    ViewChange::Zoom { factor, anchor } => Command::Zoom {
                        factor,
                        anchor: anchor.map(|(x, y)| rect.min + vec2(x, y)),
                    },
                    ViewChange::Fit => Command::Fit,
                    ViewChange::ResumeAutoFit => Command::ResumeAutoFit,
                });
            }
            let actions = plugin_host::take_bar_actions(&plugin_id, instance);
            if let Some(report) = &on_bar_action {
                for action in actions {
                    report(action);
                }
            }
        }),
    );
    if let Some(tab) = use_context::<TabContext>() {
        create_effect(clone!(plugin_id revision -> move || {
            revision.get();
            if plugin_host::take_leaving(&plugin_id, instance) {
                (tab.exit)();
            }
        }));
    }
    let content_rect = create_memo(clone!(viewport viewport_rect content_size -> move || {
        let state = viewport.get();
        Rect::from_center_size(
            viewport_rect.get().center() + state.pan,
            content_size.get() * state.zoom,
        )
    }));
    let editor_view = create_memo(
        clone!(plugin_id revision rect content content_rect viewport -> move || {
            if !handle.pan_and_zoom {
                return None;
            }
            let origin = rect.get().min;
            if !placed {
                return Some(EditorView {
                    rect: content_rect.get().translate(-origin.to_vec2()),
                    scale: viewport.get().zoom,
                });
            }
            revision.get();
            let shown = content.get().unwrap_or(rect.get());
            let scale = plugin_host::intrinsic_size(&plugin_id, instance)
                .filter(|intrinsic| intrinsic.x > 0.0 && intrinsic.y > 0.0)
                .map_or(1.0, |intrinsic| {
                    (shown.width() / intrinsic.x).min(shown.height() / intrinsic.y)
                });
            Some(EditorView {
                rect: shown.translate(-origin.to_vec2()),
                scale,
            })
        }),
    );
    let frame = create_memo(clone!(chrome content top_bar rect -> move || {
        let origin = rect.get().min;
        Some(FrameSpec {
            chrome: match chrome.get() {
                Chrome::Drawn => FrameChrome::Drawn,
                Chrome::None => FrameChrome::None,
            },
            content: content.get().map(|content| {
                let content = content.translate(-origin.to_vec2());
                ChildRect {
                    x: content.min.x,
                    y: content.min.y,
                    width: content.width(),
                    height: content.height(),
                }
            }),
            top_bar: top_bar.get(),
        })
    }));
    let gestures = pan_and_zoom || on_view_change.is_some() && handle.pan_and_zoom;
    let steered = create_memo(clone!(content_rect -> move || {
        (!read_only && !handle.pan_and_zoom).then(|| content_rect.get())
    }));
    let scroll = clone!(command steered -> move |gesture: ScrollGesture| {
        if !gestures || steered.get_untracked().is_some_and(|rect| rect.contains(gesture.pos)) {
            return;
        }
        match gesture.modifiers.ctrl && gesture.delta.y != 0.0 {
            true => command(Command::Zoom {
                factor: (gesture.delta.y * 0.002).exp(),
                anchor: Some(gesture.pos),
            }),
            false => command(Command::Pan(gesture.delta)),
        }
    });
    let zoom = clone!(command -> move |gesture: ZoomGesture| {
        if gestures {
            command(Command::Zoom {
                factor: gesture.factor,
                anchor: Some(gesture.pos),
            });
        }
    });
    let pan = clone!(command -> move |delta: Vec2| {
        if gestures {
            command(Command::Pan(delta));
        }
    });
    let child_view = child_view(block, EditorRegion::Frame);
    view! {
        <Interactive on_scroll={scroll} on_zoom={zoom} on_pan_drag={pan}>
            <PluginRegion
                editor={region_editor}
                region=EditorRegion::Frame
                frame={frame}
                view={editor_view}
                passive={passive}
                child_view
            />
        </Interactive>
    }
}

fn child_view(parent: Uuid, region: EditorRegion) -> ChildView {
    Rc::new(
        move |key: ChildId, child: Memo<Option<HostChild>>, rect: Memo<Rect>| {
            view! {
                <HostedChild parent region key child rect />
            }
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Missing,
    Unavailable,
    Preview,
    OwnFrame,
    FrameChild,
    Embedded,
}

#[component]
fn HostedChild(
    parent: Uuid,
    region: EditorRegion,
    key: ChildId,
    child: Memo<Option<HostChild>>,
    rect: Memo<Rect>,
) -> NodeId {
    let editors = editors();
    let nesting = use_context::<Nesting>().unwrap_or_else(Nesting::root);
    let tab = use_context::<TabContext>();
    let any = super::any();
    let block = create_memo(clone!(child -> move || {
        child.get().map(|child| (child.block_id, child.block_type, child.view_block))
    }));
    let kind = create_memo(clone!(editors child block any tab nesting -> move || {
        any.get();
        let Some((id, block_type, view_block)) = block.get() else {
            return Kind::Missing;
        };
        if nesting.above.contains(&id) {
            return Kind::Unavailable;
        }
        editors.ensure(id, block_type, view_block);
        let Some(child) = child.get() else {
            return Kind::Missing;
        };
        let in_stack = tab
            .as_ref()
            .is_some_and(|tab| tab.stack.get().contains(&id));
        match child.mode {
            ChildMode::Preview => Kind::Preview,
            _ if child.frame_owner && child.own_frame => Kind::OwnFrame,
            _ if child.frame_owner && in_stack => Kind::FrameChild,
            _ => Kind::Embedded,
        }
    }));
    create_effect(clone!(editors child -> move || {
        let Some(child) = child.get() else {
            return;
        };
        if let (Some(size), Some(handle)) = (child.intrinsic, editors.handle(child.block_id))
            && let Some(plugin_id) = handle.plugin_id()
        {
            plugin_host::resized(plugin_id, handle.instance, size);
        }
    }));
    if let Some(reports) = use_context::<ChildReports>() {
        let status = create_memo(clone!(editors child kind any -> move || {
            any.get();
            let child = child.get()?;
            let available = !matches!(kind.get(), Kind::Unavailable | Kind::Missing);
            Some(status_of(&editors, &child, available))
        }));
        create_effect(clone!(reports status -> move || {
            let status = status.get();
            let mut statuses = reports.statuses.borrow_mut();
            match status {
                Some(status) => statuses.insert(key.0, status),
                None => statuses.remove(&key.0),
            };
            drop(statuses);
            reports.tick.update(|tick| *tick += 1);
        }));
        on_cleanup(move || {
            reports.statuses.borrow_mut().remove(&key.0);
            reports.tick.update(|tick| *tick += 1);
        });
    }
    let local = create_memo(clone!(child -> move || {
        child
            .get()
            .map_or(Rect::ZERO, |child| child.rect.translate(-child.clip.min.to_vec2()))
    }));
    let x = create_memo(clone!(local -> move || local.get().min.x));
    let y = create_memo(clone!(local -> move || local.get().min.y));
    let width = create_memo(clone!(local -> move || local.get().width()));
    let height = create_memo(clone!(local -> move || local.get().height()));
    let keys = create_memo(clone!(kind block -> move || {
        block.get().map(|(id, _, _)| (id, kind.get())).into_iter().collect::<Vec<_>>()
    }));
    let _ = rect;
    view! {
        <Canvas>
            <CanvasItem x={x} y={y} width={width} height={height}>
                <Layers>
                    <ForEach keys={keys}>
                        {move |(id, kind): (Uuid, Kind)| {
                            let reporter = reporter_for(parent, region, key);
                            let child = child.clone();
                            match kind {
                                Kind::Missing | Kind::Unavailable => view! {
                                    <Frame />
                                },
                                Kind::Preview => {
                                    let rotation = create_memo(clone!(child -> move || {
                                        child.get().map_or(0.0, |child| child.rotation)
                                    }));
                                    let opacity = create_memo(clone!(child -> move || {
                                        child.get().map_or(1.0, |child| child.opacity)
                                    }));
                                    view! {
                                        <BlockPreview
                                            block=id
                                            rotation={rotation}
                                            opacity={opacity}
                                        />
                                    }
                                }
                                Kind::OwnFrame => {
                                    let top_bar = create_memo(clone!(child -> move || {
                                        child.get().map_or(TopBar::Hidden, |child| child.top_bar)
                                    }));
                                    view! {
                                        <TabFrame block=id top_bar={top_bar} />
                                    }
                                }
                                Kind::FrameChild => {
                                    let Some(tab) = use_context::<TabContext>() else {
                                        return view! {
                                            <Frame />
                                        };
                                    };
                                    let content = create_memo(clone!(child -> move || {
                                        child.get().map(|child| child.rect.intersect(child.clip))
                                    }));
                                    let chrome = create_memo(clone!(tab -> move || {
                                        match tab.stack.get().last() == Some(&id) {
                                            true => Chrome::Drawn,
                                            false => Chrome::None,
                                        }
                                    }));
                                    let top_bar = tab.top_bar.clone();
                                    let node = view! {
                                        <BlockFrame
                                            block=id
                                            chrome={chrome}
                                            content={content}
                                            top_bar={top_bar}
                                            embedded=false
                                            passive=false
                                            reporter
                                        />
                                    };
                                    tab.portal.set(Some(node));
                                    on_cleanup(move || tab.portal.set(None));
                                    view! {
                                        <Frame />
                                    }
                                }
                                Kind::Embedded => {
                                    let passive = create_memo(clone!(child -> move || {
                                        child.get().is_none_or(|child| child.mode == ChildMode::Passive)
                                    }));
                                    view! {
                                        <BlockFrame
                                            block=id
                                            chrome=Chrome::None
                                            content=None
                                            top_bar=TopBar::Hidden
                                            embedded=true
                                            passive={passive}
                                            reporter
                                        />
                                    }
                                }
                            }
                        }}
                    </ForEach>
                </Layers>
            </CanvasItem>
        </Canvas>
    }
}

fn status_of(editors: &Editors, child: &HostChild, available: bool) -> HostChildStatus {
    let handle = editors.handle(child.block_id);
    let (interaction, capabilities, resize, intrinsic, aspect_ratio) =
        editors.with(|open| match open.get_mut(&child.block_id) {
            Some(editor) if available => (
                match editor.direct_editor_interaction() {
                    crate::editors::DirectEditorInteraction::Live => {
                        block_plugin_api::InteractionMode::Live
                    }
                    crate::editors::DirectEditorInteraction::Playback => {
                        block_plugin_api::InteractionMode::Playback
                    }
                    crate::editors::DirectEditorInteraction::Preview => {
                        block_plugin_api::InteractionMode::Preview
                    }
                },
                {
                    let capabilities = editor.direct_editor_capabilities();
                    block_plugin_api::EditorCapabilities {
                        rotation: capabilities.allow_rotation,
                        preserve_aspect_ratio: capabilities.preserve_aspect_ratio,
                        pan_and_zoom: capabilities.supports_pan_and_zoom,
                        max_zoom: capabilities.max_zoom,
                    }
                },
                match editor.direct_editor_resize() {
                    crate::editors::DirectEditorResize::Horizontal => {
                        block_plugin_api::ResizeMode::Horizontal
                    }
                    crate::editors::DirectEditorResize::Vertical => {
                        block_plugin_api::ResizeMode::Vertical
                    }
                    crate::editors::DirectEditorResize::Both => block_plugin_api::ResizeMode::Both,
                    crate::editors::DirectEditorResize::None => block_plugin_api::ResizeMode::None,
                },
                editor.direct_editor_intrinsic_size(),
                editor.render_aspect_ratio(),
            ),
            _ => (
                block_plugin_api::InteractionMode::Preview,
                block_plugin_api::EditorCapabilities::default(),
                block_plugin_api::ResizeMode::None,
                None,
                None,
            ),
        });
    let _ = handle;
    HostChildStatus {
        child: child.child,
        available,
        intrinsic,
        aspect_ratio,
        hovered: crate::host::pointer()
            .is_some_and(|position| child.rect.contains(position) && child.clip.contains(position)),
        active: available && child.is_active(),
        interaction,
        capabilities,
        resize,
        error: (!available).then(|| CHILD_UNAVAILABLE.to_owned()),
        menu: match available && child.frame_owner {
            true => frame_menu(editors, child.block_id),
            false => Vec::new(),
        },
    }
}

fn reporter_for(parent: Uuid, region: EditorRegion, key: ChildId) -> ChildReporter {
    let report_to = editors().handle(parent);
    let on_view_change: Option<Rc<dyn Fn(ViewChange)>> = report_to.clone().and_then(|parent| {
        let plugin_id = parent.plugin_id()?.to_owned();
        Some(Rc::new(move |change: ViewChange| {
            plugin_host::report_child_views(
                &plugin_id,
                parent.instance,
                region,
                vec![(key, change)],
            );
        }) as Rc<dyn Fn(ViewChange)>)
    });
    let on_bar_action: Option<Rc<dyn Fn(BarAction)>> = report_to.and_then(|parent| {
        let plugin_id = parent.plugin_id()?.to_owned();
        Some(Rc::new(move |action: BarAction| {
            plugin_host::report_child_bars(
                &plugin_id,
                parent.instance,
                region,
                vec![(key, action)],
            );
        }) as Rc<dyn Fn(BarAction)>)
    });
    ChildReporter {
        view: on_view_change,
        bar: on_bar_action,
    }
}

#[component]
fn BlockPreview(block: Uuid, rotation: Prop<f32>, opacity: Prop<f32>) -> NodeId {
    let editors = editors();
    let any = super::any();
    let keys = create_memo(clone!(editors any -> move || {
        any.get();
        region_key(&editors, block, true).into_iter().collect::<Vec<_>>()
    }));
    let fallback = create_memo(clone!(keys -> move || keys.with(Vec::is_empty)));
    let label = create_memo(clone!(editors -> move || {
        any.get();
        let label = crate::be::node(block).map(|node| BlockLabel::for_node(&editors.registry(), &node));
        HostItem::Fallback {
            name: label
                .as_ref()
                .map_or_else(|| "Loading…".to_owned(), |label| label.name.clone()),
            automatic: label.as_ref().is_some_and(|label| label.automatic),
            icon: label.and_then(|label| label.icon).map(str::to_owned),
        }
    }));
    let child_view = child_view(block, EditorRegion::Preview);
    view! {
        <Layers>
            <Show condition={fallback}>
                <HostItemFace content={label.clone()} />
            </Show>
            <ForEach keys={keys}>
                {move |_key: RegionKey| {
                    let Some(region_editor) = editors.handle(block).and_then(|handle| editors.region(&handle)) else {
                        return view! {
                            <Frame />
                        };
                    };
                    view! {
                        <PluginRegion
                            editor={region_editor}
                            region=EditorRegion::Preview
                            passive=true
                            rotation={rotation.clone()}
                            opacity={opacity.clone()}
                            child_view={Rc::clone(&child_view)}
                        />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

pub(super) fn reports() -> (ChildReports, ReadSignal<u64>) {
    let (tick, set_tick) = create_signal(0u64);
    (
        ChildReports {
            statuses: Rc::new(RefCell::new(BTreeMap::new())),
            tick: set_tick,
        },
        tick,
    )
}

pub(super) fn provide_reports(reports: ChildReports) {
    provide_context(reports);
}

pub(super) fn statuses(reports: &ChildReports) -> Vec<HostChildStatus> {
    reports.statuses.borrow().values().cloned().collect()
}

#[component]
pub(crate) fn HeadlessShell(shell: Memo<Option<Uuid>>, rect: Memo<Rect>) -> NodeId {
    let editors = editors();
    let any = super::any();
    let keys = create_memo(clone!(editors -> move || {
        any.get();
        shell
            .get()
            .and_then(|block| Some((block, region_key(&editors, block, false)?)))
            .into_iter()
            .collect::<Vec<_>>()
    }));
    view! {
        <Layers>
            <ForEach keys={keys}>
                {move |(block, _): (Uuid, RegionKey)| {
                    let Some(editor) = editors.handle(block).and_then(|handle| editors.region(&handle)) else {
                        return view! {
                            <Frame />
                        };
                    };
                    view! {
                        <HeadlessRegion editor rect={rect.clone()} />
                    }
                }}
            </ForEach>
        </Layers>
    }
}

#[component]
fn HeadlessRegion(editor: RegionEditor, rect: Memo<Rect>) -> NodeId {
    let instance = editor.instance;
    let plugin_id = editor.plugin_id().to_owned();
    plugin_host::mount_region(RegionSlot {
        plugin: &editor.plugin,
        block_types: &editor.block_types,
        client_id: editor.client_id,
        role: editor.role,
        instance,
        region: EditorRegion::Frame,
    });
    on_cleanup(clone!(plugin_id -> move || {
        plugin_host::unmount_region(&plugin_id, instance, EditorRegion::Frame)
    }));
    create_effect(move || {
        let rect = rect.get();
        plugin_host::place_region(
            &plugin_id,
            instance,
            EditorRegion::Frame,
            RegionPlacement { rect, clip: rect },
            Some(FrameSpec::default()),
            None,
        );
    });
    view! {
        <Frame />
    }
}
