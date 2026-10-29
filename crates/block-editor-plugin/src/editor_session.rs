use be_block::presence::{PresenceKind, UserActive, pick_free_color};
use block_plugin_api::WebViewCommand;
use block_plugin_api::accesskit::{ActionRequest, NodeId, TreeUpdate};
use block_plugin_api::guest::{debug::debug, shell};
use block_plugin_api::ops::guest::{Sender, spawn};
use block_plugin_api::{
    AnchoredView, ArtifactDescription, BarAction, ChildId, ChildPlacement, ChildPlacements,
    ChildStatus, EditorInstanceId, Failure, FrameChrome, FrameCommit, FrameReport, LayerSpace,
    MAX_CHILDREN, MAX_COLLECTION_ITEMS, MenuEntry, Occluder, ParentView, ScreenPlacement,
    ScreenRequest, ScreenWindow, Size, TimedInput, View, ViewId, ViewPlacement, ViewSize,
    ViewportMetrics, WebViewId,
};
use block_ui::BlockCatalog;
use geometry::{Pos2, Rect, Vec2, vec2};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
use uuid::Uuid;

#[cfg(target_arch = "wasm32")]
use crate::plugin::PaintTarget;
use crate::plugin::{Frame, Instance, Region};
use crate::{EditorHost, Module, Waker, host::BlockDrag};
use block_plugin_api::ViewRef;

pub type Open = fn(EditorHost) -> Box<dyn Instance>;

static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

pub struct EditorSession {
    app: Box<dyn Instance>,
    module: Module,
    opened: HashSet<ViewId>,
    instance: EditorInstanceId,
    views: HashMap<ViewId, ViewState>,
    host: EditorHost,
    own_block: Option<Uuid>,
    drag: Option<(ViewId, BlockDrag)>,
    files: Option<(ViewId, crate::host::FileDrop)>,
    intrinsic: Option<Vec2>,
    aspect_ratio: Option<f32>,
    artifact: Option<ArtifactState>,
    sent_menu: Vec<MenuEntry>,
    sent_card: bool,
    audio_watched: bool,
    docks_watched: bool,
    web_views: HashMap<WebViewId, Rc<shell::WebView>>,
    link: crate::link::Link,
    asks: Vec<block_plugin_api::WindowRequest>,
}

struct ArtifactState {
    data: Vec<u8>,
    draft: Vec<u8>,
    described: Option<Vec<u8>>,
    edited: bool,
    regenerating: Option<Sender<Result<(), Failure>>>,
}

impl ArtifactState {
    fn new(data: Vec<u8>) -> Self {
        Self {
            draft: data.clone(),
            data,
            described: None,
            edited: false,
            regenerating: None,
        }
    }

    fn settings(&mut self, data: Vec<u8>) {
        self.draft.clone_from(&data);
        self.data = data;
        self.edited = false;
    }
}

type ReportedChildren = (Vec<ChildPlacement>, Vec<Occluder>);

#[derive(Default)]
struct ViewState {
    placement: Option<ScreenPlacement>,
    metrics: Option<ViewportMetrics>,
    frame: Option<block_plugin_api::FrameSpec>,
    used: Option<Vec2>,
    reported: Option<Vec2>,
    report: Option<FrameReport>,
    reported_frame: Option<FrameReport>,
    tree: Option<TreeUpdate>,
    epoch: u64,
    window: block_plugin_api::WindowRequests,
    reported_window: Option<block_plugin_api::WindowRequests>,
    stopped: Option<block_plugin_api::Stopped>,
    reported_stopped: bool,
    children: Vec<ChildPlacement>,
    occluders: Vec<Occluder>,
    reported_children: Option<ReportedChildren>,
    age: u32,
    windows: bool,
    docked: bool,
    view_placement: Option<ViewPlacement>,
    host_place: Option<String>,
    layer_space: Option<LayerSpace>,
    anchored: Vec<AnchoredSurface>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AnchoredSurface {
    pub(crate) view: ViewId,
    pub(crate) surface: u32,
    pub(crate) pixels: [u32; 2],
}

impl Drop for EditorSession {
    fn drop(&mut self) {
        self.host.cancel_tasks();
        self.module.leave(self.instance);
    }
}

impl EditorSession {
    pub fn new(instance: EditorInstanceId, waker: Waker, open: Open) -> Self {
        Self::joining(instance, waker, open, &Module::default())
    }

