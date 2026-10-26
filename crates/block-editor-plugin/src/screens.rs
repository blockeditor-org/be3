use block_plugin_api::{
    Catalog, DebugTemplate, EditorInstanceId, EditorRegion, Edits, FrameCommit, Opening, ScreenId,
    ScreenLayout, ScreenRequest, SurfaceSpec, TemplateCategory,
};
use block_ui::{BlockCatalog, BlockTypeEntry, EditorEntry, TemplateEntry};
use lucide::Glyph;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::{
    Waker,
    editor_session::{EditorSession, Open},
    wire::Delivery,
};

pub(crate) type Opener = Open;

pub(crate) struct Screens {
    sessions: HashMap<EditorInstanceId, EditorSession>,
    apps: Vec<(Uuid, Opener)>,
    waker: Waker,
    requests: Vec<ScreenRequest>,
    layout: ScreenLayout,
    block_types: Rc<BlockCatalog>,
    catalog: Catalog,
    debug_templates: Vec<DebugTemplate>,
    surface: Option<SurfaceSpec>,
    next_surface: u32,
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
            sessions: HashMap::new(),
            apps,
            waker,
            requests: Vec::new(),
            layout: ScreenLayout::default(),
            block_types: Rc::new(BlockCatalog::default()),
            catalog: Catalog::default(),
            debug_templates: Vec::new(),
            surface: None,
            next_surface: 0,
            dirty: Arc::default(),
            everything: true,
            reporting: HashSet::new(),
            reporting_everything: true,
            laid_out: HashSet::new(),
            serial: 0,
        }
    }

    pub(crate) fn adopt(&mut self, instance: EditorInstanceId, session: EditorSession) {
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
        self.sessions.entry(instance).or_insert_with(|| {
            let open = apps
                .iter()
                .find(|(declared, _)| *declared == editor)
                .map(|(_, open)| *open)
                .unwrap_or_else(|| panic!("this plugin has no editor {editor}"));
            EditorSession::new(instance, waker, open)
        })
    }

    pub(crate) fn paintings(&mut self) -> Vec<u8> {
        let mut paintings = paint_format::ScreenPaintings::default();
        for placement in self.layout.screens.clone() {
            let Some(session) = self.sessions.get_mut(&placement.instance) else {
                continue;
            };
            let overflows = session.overflows(placement.region);
            if !overflows.is_empty() {
                paintings.overflows.push((placement.screen.0, overflows));
            }
            if let Some(painting) = session.painting(placement.region) {
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
            Delivery::Draw { now_micros, seen } => {
                crate::clock::set_frame_time(std::time::Duration::from_micros(now_micros));
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
                self.relayout();
            }
            Delivery::Screens(screens) => {
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
                for event in &batch.events {
                    session.input(region, event);
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
            Opening::Artifact(open) => session.connect_artifact(
                Uuid::from_bytes(open.block_id),
                Uuid::from_bytes(open.block_type),
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

    fn screen(&self, screen: ScreenId) -> Option<(EditorInstanceId, EditorRegion)> {
        self.layout
            .placement(screen)
            .map(|placement| (placement.instance, placement.region))
    }

    fn relayout(&mut self) {
        let previous = std::mem::take(&mut self.layout);
        let next = &mut self.next_surface;
        self.layout = ScreenLayout::placed(&self.requests, |screen| {
            previous
                .placement(screen)
                .map(|placement| placement.surface)
                .unwrap_or_else(|| {
                    *next += 1;
                    *next
                })
        });
        let mut placements: HashMap<EditorInstanceId, Vec<_>> = HashMap::new();
        for placement in &self.layout.screens {
            placements
                .entry(placement.instance)
                .or_default()
                .push(*placement);
        }
        for (instance, session) in &mut self.sessions {
            session.place(
                placements.get(instance).map_or(&[], Vec::as_slice),
                &self.requests,
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
                child_edits: block_ui::ChildEdits {
                    add: descriptor.children.add,
                    delete: descriptor.children.delete,
                    replace: descriptor.children.replace,
                },
                press: match descriptor.press {
                    block_plugin_api::Press::Open => block_ui::Press::Open,
                    block_plugin_api::Press::Zoom => block_ui::Press::Zoom,
                },
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
        edits: match editor.edits {
            Edits::Type(block_type) => block_ui::Edits::Type(Uuid::from_bytes(block_type)),
            Edits::Any => block_ui::Edits::Any,
            Edits::Nothing => block_ui::Edits::Nothing,
        },
        name: editor.name.clone(),
        icon: Glyph::from_codepoint(&editor.icon_codepoint),
    }))
}

static DESCRIBE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn describing() -> bool {
    DESCRIBE.load(std::sync::atomic::Ordering::Relaxed)
}

static SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub(crate) fn seen() -> bool {
    SEEN.load(std::sync::atomic::Ordering::Relaxed)
}
