use be_block::presence::{PresenceKind, UserActive, pick_free_color};
use block_plugin_api::WebViewCommand;
use block_plugin_api::accesskit::{ActionRequest, NodeId, TreeUpdate};
use block_plugin_api::guest::{debug::debug, shell};
use block_plugin_api::ops::guest::{Sender, spawn};
use block_plugin_api::{
    ArtifactDescription, BarAction, ChildId, ChildPlacement, ChildPlacements, ChildStatus,
    EditorInstanceId, EditorRegion, Failure, FrameChrome, FrameCommit, FrameReport, MAX_CHILDREN,
    MAX_COLLECTION_ITEMS, MenuEntry, Occluder, RegionSize, ScreenPlacement, ScreenRequest,
    ScreenWindow, ScreenWindowRequest, Size, TimedInput, ViewportMetrics, WebViewId, WindowReport,
    WindowRequest,
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
use crate::plugin::{Frame, Instance, Region, RegionId};
use crate::{EditorHost, Module, Waker, host::BlockDrag};

pub type Open = fn(EditorHost) -> Box<dyn Instance>;

static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);

pub struct EditorSession {
    app: Box<dyn Instance>,
    module: Module,
    opened: HashSet<EditorRegion>,
    instance: EditorInstanceId,
    regions: HashMap<EditorRegion, RegionState>,
    host: EditorHost,
    own_block: Option<Uuid>,
    drag: Option<(EditorRegion, BlockDrag)>,
    files: Option<(EditorRegion, crate::host::FileDrop)>,
    intrinsic: Option<Vec2>,
    aspect_ratio: Option<f32>,
    artifact: Option<ArtifactState>,
    sent_menu: Vec<MenuEntry>,
    picks: HashMap<u64, Sender<block_plugin_api::BlockPick>>,
    audio_watched: bool,
    web_views: HashMap<WebViewId, Rc<shell::WebView>>,
    link: crate::link::Link,
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
struct RegionState {
    placement: Option<ScreenPlacement>,
    metrics: Option<ViewportMetrics>,
    frame: Option<block_plugin_api::FrameSpec>,
    opens_windows: bool,
    window_requests: Vec<WindowRequest>,
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
            regions: HashMap::new(),
            host,
            own_block: None,
            drag: None,
            files: None,
            intrinsic: None,
            aspect_ratio: None,
            artifact: None,
            sent_menu: Vec::new(),
            picks: HashMap::new(),
            audio_watched: false,
            web_views: HashMap::new(),
            link: crate::link::Link::default(),
        }
    }

    pub fn pick_menu(&self, id: String) {
        self.host.push_menu_pick(id);
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

    fn open(&mut self, region: EditorRegion) {
        if self.opened.insert(region) {
            self.app.open_region(region);
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

    pub(crate) fn pick_requested(
        &mut self,
        request: block_plugin_api::shell_types::PickRequest,
        answer: Sender<block_plugin_api::BlockPick>,
    ) {
        let block_plugin_api::shell_types::PickRequest {
            pick,
            filter,
            parent,
            within,
            touch,
        } = request;
        let parent = crate::graph::BlockParent::decode(parent);
        let within = within.map(Uuid::from_bytes);
        self.picks.insert(pick, answer);
        let anchor = filter.anchor;
        self.host.request_pick(crate::host::PickRequest {
            pick,
            filter,
            parent,
            within,
            anchor,
            touch,
        });
    }

    pub(crate) fn withdraw_pick(&mut self, pick: u64) {
        if self.picks.remove(&pick).is_some() {
            self.host.withdraw_pick(pick);
        }
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

    pub(crate) fn set_drag(&mut self, drag: Option<(EditorRegion, BlockDrag)>) {
        self.drag = drag;
    }

    pub(crate) fn set_files(&mut self, files: Option<(EditorRegion, crate::host::FileDrop)>) {
        self.files = files;
    }

    pub fn connect(&mut self, block_id: Uuid, block_type: Uuid) {
        self.host.set_block_type(block_type);
        self.own_block = Some(block_id);
        self.link.connect(block_id, block_type);
        self.app.connect(block_id);
    }

    pub(crate) fn connect_creation(&mut self, template: String) {
        self.host.set_editable(true);
        self.app.connect_creation(template);
    }

    pub(crate) fn connect_artifact(&mut self, block_id: Uuid, block_type: Uuid, data: Vec<u8>) {
        self.artifact = Some(ArtifactState::new(data));
        self.host.set_editable(true);
        self.app.connect_artifact(crate::Artifact {
            block_id,
            block_type,
        });
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
        self.regions
            .retain(|region, _| placements.iter().any(|it| it.region == *region));
        for placement in placements {
            let Some(request) = requests
                .iter()
                .find(|request| request.screen == placement.screen)
            else {
                continue;
            };
            let state = self.regions.entry(placement.region).or_default();
            if state.placement.map(|placed| placed.screen) != Some(placement.screen) {
                state.epoch = NEXT_EPOCH.fetch_add(1, Ordering::Relaxed);
                state.tree = None;
            }
            state.placement = Some(*placement);
            state.metrics = Some(request.metrics.clone());
            state.frame = request.frame;
            state.opens_windows = request.opens_windows;
        }
    }

    pub fn commit(&mut self, commit: &mut FrameCommit) {
        commit.sizes.extend(self.region_sizes());
        commit.reports.extend(self.frame_reports());
        commit.windows.extend(self.windows());
        commit.window_requests.extend(self.window_requests());
        commit.children.extend(self.children());
        self.retain_child_statuses();
    }

    pub fn outbound(&mut self) {
        let instance = self.instance;
        self.link.exchange(instance, &self.host);
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
        if let Some(ready) = self.host.take_creation_ready() {
            shell::creation_ready(instance, ready);
        }
        if let Some(artifact) = &mut self.artifact {
            if artifact.described.as_deref() != Some(artifact.data.as_slice()) {
                artifact.described = Some(artifact.data.clone());
                let description = match self.app.describe_artifact(&artifact.data) {
                    Ok(description) => ArtifactDescription::Described {
                        source: description.source.into_bytes(),
                        summary: description.summary,
                    },
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
        for (pick, answer) in self.host.take_pick_answers() {
            if let Some(asked) = self.picks.remove(&pick) {
                asked.send(answer);
            }
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

    pub(crate) fn window_reported(&mut self, report: WindowReport) {
        let instance = self.instance;
        self.module
            .with_handler(|handler| handler.window_reported(instance, report));
    }

    fn window_requests(&mut self) -> Vec<ScreenWindowRequest> {
        let mut requests = Vec::new();
        for state in self.regions.values_mut() {
            let Some(placement) = state.placement else {
                continue;
            };
            requests.extend(
                state
                    .window_requests
                    .drain(..)
                    .map(|request| ScreenWindowRequest {
                        screen: placement.screen,
                        request,
                    }),
            );
        }
        requests
    }

    fn windows(&mut self) -> Vec<ScreenWindow> {
        let mut windows = Vec::new();
        for state in self.regions.values_mut() {
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
        for (region, state) in &mut self.regions {
            if let Some(stopped) = &state.stopped
                && !std::mem::replace(&mut state.reported_stopped, true)
            {
                shell::region_stopped(self.instance, *region, stopped.clone());
            }
        }
    }

    fn frame_reports(&mut self) -> Vec<FrameReport> {
        let mut reports = Vec::new();
        for state in self.regions.values_mut() {
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

    fn region_sizes(&mut self) -> Vec<RegionSize> {
        let mut sizes = Vec::new();
        for state in self.regions.values_mut() {
            let (Some(placement), Some(used)) = (state.placement, state.used) else {
                continue;
            };
            if state.reported == Some(used) {
                continue;
            }
            state.reported = Some(used);
            sizes.push(RegionSize {
                screen: placement.screen,
                logical: Size {
                    width: used.x,
                    height: used.y,
                },
            });
        }
        sizes
    }

    fn used(&mut self, region: EditorRegion, content: Rect) {
        let Some(state) = self.regions.get_mut(&region) else {
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
        for (region, state) in &mut self.regions {
            let current = bounded(&state.children, &state.occluders);
            if state.reported_children.as_ref() == Some(&current) {
                continue;
            }
            state.reported_children = Some(current.clone());
            let (children, occluders) = current;
            messages.push(ChildPlacements {
                instance,
                region: *region,
                children,
                occluders,
            });
        }
        messages
    }

    fn retain_child_statuses(&self) {
        let live: HashSet<ChildId> = self
            .regions
            .values()
            .flat_map(|state| state.children.iter().map(|child| child.child))
            .collect();
        self.host.retain_child_statuses(&live);
    }

    fn scale_factor(&self, region: EditorRegion) -> f32 {
        self.placement(region)
            .map_or(1.0, |placement| placement.scale_factor())
    }

    fn raster(&self, region: EditorRegion) -> f32 {
        self.placement(region)
            .map_or(1.0, |placement| placement.raster())
    }

    fn placement(&self, region: EditorRegion) -> Option<&ScreenPlacement> {
        self.regions
            .get(&region)
            .and_then(|state| state.placement.as_ref())
    }

    fn metrics(&self, region: EditorRegion) -> Option<&ViewportMetrics> {
        let state = self.regions.get(&region)?;
        state.placement.as_ref()?;
        state.metrics.as_ref()
    }

    fn rect(&self, region: EditorRegion) -> Rect {
        self.metrics(region).map_or(Rect::ZERO, |metrics| {
            Rect::from_min_size(
                Pos2::ZERO,
                vec2(metrics.logical.width, metrics.logical.height),
            )
        })
    }

    fn visible(&self, region: EditorRegion) -> Vec2 {
        self.metrics(region).map_or(Vec2::ZERO, |metrics| {
            vec2(metrics.visible.x, metrics.visible.y)
        })
    }

    fn context(&self, region: EditorRegion) -> Region {
        let state = self.regions.get(&region);
        Region {
            instance: self.instance,
            region,
            block: self.own_block,
            rect: self.rect(region),
            visible: self.visible(region),
            scale_factor: self.scale_factor(region),
            raster: self.raster(region),
            pixels: self
                .placement(region)
                .map_or([0, 0], |placement| [placement.width, placement.height]),
            age: state.map_or(0, |state| state.age),
            spec: self
                .regions
                .get(&region)
                .and_then(|state| state.frame)
                .unwrap_or_default(),
            describe: state
                .filter(|_| region == EditorRegion::Frame && crate::screens::describing())
                .map(|state| state.epoch),
            seen: crate::screens::seen(),
            opens_windows: state.is_some_and(|state| state.opens_windows),
        }
    }

    pub fn run(&mut self, region: EditorRegion, age: u32) -> Frame {
        self.frame(&[(region, age)]).pop().unwrap_or_default()
    }

    pub fn frame(&mut self, regions: &[(EditorRegion, u32)]) -> Vec<Frame> {
        let begun: Vec<Region> = regions
            .iter()
            .map(|(region, age)| self.begin(*region, *age))
            .collect();
        let module = self.module.clone();
        let mut frames = module
            .with_handler(|handler| handler.frame(&begun))
            .unwrap_or_default();
        frames.resize_with(begun.len(), Frame::default);
        begun
            .iter()
            .zip(frames)
            .map(|(region, frame)| self.finish(region.region, frame))
            .collect()
    }

    pub fn begin(&mut self, region: EditorRegion, age: u32) -> Region {
        self.open(region);
        if let Some(state) = self.regions.get_mut(&region) {
            state.age = age;
        }
        let context = self.context(region);
        self.host.begin_region(region);
        if region == EditorRegion::Frame {
            self.host
                .set_chrome_shown(context.spec.chrome == FrameChrome::Drawn);
            self.host.set_placed_content(context.spec.content);
        }
        let drag = self
            .drag
            .and_then(|(dragged, drag)| (dragged == region).then_some(drag));
        self.host.set_drag(region, drag);
        let files = self
            .files
            .as_ref()
            .and_then(|(dropped, files)| (*dropped == region).then(|| files.clone()));
        self.host.set_files(region, files);
        if region == EditorRegion::ArtifactSettings {
            let draft = self
                .artifact
                .as_ref()
                .map(|artifact| artifact.draft.clone());
            self.host.open_settings_draft(draft);
        }
        context
    }

    pub fn finish(&mut self, region: EditorRegion, mut frame: Frame) -> Frame {
        let host = self.rect(region);
        let draft = match region {
            EditorRegion::ArtifactSettings => self.host.close_settings_draft(),
            EditorRegion::Frame | EditorRegion::Preview | EditorRegion::Window(_) => None,
        };
        let delivered_drop = self
            .drag
            .is_some_and(|(dragged, drag)| dragged == region && drag.dropped);
        let delivered_files = self
            .files
            .as_ref()
            .is_some_and(|(dropped, files)| *dropped == region && files.dropped);
        if let Some(state) = self.regions.get_mut(&region)
            && state.stopped.is_none()
        {
            state.stopped = frame.stopped.take();
        }

        if let Some(delay) = self.host.take_frame_request(region) {
            frame.repaint_after = Some(frame.repaint_after.map_or(delay, |held| held.min(delay)));
        }
        if let (Some(artifact), Some(draft)) = (self.artifact.as_mut(), draft) {
            artifact.edited |= artifact.draft != draft;
            artifact.draft = draft;
        }
        let (placed, occluders) = self.host.end_region(region);
        self.host.set_drag(region, None);
        self.host.set_files(region, None);
        if delivered_drop {
            self.drag = None;
        }
        if delivered_files {
            self.files = None;
        }
        let screen = self.placement(region).map(|placement| placement.screen);
        let content = frame.content.unwrap_or(host);
        self.used(region, content);
        let reported_content = self
            .host
            .take_content(region)
            .map(|reported| reported.intersect(content))
            .filter(|reported| reported.is_positive())
            .unwrap_or(content);
        if let (Some(state), Some(screen)) = (self.regions.get_mut(&region), screen) {
            state.children = placed;
            state.occluders = occluders;
            state
                .window_requests
                .extend(std::mem::take(&mut frame.window_requests));
            state.window = frame.window.clone();
            let reports = matches!(region, EditorRegion::Frame | EditorRegion::Window(_));
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
                content: reported_content,
                painted: frame.painted.clone(),
                floating: frame.floating.clone(),
                drawing: frame.drawing.clone(),
                every_sample: frame.every_sample,
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
            });
        }
        frame
    }

    fn id(&self, region: EditorRegion) -> RegionId {
        RegionId {
            instance: self.instance,
            region,
        }
    }

    pub(crate) fn painting(&mut self, region: EditorRegion) -> Option<paint_format::Painting> {
        let id = self.id(region);
        self.module.with_handler(|handler| handler.painting(id))?
    }

    pub(crate) fn overflows(&mut self, region: EditorRegion) -> Vec<String> {
        let id = self.id(region);
        self.module
            .with_handler(|handler| handler.overflows(id))
            .unwrap_or_default()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn paint(
        &mut self,
        region: EditorRegion,
        target: &PaintTarget<'_>,
    ) -> Vec<block_plugin_api::SurfaceRect> {
        let id = self.id(region);
        self.module
            .with_handler(|handler| handler.paint(id, target))
            .unwrap_or_default()
    }

    pub(crate) fn input(&mut self, region: EditorRegion, event: &TimedInput) {
        self.open(region);
        let context = self.context(region);
        self.module
            .with_handler(|handler| handler.input(&context, event));
    }

    pub(crate) fn node_action(&mut self, region: EditorRegion, request: ActionRequest) {
        self.open(region);
        let context = self.context(region);
        self.module
            .with_handler(|handler| handler.node_action(&context, request));
    }

    pub(crate) fn run_action(&mut self, region: EditorRegion, id: &str) {
        self.open(region);
        let context = self.context(region);
        self.module
            .with_handler(|handler| handler.run_action(&context, id));
    }

    pub fn report(&self, region: EditorRegion) -> Option<&FrameReport> {
        self.regions
            .get(&region)
            .and_then(|state| state.report.as_ref())
    }

    pub fn placed_children(&self, region: EditorRegion) -> &[ChildPlacement] {
        self.regions
            .get(&region)
            .map_or(&[], |state| state.children.as_slice())
    }

    pub fn occluders(&self, region: EditorRegion) -> &[Occluder] {
        self.regions
            .get(&region)
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