    pub(crate) fn joining(
        instance: EditorInstanceId,
        waker: Waker,
        open: Open,
        module: &Module,
    ) -> Self {
        let host = EditorHost::new(waker);
        host.set_instance(instance);
        module.enter(instance, &host);
        Self::adopt(instance, open(host.clone()), host)
    }

    pub fn adopt(instance: EditorInstanceId, app: Box<dyn Instance>, host: EditorHost) -> Self {
        host.set_instance(instance);
        let module = host.module().unwrap_or_default();
        module.enter(instance, &host);
        Self {
            app,
            module,
            opened: HashSet::new(),
            instance,
            views: HashMap::new(),
            host,
            own_block: None,
            drag: None,
            files: None,
            intrinsic: None,
            aspect_ratio: None,
            artifact: None,
            sent_menu: Vec::new(),
            sent_card: false,
            audio_watched: false,
            docks_watched: false,
            web_views: HashMap::new(),
            link: crate::link::Link::default(),
            asks: Vec::new(),
        }
    }

    pub fn pick_menu(&self, id: String) {
        self.host.push_menu_pick(id);
    }

    pub fn dismiss(&self) {
        self.host.request_dismiss();
    }

    pub(crate) fn set_block_types(&self, catalog: Rc<BlockCatalog>) {
        self.host.set_block_types(catalog);
    }

    pub fn instance(&self) -> &dyn Instance {
        self.app.as_ref()
    }

    pub fn instance_mut(&mut self) -> &mut dyn Instance {
        self.app.as_mut()
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub(crate) fn join(&mut self, module: &Module) {
        self.module.leave(self.instance);
        module.enter(self.instance, &self.host);
        self.module = module.clone();
    }

    fn open(&mut self, view: ViewId) {
        if self.opened.insert(view) {
            let view = self.view(view);
            self.app.open_view(view);
        }
    }

    fn view(&self, id: ViewId) -> View {
        let placement = self.views.get(&id).and_then(|state| state.view_placement);
        match placement {
            Some(placement) => View { id, placement },
            None => View::requested(id).unwrap_or(View::dock(id)),
        }
    }

    pub(crate) fn set_view_block(&self, view_block: Option<Uuid>) {
        self.host.set_view_block(view_block);
    }

    pub(crate) fn set_client_id(&self, client_id: Uuid) {
        self.host.set_client_id(client_id);
    }

    pub(crate) fn set_account_id(&self, account: Uuid) {
        self.host.set_account_id(account);
    }

    pub(crate) fn set_organization_id(&self, organization: Uuid) {
        self.host.set_organization_id(organization);
    }

    pub(crate) fn set_editable(&self, editable: bool) {
        self.host.set_editable(editable);
    }

    pub(crate) fn set_focused_block(&self, focused: crate::host::FocusedBlock) {
        self.host.set_focused_block(focused);
    }

    pub(crate) fn show_dialog(&self, block_id: Uuid, dialog: block_plugin_api::ShellDialog) {
        self.host.show_dialog(block_id, dialog);
    }

    pub(crate) fn set_view(&self, view: Rect, scale: f32) {
        self.host.set_view(view, scale);
    }

    pub(crate) fn resized(&mut self, size: Vec2) {
        self.app.resized(size);
    }

    pub fn presence_visible(&mut self, visible: bool) {
        if self.own_block.is_some() {
            let value = visible.then(|| {
                let peers = self
                    .host
                    .peers_since(None, 0)
                    .map(|(_, peers)| peers)
                    .unwrap_or_default();
                let used = peers
                    .iter()
                    .filter(|peer| peer.kind == UserActive::ID)
                    .filter_map(|peer| serde_json::from_slice::<UserActive>(&peer.value).ok())
                    .map(|user| user.color);
                let color = pick_free_color(used);
                serde_json::to_vec(&UserActive { color }).unwrap_or_default()
            });
            self.host.show_presence(None, UserActive::ID, value);
        }
        self.app.presence_visible(visible);
    }

    pub(crate) fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        self.app.replace_child(old, new)
    }

    pub(crate) fn set_presenting(&self, presenting: bool) {
        self.host.set_presenting(presenting);
    }

    pub(crate) fn set_drag(&mut self, drag: Option<(ViewId, BlockDrag)>) {
        self.drag = drag;
    }

