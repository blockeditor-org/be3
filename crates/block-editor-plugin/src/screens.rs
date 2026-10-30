use block_plugin_api::{
    AnchorSpace, AnchoredPlacement, Catalog, DebugTemplate, EditorInstanceId, FrameCommit,
    HostTransform, HostView, MAX_COLLECTION_ITEMS, Opening, Pos2, ScreenId, ScreenLayout,
    ScreenPlacement, ScreenRequest, Size, SurfaceSpec, TemplateCategory, Vec2, ViewId,
    ViewPlacement, ViewRef, ViewportMetrics, WindowRequest,
};
use block_ui::{BlockCatalog, BlockTypeEntry, EditorEntry, TemplateEntry};
use lucide::Glyph;
use stable_hash::{HashMap, HashSet};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::{
    Frame, Module, ShownView, Waker,
    editor_session::{EditorSession, Open},
    plugin::Anchored,
    wire::Delivery,
};

pub(crate) type Opener = Open;

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EditorSession>,
    module: Module,
    apps: Vec<(Uuid, Opener)>,
    waker: Waker,
    requests: Vec<ScreenRequest>,
    opened: Vec<ScreenRequest>,
    anchored: Vec<AnchoredScreen>,
    next_opened: u64,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
    catalog: Catalog,
    debug_templates: Vec<DebugTemplate>,
    surface: Option<SurfaceSpec>,
    dirty: Arc<Mutex<HashSet<EditorInstanceId>>>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    everything: bool,
    reporting: HashSet<EditorInstanceId>,
    reporting_everything: bool,
    laid_out: HashSet<EditorInstanceId>,
    serial: u64,
}

#[cfg(target_arch = "wasm32")]
pub(crate) enum Dirty {
    Everything,
    Instances(HashSet<EditorInstanceId>),
}

#[cfg(target_arch = "wasm32")]
impl Dirty {
    pub(crate) fn contains(&self, instance: EditorInstanceId) -> bool {
        match self {
            Self::Everything => true,
            Self::Instances(instances) => instances.contains(&instance),
        }
    }
}

impl Screens {
    pub(crate) fn new(apps: Vec<(Uuid, Opener)>, waker: Waker) -> Self {
        Self {
            sessions: HashMap::default(),
            module: Module::default(),
            apps,
            waker,
            requests: Vec::new(),
            opened: Vec::new(),
            anchored: Vec::new(),
            next_opened: 0,
            layout: ScreenLayout::default(),
            block_types: Rc::new(BlockCatalog::default()),
            catalog: Catalog::default(),
            debug_templates: Vec::new(),
            surface: None,
            dirty: Arc::default(),
            everything: true,
            reporting: HashSet::default(),
            reporting_everything: true,
            laid_out: HashSet::default(),
            serial: 0,
        }
    }

    pub(crate) fn adopt(&mut self, instance: EditorInstanceId, mut session: EditorSession) {
        session.join(&self.module);
        self.sessions.insert(instance, session);
        self.touch_everything();
    }

    fn touch_everything(&mut self) {
        self.everything = true;
        self.reporting_everything = true;
    }

    pub(crate) fn ran(&mut self, instance: EditorInstanceId) {
        self.reporting.insert(instance);
        self.laid_out.insert(instance);
    }

    pub(crate) fn frame(
        &mut self,
        shown: &[(ScreenPlacement, u32)],
    ) -> Vec<(ScreenPlacement, Frame)> {
        let mut begun = Vec::new();
        for (placement, age) in shown {
            if self.is_anchored(placement.screen) {
                continue;
            }
            if let Some(session) = self.sessions.get_mut(&placement.instance) {
                begun.push((*placement, session.begin(placement.view, *age)));
            }
        }
        let regions: Vec<ShownView> = begun.iter().map(|(_, region)| region.clone()).collect();
        let mut frames = self
            .module
            .with_handler(|handler| handler.frame(&regions))
            .unwrap_or_default();
        frames.resize_with(regions.len(), Frame::default);
        let mut framed = Vec::new();
        let mut opened = Vec::new();
        let mut anchored = Vec::new();
        for ((placement, region), mut frame) in begun.into_iter().zip(frames) {
            self.open_windows(&region, &mut frame.asks, &mut opened);
            let hung = std::mem::take(&mut frame.anchored);
            self.anchor(&region, hung, &mut anchored);
            if let Some(session) = self.sessions.get_mut(&region.instance) {
                framed.push((placement, session.finish(region.view.id, frame)));
                self.ran(region.instance);
            }
        }
        let ages: HashMap<ScreenId, u32> = shown
            .iter()
            .map(|(placement, age)| (placement.screen, *age))
            .collect();
        framed.extend(self.frame_created(opened, anchored, &ages));
        framed
    }