    pub(crate) fn set_files(&mut self, files: Option<(ViewId, crate::host::FileDrop)>) {
        self.files = files;
    }

    pub fn connect(&mut self, block_id: Uuid, block_type: Uuid) {
        self.host.set_block_type(block_type);
        self.own_block = Some(block_id);
        self.link.connect(block_id, block_type);
        self.app.connect(block_id);
    }

    pub(crate) fn connect_picker(&mut self, picker: crate::host::PickerRequest) {
        self.host.set_editable(true);
        self.host.set_picker(picker);
        self.app.connect(Uuid::nil());
    }

    pub(crate) fn connect_creation(&mut self, template: String) {
        self.host.set_editable(true);
        self.app.connect_creation(template);
    }

    pub(crate) fn connect_artifact(&mut self, artifact: crate::Artifact, data: Vec<u8>) {
        self.artifact = Some(ArtifactState::new(data));
        self.host.set_editable(true);
        self.app.connect_artifact(artifact);
    }

    pub(crate) fn artifact_settings(&mut self, data: Vec<u8>) {
        if let Some(artifact) = &mut self.artifact {
            artifact.settings(data);
        }
    }

    pub(crate) fn regenerate_artifact(
        &mut self,
        data: &[u8],
        regenerated: Sender<Result<(), Failure>>,
    ) {
        let Some(artifact) = &mut self.artifact else {
            regenerated.send(Err(Failure::Unavailable));
            return;
        };
        artifact.regenerating = Some(regenerated);
        self.app.regenerate_artifact(data);
    }

    pub(crate) fn commit_creation(&mut self) -> Result<[u8; 16], Failure> {
        self.app
            .create_block()
            .map(Uuid::into_bytes)
            .map_err(Failure::Failed)
    }

    pub fn place(&mut self, placements: &[ScreenPlacement], requests: &[ScreenRequest]) {
        self.views
            .retain(|view, _| placements.iter().any(|it| it.view == *view));
        for placement in placements {
            let Some(request) = requests
                .iter()
                .find(|request| request.screen == placement.screen)
            else {
                continue;
            };
            let state = self.views.entry(placement.view).or_default();
            if state.placement.map(|placed| placed.screen) != Some(placement.screen) {
                state.epoch = NEXT_EPOCH.fetch_add(1, Ordering::Relaxed);
                state.tree = None;
            }
            state.placement = Some(*placement);
            state.metrics = Some(request.metrics.clone());
            state.frame = request.frame;
            state.windows = request.windows;
            state.docked = request.docked;
            state.view_placement = Some(request.placement);
            state.host_place.clone_from(&request.host_place);
            state.layer_space.clone_from(&request.layers);
        }
    }

    pub fn commit(&mut self, commit: &mut FrameCommit) {
        commit.sizes.extend(self.view_sizes());
        commit.reports.extend(self.frame_reports());
        commit.windows.extend(self.windows());
        commit.children.extend(self.children());
        commit.window_requests.append(&mut self.asks);
        commit
            .docks
            .extend(self.host.take_dock_messages(self.instance));
        self.retain_child_statuses();
    }