    fn open_windows(
        &mut self,
        region: &ShownView,
        asks: &mut [WindowRequest],
        opened: &mut Vec<ScreenRequest>,
    ) {
        let instance = region.instance;
        for ask in asks {
            match ask {
                WindowRequest::Open(open) => {
                    let held = self
                        .layout
                        .screens
                        .iter()
                        .map(|placement| (placement.instance, placement.view, placement.screen))
                        .chain(
                            opened
                                .iter()
                                .map(|request| (request.instance, request.view, request.screen)),
                        )
                        .find(|(held, view, _)| *held == instance && *view == open.view);
                    if let Some((_, _, screen)) = held {
                        open.screen = screen;
                        continue;
                    }
                    self.next_opened += 1;
                    open.screen = ScreenId::opened(self.next_opened);
                    opened.push(opened_window(region, open.view, open.screen, open.size));
                }
                WindowRequest::Close(view) => {
                    let view = *view;
                    self.opened
                        .retain(|held| (held.instance, held.view) != (instance, view));
                }
                _ => {}
            }
        }
    }

    fn is_anchored(&self, screen: ScreenId) -> bool {
        self.anchored
            .iter()
            .any(|anchored| anchored.request.screen == screen)
    }

    fn anchor(&mut self, region: &ShownView, hung: Vec<Anchored>, out: &mut Vec<AnchoredScreen>) {
        let Some(space) = region
            .anchor_space
            .filter(|_| region.view.placement.reports())
        else {
            return;
        };
        let instance = region.instance;
        let mut kept: Vec<ViewId> = Vec::new();
        for anchored in hung.into_iter().take(MAX_COLLECTION_ITEMS) {
            let view = anchored.view;
            let parent = anchored.placement.parent;
            let taken = view == region.view.id
                || kept.contains(&view)
                || out
                    .iter()
                    .any(|held| (held.request.instance, held.request.view) == (instance, view))
                || self
                    .requests
                    .iter()
                    .chain(&self.opened)
                    .any(|request| (request.instance, request.view) == (instance, view));
            if taken || (parent != region.view.id && !kept.contains(&parent)) {
                continue;
            }
            kept.push(view);
            let screen = self
                .anchored
                .iter()
                .find(|held| (held.request.instance, held.request.view) == (instance, view))
                .map(|held| held.request.screen)
                .unwrap_or_else(|| {
                    self.next_opened += 1;
                    ScreenId::opened(self.next_opened)
                });
            out.push(AnchoredScreen {
                request: anchored_request(region, &space, screen, view, anchored.placement.rect),
                placement: anchored.placement,
            });
        }
    }

    fn frame_created(
        &mut self,
        opened: Vec<ScreenRequest>,
        anchored: Vec<AnchoredScreen>,
        ages: &HashMap<ScreenId, u32>,
    ) -> Vec<(ScreenPlacement, Frame)> {
        let mut created: Vec<ScreenId> = opened.iter().map(|request| request.screen).collect();
        created.extend(anchored.iter().map(|anchored| anchored.request.screen));
        let previous = self.layout.clone();
        let moved = !opened.is_empty()
            || anchored.len() != self.anchored.len()
            || anchored
                .iter()
                .zip(&self.anchored)
                .any(|(now, was)| now.request != was.request);
        for gone in self.anchored.iter().filter(|was| {
            !anchored
                .iter()
                .any(|now| now.request.screen == was.request.screen)
        }) {
            if let Some(session) = self.sessions.get_mut(&gone.request.instance) {
                session.close_anchored(gone.request.view);
            }
        }
        self.opened.extend(opened);
        self.anchored = anchored;
        if moved {
            self.relayout();
        }
        let mut begun = Vec::new();
        for screen in created {
            let Some(placement) = self.layout.placement(screen).copied() else {
                continue;
            };
            let kept = previous.placement(screen).is_some_and(|held| {
                (held.surface, held.width, held.height)
                    == (placement.surface, placement.width, placement.height)
            });
            let age = match kept {
                true => ages.get(&screen).copied().unwrap_or(0),
                false => 0,
            };
            if let Some(session) = self.sessions.get_mut(&placement.instance) {
                begun.push((placement, kept, session.begin(placement.view, age)));
            }
        }
        if begun.is_empty() {
            return Vec::new();
        }
        let regions: Vec<ShownView> = begun.iter().map(|(_, _, region)| region.clone()).collect();
        let mut frames = self
            .module
            .with_handler(|handler| handler.frame_opened(&regions))
            .unwrap_or_default();
        frames.resize_with(regions.len(), Frame::default);
        let mut framed = Vec::new();
        for ((placement, kept, region), mut frame) in begun.into_iter().zip(frames) {
            frame.changed |= !kept;
            frame.anchored.clear();
            let anchored = self
                .anchored
                .iter()
                .find(|held| held.request.screen == placement.screen)
                .map(|held| held.placement);
            if let Some(session) = self.sessions.get_mut(&region.instance) {
                session.set_anchored(region.view.id, anchored);
                framed.push((placement, session.finish(region.view.id, frame)));
                self.ran(region.instance);
            }
        }
        framed
    }