    pub fn outbound(&mut self) {
        let instance = self.instance;
        self.link.exchange(instance, &self.host);
        self.watch_docks();
        crate::asked::asking(Some(instance), || {
            for (group, measurements) in self.host.take_performance() {
                debug::performance(group, measurements);
            }
            for (block, access) in self.host.take_simulated_access() {
                debug::simulate_access(block.into_bytes(), access);
            }
        });
        for (block, command) in self.host.take_shell_commands() {
            shell::act(instance, block.into_bytes(), command);
        }
        self.report_stopped();
        if let Some(accepted) = self.host.take_drag_accepted() {
            shell::drag_accepted(instance, accepted);
        }
        let intrinsic = self.app.intrinsic_size();
        if intrinsic != self.intrinsic {
            self.intrinsic = intrinsic;
            shell::intrinsic_size(
                instance,
                intrinsic.map(|size| Size {
                    width: size.x,
                    height: size.y,
                }),
            );
        }
        let aspect_ratio = self.app.aspect_ratio();
        if aspect_ratio != self.aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            shell::aspect_ratio(instance, aspect_ratio);
        }
        if let Some(handles) = self.host.take_handles_dismiss() {
            shell::handles_dismiss(instance, handles);
        }
        for answer in self.host.take_dialog_closes() {
            shell::close_dialog(instance, answer);
        }
        for child in self.host.take_child_dismissals() {
            shell::dismiss_child(instance, child);
        }
        if let Some(artifact) = &mut self.artifact {
            if artifact.described.as_deref() != Some(artifact.data.as_slice()) {
                artifact.described = Some(artifact.data.clone());
                let description = match self.app.describe_artifact(&artifact.data) {
                    Ok(summary) => ArtifactDescription::Described(summary),
                    Err(error) => ArtifactDescription::Unreadable(error),
                };
                shell::artifact_described(instance, description);
            }
            if artifact.edited {
                artifact.edited = false;
                shell::artifact_edited(instance, artifact.draft.clone());
            }
            if artifact.regenerating.is_some()
                && let Some(result) = self.app.poll_artifact()
                && let Some(regenerated) = artifact.regenerating.take()
            {
                regenerated.send(result.map_err(Failure::Failed));
            }
        }
        if self.host.take_leave_frame() {
            shell::leave_frame(instance);
        }
        for action in self.host.take_bar_actions() {
            shell::run_bar_action(instance, action);
        }
        let menu = self.host.menu();
        if self.sent_menu != menu {
            shell::menu(instance, MenuEntry::flatten(&menu));
            self.sent_menu = menu;
        }
        for (child, id) in self.host.take_child_menu_picks() {
            shell::child_menu_pick(instance, child, id);
        }
        for text in self.host.take_copied_text() {
            shell::copy_text(instance, text);
        }
        for png in self.host.take_copied_images() {
            shell::copy_image(instance, png);
        }
        if self.host.take_paste_request() {
            shell::paste(instance);
        }
        let card = self.host.card();
        if self.sent_card != card {
            shell::show_card(instance, card);
            self.sent_card = card;
        }
        if let Some(pick) = self.host.take_picked() {
            shell::picked(instance, pick);
        }
        for call in self.host.take_acts() {
            crate::asked::asking(Some(instance), call);
        }
        for commit in self.host.take_child_commits() {
            shell::commit_child(instance, commit.child, commit.parent.encode(), commit.name);
        }
        for command in self.host.take_audio_commands() {
            self.watch_audio();
            shell::play_audio(instance, command);
        }
        for (web_view, command) in self.host.take_web_view_commands() {
            self.command_web_view(web_view, command);
        }
        for presenting in self.host.take_present_requests() {
            shell::present(instance, presenting);
        }
        for change in self.host.take_view_changes() {
            shell::change_view(instance, change);
        }
        for (block_id, block_type) in self.host.take_block_drags() {
            shell::drag_block(instance, block_id.into_bytes(), block_type.into_bytes());
        }
        if let Some(focused) = self.host.take_focus_report() {
            shell::focused(
                instance,
                block_plugin_api::shell_types::Focus {
                    block_id: focused.block_id.map(Uuid::into_bytes),
                    block_type: focused.block_type.into_bytes(),
                    via: focused.via.into_iter().map(Uuid::into_bytes).collect(),
                },
            );
        }
        for opening in self.host.take_watch_openings() {
            opening(&self.host, instance);
        }
    }

    fn windows(&mut self) -> Vec<ScreenWindow> {
        let mut windows = Vec::new();
        for state in self.views.values_mut() {
            let Some(placement) = state.placement else {
                continue;
            };
            if state.reported_window.as_ref() != Some(&state.window) {
                state.reported_window = Some(state.window.clone());
                windows.push(ScreenWindow {
                    screen: placement.screen,
                    window: state.window.clone(),
                });
            }
        }
        windows
    }

    fn watch_audio(&mut self) {
        if std::mem::replace(&mut self.audio_watched, true) {
            return;
        }
        let instance = self.instance;
        let watch = crate::asked::asking(Some(instance), || shell::AudioStatusWatch::new(instance));
        let host = self.host.clone();
        spawn(async move {
            while let Ok(status) = watch.next().await {
                host.set_audio(status);
            }
        });
    }

    fn watch_docks(&mut self) {
        if self.docks_watched || !self.host.uses_docks() {
            return;
        }
        self.docks_watched = true;
        let instance = self.instance;
        let watch = crate::asked::asking(Some(instance), || {
            block_plugin_api::DockWatch::new(instance)
        });
        let host = self.host.clone();
        spawn(async move {
            while let Ok(updates) = watch.next().await {
                host.receive_dock_updates(updates);
            }
        });
    }

    fn command_web_view(&mut self, web_view: WebViewId, command: WebViewCommand) {
        match command {
            WebViewCommand::Open(url) => {
                let instance = self.instance;
                let shown = Rc::new(crate::asked::asking(Some(instance), || {
                    shell::WebView::new(instance, web_view, url)
                }));
                let watched = Rc::downgrade(&shown);
                let host = self.host.clone();
                spawn(async move {
                    while let Some(shown) = watched.upgrade() {
                        let next = shown.next_events();
                        drop(shown);
                        let Ok(events) = next.await else {
                            break;
                        };
                        for event in events {
                            host.push_web_view_event(web_view, event);
                        }
                    }
                });
                self.web_views.insert(web_view, shown);
            }
            WebViewCommand::Load(url) => {
                if let Some(shown) = self.web_views.get(&web_view) {
                    shown.load(url);
                }
            }
            WebViewCommand::Reload => {
                if let Some(shown) = self.web_views.get(&web_view) {
                    shown.reload();
                }
            }
            WebViewCommand::FocusApp => {
                if let Some(shown) = self.web_views.get(&web_view) {
                    shown.focus_app();
                }
            }
            WebViewCommand::Close => {
                self.web_views.remove(&web_view);
            }
        }
    }

    fn report_stopped(&mut self) {
        for (view, state) in &mut self.views {
            if let Some(stopped) = &state.stopped
                && !std::mem::replace(&mut state.reported_stopped, true)
            {
                shell::view_stopped(self.instance, *view, stopped.clone());
            }
        }
    }

    fn frame_reports(&mut self) -> Vec<FrameReport> {
        let mut reports = Vec::new();
        for state in self.views.values_mut() {
            let Some(report) = &state.report else {
                continue;
            };
            if state.reported_frame.as_ref() == Some(report) && state.tree.is_none() {
                continue;
            }
            state.reported_frame = Some(report.clone());
            let mut report = report.clone();
            if let Some(description) = &mut report.description {
                description.tree = state.tree.take().as_ref().map(Into::into);
            }
            reports.push(report);
        }
        reports
    }

    fn view_sizes(&mut self) -> Vec<ViewSize> {
        let mut sizes = Vec::new();
        for state in self.views.values_mut() {
            let (Some(placement), Some(used)) = (state.placement, state.used) else {
                continue;
            };
            if state.reported == Some(used) {
                continue;
            }
            state.reported = Some(used);
            sizes.push(ViewSize {
                screen: placement.screen,
                logical: Size {
                    width: used.x,
                    height: used.y,
                },
            });
        }
        sizes
    }

    fn used(&mut self, view: ViewId, content: Rect) {
        let Some(state) = self.views.get_mut(&view) else {
            return;
        };
        let used = content.max.to_vec2().max(Vec2::ZERO);
        state.used = Some(vec2(used.x.round(), used.y.round()));
    }

    pub(crate) fn child_bar_action(&self, child: ChildId, action: BarAction) {
        self.host.push_child_bar_action(child, action);
    }

    pub(crate) fn child_focused(&self, child: ChildId) {
        self.host.push_child_focus(child);
    }

    pub(crate) fn set_child_statuses(&self, statuses: Vec<ChildStatus>) {
        self.host.set_child_statuses(statuses);
    }

    fn children(&mut self) -> Vec<ChildPlacements> {
        let instance = self.instance;
        let mut messages = Vec::new();
        for (view, state) in &mut self.views {
            let current = bounded(&state.children, &state.occluders);
            if state.reported_children.as_ref() == Some(&current) {
                continue;
            }
            state.reported_children = Some(current.clone());
            let (children, occluders) = current;
            messages.push(ChildPlacements {
                instance,
                view: *view,
                children,
                occluders,
            });
        }
        messages
    }

    fn retain_child_statuses(&self) {
        let live: HashSet<ChildId> = self
            .views
            .values()
            .flat_map(|state| state.children.iter().map(|child| child.child))
            .collect();
        self.host.retain_child_statuses(&live);
    }