    pub(crate) fn module(&self) -> &Module {
        &self.module
    }

    pub(crate) fn commit(&mut self) -> FrameCommit {
        let mut commit = FrameCommit {
            serial: self.serial,
            sizes: Vec::new(),
            reports: Vec::new(),
            windows: Vec::new(),
            children: Vec::new(),
            repaint_after_micros: None,
            animating: false,
            presented: None,
            window_requests: Vec::new(),
            docks: Vec::new(),
        };
        let mut laid_out: Vec<_> = self.laid_out.drain().collect();
        laid_out.sort_by_key(|instance| instance.0);
        for instance in laid_out {
            if let Some(session) = self.sessions.get_mut(&instance) {
                session.commit(&mut commit);
            }
        }
        commit
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn take_dirty(&mut self) -> Dirty {
        let dirty =
            std::mem::take(&mut *self.dirty.lock().unwrap_or_else(|held| held.into_inner()));
        match std::mem::take(&mut self.everything) {
            true => Dirty::Everything,
            false => Dirty::Instances(dirty),
        }
    }

    fn mark(&mut self, instance: EditorInstanceId) {
        if self.sessions.contains_key(&instance) {
            self.dirty
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .insert(instance);
            self.reporting.insert(instance);
        }
    }

    fn open(&mut self, instance: EditorInstanceId, editor: Uuid) -> &mut EditorSession {
        let apps = &self.apps;
        let marks = Arc::clone(&self.dirty);
        let inner = self.waker.clone();
        let waker = Waker::default();
        waker.install(move || {
            marks
                .lock()
                .unwrap_or_else(|held| held.into_inner())
                .insert(instance);
            inner.wake();
        });
        let module = &self.module;
        self.sessions.entry(instance).or_insert_with(|| {
            let open = apps
                .iter()
                .find(|(declared, _)| *declared == editor)
                .map(|(_, open)| *open)
                .unwrap_or_else(|| panic!("this plugin has no editor {editor}"));
            EditorSession::joining(instance, waker, open, module)
        })
    }

    pub(crate) fn paintings(&mut self) -> Vec<u8> {
        let mut paintings = paint_format::ScreenPaintings::default();
        for placement in self.layout.screens.clone() {
            let Some(session) = self.sessions.get_mut(&placement.instance) else {
                continue;
            };
            let overflows = session.overflows(placement.view);
            if !overflows.is_empty() {
                paintings.overflows.push((placement.screen.0, overflows));
            }
            if let Some(painting) = session.painting(placement.view) {
                paintings.screens.push((placement.screen.0, painting));
                if paintings.encode().len() > block_plugin_api::MAX_BLOB_BYTES {
                    paintings.screens.pop();
                }
            }
        }
        paintings.encode()
    }

    pub(crate) fn describe(&mut self, on: bool) {
        DESCRIBE.store(on, std::sync::atomic::Ordering::Relaxed);
        self.touch_everything();
    }

    pub(crate) fn node_action(
        &mut self,
        screen: ScreenId,
        request: block_plugin_api::accesskit::ActionRequest,
    ) {
        let Some((instance, region)) = self.screen(screen) else {
            return;
        };
        if let Some(session) = self.sessions.get_mut(&instance) {
            session.node_action(region, request);
            self.mark(instance);
        }
    }

    pub(crate) fn run_action(&mut self, screen: ScreenId, id: &str) {
        let Some((instance, region)) = self.screen(screen) else {
            return;
        };
        if let Some(session) = self.sessions.get_mut(&instance) {
            session.run_action(region, id);
            self.mark(instance);
        }
    }

    pub(crate) fn layout(&self) -> &ScreenLayout {
        &self.layout
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn surface(&self) -> Option<SurfaceSpec> {
        self.surface
    }

    pub(crate) fn deliver(&mut self, delivery: Delivery) -> bool {
        match delivery {
            Delivery::Accepted(accepted) => {
                self.surface = accepted.surface;
                crate::preferences::receive(accepted.preferences);
            }
            Delivery::Preferences(preferences) => crate::preferences::receive(preferences),
            Delivery::Fonts(fonts) => crate::fonts::receive(&fonts),
            Delivery::UtcOffset(seconds) => crate::clock::receive(seconds),
            Delivery::Configure(serial) => {
                self.serial = serial;
                return false;
            }
            Delivery::Draw {
                now_micros,
                wall_micros,
                seen,
            } => {
                crate::clock::set_frame_time(
                    std::time::Duration::from_micros(now_micros),
                    std::time::Duration::from_micros(wall_micros),
                );
                SEEN.store(seen, std::sync::atomic::Ordering::Relaxed);
                return false;
            }
            Delivery::Open(opening) => {
                let instance = opening.instance();
                self.open_editor(opening);
                if !self.sessions.contains_key(&instance) {
                    return false;
                }
                self.mark(instance);
                return true;
            }
            Delivery::Close(instance) => {
                self.sessions.remove(&instance);
                self.requests.retain(|request| request.instance != instance);
                self.opened.retain(|request| request.instance != instance);
                self.anchored
                    .retain(|anchored| anchored.request.instance != instance);
                self.relayout();
            }
            Delivery::Screens(screens) => {
                self.opened.retain(|opened| {
                    !screens.iter().any(|request| {
                        (request.instance, request.view) == (opened.instance, opened.view)
                    })
                });
                self.requests = screens;
                self.relayout();
            }
            Delivery::Input(batch) => {
                let Some((instance, region)) = self.screen(batch.screen) else {
                    return false;
                };
                let Some(session) = self.sessions.get_mut(&instance) else {
                    return false;
                };
                if !batch.dismissed.is_empty() {
                    session.dismiss_anchored(region, &batch.dismissed);
                }
                for event in &batch.events {
                    let over = event.over.and_then(|over| {
                        self.layout.placement(over).map(|placement| ViewRef {
                            instance: placement.instance,
                            view: placement.view,
                        })
                    });
                    session.input(region, event, over);
                }
                self.mark(instance);
                return true;
            }
            Delivery::Editor(instance, call) => return self.call(instance, call),
            Delivery::Call(call) => {
                call(self);
                return true;
            }
            Delivery::Shutdown | Delivery::Paintings(_) => return false,
        }
        self.touch_everything();
        true
    }

    fn open_editor(&mut self, opening: Opening) {
        let block_types = Rc::clone(&self.block_types);
        let (instance, editor, account_id, organization_id, client_id) = match &opening {
            Opening::Block(open) => (
                open.instance,
                open.editor,
                open.account_id,
                open.organization_id,
                open.client_id,
            ),
            Opening::Creation(open) => (
                open.instance,
                open.editor,
                open.account_id,
                open.organization_id,
                open.client_id,
            ),
            Opening::Artifact(open) => (
                open.instance,
                open.editor,
                open.account_id,
                open.organization_id,
                open.client_id,
            ),
            Opening::Picker(open) => (
                open.instance,
                open.editor,
                open.account_id,
                open.organization_id,
                open.client_id,
            ),
            Opening::Rename(open) => (
                open.instance,
                open.editor,
                open.account_id,
                open.organization_id,
                open.client_id,
            ),
        };
        let session = self.open(instance, Uuid::from_bytes(editor));
        session.set_block_types(block_types);
        session.set_client_id(Uuid::from_bytes(client_id));
        session.set_account_id(Uuid::from_bytes(account_id));
        session.set_organization_id(Uuid::from_bytes(organization_id));
        match opening {
            Opening::Block(open) => {
                session.set_view_block(open.view_block.map(Uuid::from_bytes));
                session.set_editable(open.editable);
                session.connect(
                    Uuid::from_bytes(open.block_id),
                    Uuid::from_bytes(open.block_type),
                );
            }
            Opening::Creation(open) => session.connect_creation(open.template),
            Opening::Picker(open) => session.connect_picker(crate::host::PickerRequest {
                filter: open.filter,
                parent: crate::graph::BlockParent::decode(open.parent),
                sheet: open.sheet,
            }),
            Opening::Rename(open) => session.connect_rename(crate::host::RenameRequest {
                block: Uuid::from_bytes(open.block_id),
                sheet: open.sheet,
            }),
            Opening::Artifact(open) => session.connect_artifact(
                crate::Artifact {
                    block_id: Uuid::from_bytes(open.block_id),
                    block_type: Uuid::from_bytes(open.block_type),
                    source: Uuid::from_bytes(open.source),
                },
                open.data,
            ),
        }
    }

    pub(crate) fn call(
        &mut self,
        instance: EditorInstanceId,
        call: crate::wire::EditorCall,
    ) -> bool {
        let Some(session) = self.sessions.get_mut(&instance) else {
            return false;
        };
        call(session);
        self.mark(instance);
        true
    }

    pub(crate) fn set_catalog(&mut self, catalog: Catalog) {
        self.catalog = catalog;
        self.catalogued();
    }

    pub(crate) fn set_debug_templates(&mut self, templates: Vec<DebugTemplate>) {
        self.debug_templates = templates;
        self.catalogued();
    }

    fn catalogued(&mut self) {
        self.block_types = Rc::new(self::catalog(&self.catalog, &self.debug_templates));
        for session in self.sessions.values() {
            session.set_block_types(Rc::clone(&self.block_types));
        }
        self.touch_everything();
    }

    pub(crate) fn outbound(&mut self) {
        let missing = crate::fonts::take_missing();
        if !missing.is_empty() {
            block_plugin_api::guest::host::missing_characters(missing);
        }
        let woken = self
            .dirty
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clone();
        let everything = std::mem::take(&mut self.reporting_everything);
        let reporting = std::mem::take(&mut self.reporting);
        for (instance, session) in &mut self.sessions {
            if everything || reporting.contains(instance) || woken.contains(instance) {
                session.outbound();
            }
        }
    }

    pub(crate) fn get(&self, instance: EditorInstanceId) -> Option<&EditorSession> {
        self.sessions.get(&instance)
    }

    pub(crate) fn session(&mut self, instance: EditorInstanceId) -> Option<&mut EditorSession> {
        self.sessions.get_mut(&instance)
    }

    fn screen(&self, screen: ScreenId) -> Option<(EditorInstanceId, ViewId)> {
        self.layout
            .placement(screen)
            .map(|placement| (placement.instance, placement.view))
    }

    fn requested(&self) -> Vec<ScreenRequest> {
        let mut requested = self.requests.clone();
        requested.extend(
            self.opened
                .iter()
                .filter(|opened| {
                    !self.requests.iter().any(|request| {
                        (request.instance, request.view) == (opened.instance, opened.view)
                    })
                })
                .cloned(),
        );
        let mut hanging: Vec<(EditorInstanceId, ViewId)> = requested
            .iter()
            .map(|request| (request.instance, request.view))
            .collect();
        for anchored in &self.anchored {
            let request = &anchored.request;
            if hanging.contains(&(request.instance, anchored.placement.parent)) {
                hanging.push((request.instance, request.view));
                requested.push(request.clone());
            }
        }
        requested
    }

    fn relayout(&mut self) {
        let previous = std::mem::take(&mut self.layout);
        let requests = self.requested();
        self.layout = ScreenLayout::placed(&requests, |screen| {
            previous
                .placement(screen)
                .map_or_else(new_surface, |placement| placement.surface)
        });
        let mut placements: HashMap<EditorInstanceId, Vec<_>> = HashMap::default();
        for placement in &self.layout.screens {
            placements
                .entry(placement.instance)
                .or_default()
                .push(*placement);
        }
        for (instance, session) in &mut self.sessions {
            session.place(
                placements.get(instance).map_or(&[], Vec::as_slice),
                &requests,
            );
        }
    }
}

fn catalog(catalog: &Catalog, debug: &[DebugTemplate]) -> BlockCatalog {
    BlockCatalog::new(catalog.types.iter().map(|descriptor| {
        (
            Uuid::from_bytes(descriptor.block_type),
            BlockTypeEntry {
                display_name: descriptor.display_name.clone(),
                color: descriptor.color,
                icon: Glyph::from_codepoint(&descriptor.icon_codepoint),
                children: descriptor.children,
                press: descriptor.press,
                plain_file: descriptor.plain_file,
            },
        )
    }))
    .with_templates(catalog.templates.iter().map(|template| {
        TemplateEntry {
            color: catalog
                .types
                .iter()
                .find(|kind| kind.block_type == template.block_type)
                .and_then(|kind| kind.color),
            editor: Uuid::from_bytes(template.editor),
            template: template.template.clone(),
            block_type: Uuid::from_bytes(template.block_type),
            name: template.name.clone(),
            icon: Glyph::from_codepoint(&template.icon_codepoint),
            category: match template.category {
                _ if debug.iter().any(|listed| {
                    listed.editor == template.editor && listed.template == template.template
                }) =>
                {
                    block_ui::TemplateCategory::Debug
                }
                TemplateCategory::Important => block_ui::TemplateCategory::Important,
                TemplateCategory::Regular => block_ui::TemplateCategory::Regular,
                TemplateCategory::Template => block_ui::TemplateCategory::Template,
            },
            dialog: template.dialog,
        }
    }))
    .with_editors(catalog.editors.iter().map(|editor| EditorEntry {
        editor: Uuid::from_bytes(editor.editor),
        edits: editor.edits,
        name: editor.name.clone(),
        icon: Glyph::from_codepoint(&editor.icon_codepoint),
        extensions: editor.extensions.clone(),
    }))
}

const LARGEST_WINDOW: f32 = 16384.0;

struct AnchoredScreen {
    request: ScreenRequest,
    placement: AnchoredPlacement,
}

fn anchored_request(
    region: &ShownView,
    space: &AnchorSpace,
    screen: ScreenId,
    view: ViewId,
    rect: block_plugin_api::Rect,
) -> ScreenRequest {
    let raster = match region.raster.is_finite() && region.raster > 0.0 {
        true => region.raster,
        false => 1.0,
    };
    let per = |scale: f32| match scale.is_finite() && scale > 0.0 {
        true => raster / scale,
        false => raster,
    };
    let density = Vec2::new(per(space.to_host.scale.x), per(space.to_host.scale.y));
    let side = |points: f32| match points.is_finite() {
        true => points.clamp(0.0, LARGEST_WINDOW),
        false => 0.0,
    };
    let (width, height) = (side(rect.width()), side(rect.height()));
    let pixels = |points: f32, density: f32| ((points * density).ceil() as u32).max(1);
    ScreenRequest {
        screen,
        instance: region.instance,
        view,
        placement: ViewPlacement::Anchored,
        metrics: ViewportMetrics {
            logical: Size { width, height },
            visible: Pos2::ZERO,
            pixel_width: pixels(width, density.x),
            pixel_height: pixels(height, density.y),
            scale_factor: density.x,
            raster: density.x,
        },
        frame: None,
        windows: false,
        host_place: None,
        origin: None,
        docked: false,
        anchor_space: Some(AnchorSpace {
            to_host: HostTransform {
                origin: rect.min,
                scale: Vec2::new(1.0, 1.0),
            },
            bounds: space.bounds,
        }),
    }
}

fn opened_window(region: &ShownView, view: ViewId, screen: ScreenId, size: Size) -> ScreenRequest {
    let side = |points: f32| match points.is_finite() {
        true => points.clamp(1.0, LARGEST_WINDOW),
        false => 1.0,
    };
    let (width, height) = (side(size.width), side(size.height));
    let scale_factor = match region.scale_factor.is_finite() && region.scale_factor > 0.0 {
        true => region.scale_factor,
        false => 1.0,
    };
    let raster = match region.raster.is_finite() && region.raster > 0.0 {
        true => region.raster,
        false => scale_factor,
    };
    let pixels = |points: f32| ((points * raster).round() as u32).max(1);
    ScreenRequest {
        screen,
        instance: region.instance,
        view,
        placement: ViewPlacement::Host(HostView::Window),
        metrics: ViewportMetrics {
            logical: Size { width, height },
            visible: Pos2::ZERO,
            pixel_width: pixels(width),
            pixel_height: pixels(height),
            scale_factor,
            raster,
        },
        frame: None,
        windows: true,
        host_place: None,
        origin: None,
        docked: false,
        anchor_space: None,
    }
}

static NEXT_SURFACE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

pub(crate) fn new_surface() -> u32 {
    NEXT_SURFACE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

static DESCRIBE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn describing() -> bool {
    DESCRIBE.load(std::sync::atomic::Ordering::Relaxed)
}

static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub(crate) fn seen() -> bool {
    SEEN.load(std::sync::atomic::Ordering::Relaxed)
}