    fn scale_factor(&self, view: ViewId) -> f32 {
        self.placement(view)
            .map_or(1.0, |placement| placement.scale_factor())
    }

    fn raster(&self, view: ViewId) -> f32 {
        self.placement(view)
            .map_or(1.0, |placement| placement.raster())
    }

    fn placement(&self, view: ViewId) -> Option<&ScreenPlacement> {
        self.views
            .get(&view)
            .and_then(|state| state.placement.as_ref())
    }

    fn metrics(&self, view: ViewId) -> Option<&ViewportMetrics> {
        let state = self.views.get(&view)?;
        state.placement.as_ref()?;
        state.metrics.as_ref()
    }

    fn rect(&self, view: ViewId) -> Rect {
        self.metrics(view).map_or(Rect::ZERO, |metrics| {
            Rect::from_min_size(
                Pos2::ZERO,
                vec2(metrics.logical.width, metrics.logical.height),
            )
        })
    }

    fn visible(&self, view: ViewId) -> Vec2 {
        self.metrics(view).map_or(Vec2::ZERO, |metrics| {
            vec2(metrics.visible.x, metrics.visible.y)
        })
    }

    fn context(&self, view: ViewId) -> Region {
        let state = self.views.get(&view);
        let shown = self.view(view);
        Region {
            instance: self.instance,
            view: shown,
            block: self.own_block,
            rect: self.rect(view),
            visible: self.visible(view),
            scale_factor: self.scale_factor(view),
            raster: self.raster(view),
            pixels: self
                .placement(view)
                .map_or([0, 0], |placement| [placement.width, placement.height]),
            age: state.map_or(0, |state| state.age),
            spec: self
                .views
                .get(&view)
                .and_then(|state| state.frame)
                .unwrap_or_default(),
            describe: state
                .filter(|_| shown.placement.reports() && crate::screens::describing())
                .map(|state| state.epoch),
            seen: crate::screens::seen(),
            windows: state.is_some_and(|state| state.windows),
            docked: state.is_some_and(|state| state.docked),
            host_place: state.and_then(|state| state.host_place.clone()),
            layers: state.and_then(|state| state.layer_space.clone()),
        }
    }

    pub fn run(&mut self, view: ViewId, age: u32) -> Frame {
        self.frame(&[(view, age)]).pop().unwrap_or_default()
    }

    pub fn frame(&mut self, views: &[(ViewId, u32)]) -> Vec<Frame> {
        let begun: Vec<Region> = views
            .iter()
            .map(|(view, age)| self.begin(*view, *age))
            .collect();
        let module = self.module.clone();
        let mut frames = module
            .with_handler(|handler| handler.frame(&begun))
            .unwrap_or_default();
        frames.resize_with(begun.len(), Frame::default);
        begun
            .iter()
            .zip(frames)
            .map(|(view, frame)| self.finish(view.view.id, frame))
            .collect()
    }

    pub fn begin(&mut self, view: ViewId, age: u32) -> Region {
        self.open(view);
        if let Some(state) = self.views.get_mut(&view) {
            state.age = age;
        }
        let context = self.context(view);
        self.host.begin_view(view);
        if view.is_main() {
            self.host
                .set_chrome_shown(context.spec.chrome == FrameChrome::Drawn);
            self.host.set_placed_content(context.spec.content);
        }
        let drag = self
            .drag
            .and_then(|(dragged, drag)| (dragged == view).then_some(drag));
        self.host.set_drag(view, drag);
        let files = self
            .files
            .as_ref()
            .and_then(|(dropped, files)| (*dropped == view).then(|| files.clone()));
        self.host.set_files(view, files);
        if view == ViewId::ARTIFACT_SETTINGS {
            let draft = self.artifact.as_ref().map(|artifact| crate::SettingsDraft {
                saved: artifact.data.clone(),
                draft: artifact.draft.clone(),
            });
            self.host.open_settings_draft(draft);
        }
        context
    }

    pub fn finish(&mut self, view: ViewId, mut frame: Frame) -> Frame {
        let host = self.rect(view);
        let draft = match view {
            ViewId::ARTIFACT_SETTINGS => self.host.close_settings_draft(),
            _ => None,
        };
        let delivered_drop = self
            .drag
            .is_some_and(|(dragged, drag)| dragged == view && drag.dropped);
        let delivered_files = self
            .files
            .as_ref()
            .is_some_and(|(dropped, files)| *dropped == view && files.dropped);
        if let Some(state) = self.views.get_mut(&view)
            && state.stopped.is_none()
        {
            state.stopped = frame.stopped.take();
        }

        self.asks.append(&mut frame.asks);
        if let Some(delay) = self.host.take_frame_request(view) {
            frame.repaint_after = Some(frame.repaint_after.map_or(delay, |held| held.min(delay)));
        }
        if let (Some(artifact), Some(draft)) = (self.artifact.as_mut(), draft) {
            artifact.edited |= artifact.draft != draft;
            artifact.draft = draft;
        }
        let (placed, occluders) = self.host.end_view(view);
        self.host.set_drag(view, None);
        self.host.set_files(view, None);
        if delivered_drop {
            self.drag = None;
        }
        if delivered_files {
            self.files = None;
        }
        let screen = self.placement(view).map(|placement| placement.screen);
        let raster = self.raster(view);
        let content = frame.content.unwrap_or(host);
        self.used(view, content);
        let reported_content = self
            .host
            .take_content(view)
            .map(|reported| reported.intersect(content))
            .filter(|reported| reported.is_positive())
            .unwrap_or(content);
        if let (Some(state), Some(screen)) = (self.views.get_mut(&view), screen) {
            state.children = placed;
            state.occluders = occluders;
            state.window = frame.window.clone();
            let reports = state
                .view_placement
                .unwrap_or(ViewPlacement::Parent(ParentView::Main))
                .reports();
            let opened = match reports && state.layer_space.is_some() {
                true => std::mem::take(&mut frame.anchored),
                false => Vec::new(),
            };
            let density = state
                .layer_space
                .as_ref()
                .map_or(Vec2::splat(raster), |space| {
                    let scale = space.to_host.scale;
                    let per = |scale: f32| match scale > 0.0 {
                        true => raster / scale,
                        false => raster,
                    };
                    vec2(per(scale.x), per(scale.y))
                });
            let anchored = surfaced(view, &mut state.anchored, opened, density);
            frame.anchored.clone_from(&anchored);
            match frame.description.as_mut() {
                Some(description) => {
                    if let Some(tree) = description.tree.take() {
                        merge_tree(&mut state.tree, tree);
                    }
                }
                None => state.tree = None,
            }
            state.report = reports.then(|| FrameReport {
                screen,
                content: block_plugin_api::Insets::between(host, reported_content),
                painted: frame.painted.clone(),
                floating: frame.floating.clone(),
                floating_painted: frame.floating_painted.clone(),
                scrims: frame.scrims.clone(),
                drawing: frame.drawing.clone(),
                claims: frame
                    .claims
                    .iter()
                    .map(|claim| block_plugin_api::PressClaim {
                        modifiers: claim.modifiers,
                        rect: claim.rect,
                    })
                    .collect(),
                description: frame.description.as_ref().map(|description| {
                    block_plugin_api::accessibility::Description {
                        tree: None,
                        actions: description.actions.clone(),
                    }
                }),
                anchored,
            });
        }
        frame
    }

    fn id(&self, view: ViewId) -> ViewRef {
        ViewRef {
            instance: self.instance,
            view,
        }
    }

    pub(crate) fn painting(&mut self, view: ViewId) -> Option<paint_format::Painting> {
        let id = self.id(view);
        self.module.with_handler(|handler| handler.painting(id))?
    }

    pub(crate) fn anchored_paintings(
        &mut self,
        view: ViewId,
    ) -> Vec<(ViewId, paint_format::Painting)> {
        let id = self.id(view);
        let anchored: Vec<ViewId> = self
            .anchored_surfaces(view)
            .iter()
            .map(|surface| surface.view)
            .collect();
        self.module
            .with_handler(|handler| {
                anchored
                    .into_iter()
                    .filter_map(|shown| Some((shown, handler.anchored_painting(id, shown)?)))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn overflows(&mut self, view: ViewId) -> Vec<String> {
        let id = self.id(view);
        self.module
            .with_handler(|handler| handler.overflows(id))
            .unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn paint(
        &mut self,
        view: ViewId,
        target: &PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        let id = self.id(view);
        self.module
            .with_handler(|handler| handler.paint(id, target))
            .unwrap_or_default()
    }

    pub(crate) fn anchored_surfaces(&self, view: ViewId) -> &[AnchoredSurface] {
        self.views
            .get(&view)
            .map_or(&[], |state| state.anchored.as_slice())
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn paint_anchored(
        &mut self,
        view: ViewId,
        anchored: ViewId,
        target: &PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        let id = self.id(view);
        self.module
            .with_handler(|handler| handler.paint_anchored(id, anchored, target))
            .unwrap_or_default()
    }

    pub(crate) fn dismiss_anchored(&mut self, view: ViewId, anchored: &[ViewId]) {
        self.open(view);
        let context = self.context(view);
        self.module
            .with_handler(|handler| handler.dismiss_anchored(&context, anchored));
    }

    pub(crate) fn input(&mut self, view: ViewId, event: &TimedInput, over: Option<ViewRef>) {
        self.open(view);
        let context = self.context(view);
        self.module
            .with_handler(|handler| handler.input(&context, event, over));
    }

    pub(crate) fn node_action(&mut self, view: ViewId, request: ActionRequest) {
        self.open(view);
        let context = self.context(view);
        self.module
            .with_handler(|handler| handler.node_action(&context, request));
    }

    pub(crate) fn run_action(&mut self, view: ViewId, id: &str) {
        self.open(view);
        let context = self.context(view);
        self.module
            .with_handler(|handler| handler.run_action(&context, id));
    }

    pub fn report(&self, view: ViewId) -> Option<&FrameReport> {
        self.views
            .get(&view)
            .and_then(|state| state.report.as_ref())
    }

    pub fn placed_children(&self, view: ViewId) -> &[ChildPlacement] {
        self.views
            .get(&view)
            .map_or(&[], |state| state.children.as_slice())
    }

    pub fn occluders(&self, view: ViewId) -> &[Occluder] {
        self.views
            .get(&view)
            .map_or(&[], |state| state.occluders.as_slice())
    }

    pub fn host(&self) -> &EditorHost {
        &self.host
    }
}

fn bounded(
    children: &[ChildPlacement],
    occluders: &[Occluder],
) -> (Vec<ChildPlacement>, Vec<Occluder>) {
    let kept = children.len().min(MAX_CHILDREN);
    (
        children[..kept].to_vec(),
        occluders
            .iter()
            .take(MAX_COLLECTION_ITEMS)
            .map(|occluder| Occluder {
                after: occluder.after.min(kept as u32),
                rect: occluder.rect,
            })
            .collect(),
    )
}

fn surfaced(
    owner: ViewId,
    pool: &mut Vec<AnchoredSurface>,
    opened: Vec<AnchoredView>,
    density: Vec2,
) -> Vec<AnchoredView> {
    let mut kept: Vec<AnchoredView> = Vec::new();
    for mut anchored in opened.into_iter().take(MAX_COLLECTION_ITEMS) {
        let listed = |id: ViewId| kept.iter().any(|other| other.view == id);
        let parent = anchored.placement.parent;
        let orphan = parent != owner && !listed(parent);
        if anchored.view == owner || listed(anchored.view) || orphan {
            continue;
        }
        anchored.painted.truncate(MAX_COLLECTION_ITEMS);
        anchored.floating.truncate(MAX_COLLECTION_ITEMS);
        anchored.scrims.truncate(MAX_COLLECTION_ITEMS);
        kept.push(anchored);
    }
    let held = std::mem::take(pool);
    for anchored in &mut kept {
        anchored.surface = held
            .iter()
            .find(|surface| surface.view == anchored.view)
            .map_or_else(crate::screens::new_surface, |surface| surface.surface);
        pool.push(AnchoredSurface {
            view: anchored.view,
            surface: anchored.surface,
            pixels: [
                pixels(anchored.rect.width(), density.x),
                pixels(anchored.rect.height(), density.y),
            ],
        });
    }
    kept
}

fn pixels(points: f32, density: f32) -> u32 {
    (points * density).ceil().max(1.0) as u32
}

fn merge_tree(held: &mut Option<TreeUpdate>, update: TreeUpdate) {
    let Some(held) = held else {
        *held = Some(update);
        return;
    };
    let replaced: HashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
    held.nodes.retain(|(id, _)| !replaced.contains(id));
    held.nodes.extend(update.nodes);
    if update.tree.is_some() {
        held.tree = update.tree;
    }
    held.focus = update.focus;
}
