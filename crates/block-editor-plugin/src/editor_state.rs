use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use be_block::presence::{ChannelKind, PresenceKind};
use be_block::{EditorViewContent, ViewState};
use block_plugin_api::{BarAction, ChildContent, FitArea, HostPanel, HostWindowId, WebViewId};
use geometry::{CanvasView, Pos2, Rect, Vec2};
use uuid::Uuid;

use crate::state::{Callbacks, Subscription};
use crate::{BlockFilter, BlockGraph, BlockParent, EditorHost, FileDrop, LiveContent, Waker};

mod artifacts;
mod children;
mod creation;
mod fitting;

pub use artifacts::ArtifactsCore;
pub use children::{ChildState, PlacedChild};
pub use creation::CreationCore;
pub use fitting::Fit;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointScale {
    pub ratio: f32,
    pub pixels_per_point: f32,
}

impl Default for PointScale {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            pixels_per_point: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub position: Pos2,
    pub block_id: Uuid,
    pub block_type: Uuid,
    pub dropped: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChildTarget {
    pub id: Uuid,
    pub block_type: Uuid,
    pub view_block: Option<Uuid>,
    pub editor: Option<Uuid>,
}

impl ChildTarget {
    pub fn new(id: Uuid, block_type: Uuid) -> Self {
        Self {
            id,
            block_type,
            view_block: None,
            editor: None,
        }
    }

    pub fn edited_with(mut self, editor: Option<Uuid>) -> Self {
        self.editor = editor;
        self
    }

    pub fn viewed_by(mut self, view_block: Uuid) -> Self {
        self.view_block = Some(view_block);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubregionContent {
    Block(ChildTarget),
    WebView(WebViewId),
    Host(HostPanel),
    Window(HostWindowId),
    Creation {
        editor: Uuid,
        template: String,
    },
    ArtifactSettings {
        block: Uuid,
    },
    Picker {
        filter: BlockFilter,
        parent: BlockParent,
    },
    DockTab(block_plugin_api::DockTabPath),
    Rename(Uuid),
}

impl From<ChildTarget> for SubregionContent {
    fn from(target: ChildTarget) -> Self {
        Self::Block(target)
    }
}

impl SubregionContent {
    pub fn wire(&self) -> ChildContent {
        match self {
            Self::Block(target) => ChildContent::Block {
                block_id: target.id.into_bytes(),
                block_type: target.block_type.into_bytes(),
                view_block: target.view_block.map(Uuid::into_bytes),
                editor: target.editor.map(Uuid::into_bytes),
            },
            Self::WebView(web_view) => ChildContent::WebView(*web_view),
            Self::Host(panel) => ChildContent::Host(*panel),
            Self::Window(window) => ChildContent::Window(*window),
            Self::Creation { editor, template } => ChildContent::Creation {
                editor: editor.into_bytes(),
                template: template.clone(),
            },
            Self::ArtifactSettings { block } => ChildContent::ArtifactSettings {
                block_id: block.into_bytes(),
            },
            Self::Picker { filter, parent } => ChildContent::Picker {
                filter: filter.clone(),
                parent: parent.encode(),
            },
            Self::DockTab(tab) => ChildContent::DockTab(tab.clone()),
            Self::Rename(block) => ChildContent::Rename(block.into_bytes()),
        }
    }
}

pub fn fit_content(available: Rect, content: Vec2) -> Rect {
    if content.x <= 0.0 || content.y <= 0.0 {
        return Rect::ZERO;
    }
    let scale = (available.width() / content.x)
        .min(available.height() / content.y)
        .max(f32::EPSILON);
    let size = Vec2::new(content.x * scale, content.y * scale);
    Rect::from_min_size(
        Pos2::new(
            available.center().x - size.x / 2.0,
            available.center().y - size.y / 2.0,
        ),
        size,
    )
}

pub fn short_id(id: &[u8]) -> String {
    id.iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Clone, PartialEq)]
pub struct ShownEditor {
    pub files: Option<FileDrop>,
    pub chrome: bool,
    pub placed_content: Option<Rect>,
    pub frame_anchor: Option<Rect>,
    pub editable: bool,
    pub presenting: bool,
    pub drag: Option<Drag>,
    pub placed: Rect,
    pub canvas: Option<CanvasView>,
    pub world: Option<Vec2>,
    pub scale: f32,
    pub pixels_per_point: f32,
    pub resized: Option<Vec2>,
    pub presence_visible: bool,
    pub revealed: Option<u64>,
}

impl ShownEditor {
    fn of(host: &EditorHost) -> Self {
        Self {
            files: None,
            chrome: true,
            placed_content: None,
            frame_anchor: None,
            editable: host.editable(),
            presenting: false,
            drag: None,
            placed: Rect::ZERO,
            canvas: None,
            world: None,
            scale: 1.0,
            pixels_per_point: 1.0,
            resized: None,
            presence_visible: false,
            revealed: None,
        }
    }
}

type Pump = Rc<dyn Fn()>;

#[derive(Default)]
pub struct ContentCache {
    contents: RefCell<HashMap<Option<Uuid>, Box<dyn Any>>>,
    pumps: RefCell<HashMap<Option<Uuid>, Pump>>,
    due: Rc<RefCell<Vec<Option<Uuid>>>>,
}

type ReplaceChild = Rc<dyn Fn(Uuid, Uuid) -> bool>;
type Wake = (u64, Arc<AtomicU64>, Rc<dyn Fn(u64)>);

#[derive(Clone)]
pub struct EditorCore(Rc<Inner>);

struct Inner {
    host: EditorHost,
    block: Uuid,
    scale: Rc<Cell<PointScale>>,
    contents: Rc<ContentCache>,
    replace: RefCell<Option<ReplaceChild>>,
    content_rect: Cell<Rect>,
    intrinsic: Cell<Option<Vec2>>,
    pending_resize: Cell<Option<Vec2>>,
    pending_presence: Cell<Option<bool>>,
    pending_reveal: Cell<Option<u64>>,
    seen: Cell<Option<(u64, f32)>>,
    shown_rect: Cell<Option<Rect>>,
    shown: RefCell<ShownEditor>,
    on_shown: Callbacks<(), dyn Fn(&ShownEditor)>,
    children: children::Children,
    wakes: Rc<RefCell<Vec<Wake>>>,
    next_wake: Cell<u64>,
}

impl EditorCore {
    pub fn new(host: EditorHost, block: Uuid) -> Self {
        Self::scaled(host, block, Rc::default(), Rc::default())
    }

    pub fn scaled(
        host: EditorHost,
        block: Uuid,
        scale: Rc<Cell<PointScale>>,
        contents: Rc<ContentCache>,
    ) -> Self {
        Self(Rc::new(Inner {
            shown: RefCell::new(ShownEditor::of(&host)),
            host,
            block,
            scale,
            contents,
            replace: RefCell::new(None),
            content_rect: Cell::new(Rect::ZERO),
            intrinsic: Cell::new(None),
            pending_resize: Cell::new(None),
            pending_presence: Cell::new(None),
            pending_reveal: Cell::new(None),
            seen: Cell::new(None),
            shown_rect: Cell::new(None),
            on_shown: Callbacks::default(),
            children: children::Children::default(),
            wakes: Rc::default(),
            next_wake: Cell::new(0),
        }))
    }

    pub fn host(&self) -> &EditorHost {
        &self.0.host
    }

    pub fn graph(&self) -> BlockGraph {
        self.0.host.graph()
    }

    pub fn block_id(&self) -> Uuid {
        self.0.block
    }

    pub fn contents(&self) -> Rc<ContentCache> {
        Rc::clone(&self.0.contents)
    }

    pub fn scale(&self) -> PointScale {
        self.0.scale.get()
    }

    pub fn ratio(&self) -> f32 {
        self.0.scale.get().ratio
    }

    pub fn shown(&self) -> ShownEditor {
        self.0.shown.borrow().clone()
    }

    pub fn on_shown(&self, run: impl Fn(&ShownEditor) + 'static) -> Subscription {
        self.0.on_shown.add((), Rc::new(run))
    }

    pub fn content<C>(&self, block: Option<Uuid>) -> LiveContent<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        let contents = &self.0.contents;
        let cached = contents
            .contents
            .borrow()
            .get(&block)
            .and_then(|held| held.downcast_ref::<LiveContent<C>>())
            .cloned();
        if let Some(live) = cached {
            return live;
        }
        let live = LiveContent::<C>::new(self.0.host.clone(), block);
        let due = Rc::downgrade(&contents.due);
        live.on_due(move || {
            if let Some(due) = due.upgrade() {
                due.borrow_mut().push(block);
            }
        });
        let pumped = live.clone();
        contents
            .pumps
            .borrow_mut()
            .insert(block, Rc::new(move || pumped.pump()));
        contents.due.borrow_mut().push(block);
        contents
            .contents
            .borrow_mut()
            .insert(block, Box::new(live.clone()));
        live
    }

    pub fn content_of<C>(&self, block: Uuid) -> LiveContent<C>
    where
        C: be_block::LiveEdit + Clone + Default,
    {
        if block == self.0.block {
            return self.content(None);
        }
        self.0.host.watch_content(block, C::CONTENT_TYPE);
        self.content(Some(block))
    }

    pub fn view_content(&self) -> Option<LiveContent<EditorViewContent>> {
        self.0
            .host
            .view_block()
            .map(|block| self.content_of::<EditorViewContent>(block))
    }

    pub fn view_state(&self, key: &str) -> Option<ViewState> {
        self.view_content()?
            .read(|view| view.root().state(key).cloned())
            .ok()
            .flatten()
    }

    pub fn set_view_state(&self, key: &str, state: Option<&ViewState>) {
        let Some(view) = self.view_content() else {
            return;
        };
        let client = self.0.host.client_id();
        let now = now_milliseconds();
        let edit = view.read(|held| {
            let root = held.root();
            (root.state(key) != state).then(|| root.set_state(key, state, now, client))
        });
        if let Some(edit) = edit.ok().flatten() {
            view.operate_anyway(edit);
        }
    }

    pub fn create_child<C: be_block::BlockContent>(&self, content: &C) -> Uuid {
        self.graph()
            .create(content, BlockParent::Block(self.0.block))
    }

    pub fn version_status(&self) -> Result<crate::VersionStatus, crate::Unready> {
        match self.0.host.version_known() {
            true => Ok(self.0.host.version_status()),
            false => Err(crate::Unready::Loading),
        }
    }

    pub fn version(&self, command: crate::VersionCommand) {
        self.0.host.version(self.0.block, command);
    }

    pub fn woken(&self, run: impl Fn(u64) + 'static) -> (Waker, Subscription) {
        let count = Arc::new(AtomicU64::new(0));
        let id = self.0.next_wake.get();
        self.0.next_wake.set(id + 1);
        self.0
            .wakes
            .borrow_mut()
            .push((id, Arc::clone(&count), Rc::new(run)));
        let wakes = Rc::downgrade(&self.0.wakes);
        let unsubscribe = Subscription::new(move || {
            if let Some(wakes) = wakes.upgrade() {
                wakes.borrow_mut().retain(|(held, _, _)| *held != id);
            }
        });
        (self.0.host.waker().counting(count), unsubscribe)
    }

    pub fn new_peers<P: PresenceKind>(&self, seen: &Cell<u64>) -> Option<Vec<(u64, P)>> {
        let (latest, held) = self.0.host.peers_since(None, seen.get())?;
        seen.set(latest);
        Some(
            held.iter()
                .filter(|peer| peer.kind == P::ID)
                .filter_map(|peer| {
                    serde_json::from_slice(&peer.value)
                        .ok()
                        .map(|value| (peer.client, value))
                })
                .collect(),
        )
    }

    pub fn new_peer_messages<M: ChannelKind>(&self, seen: &Cell<u64>) -> Vec<(u64, M)> {
        let received = self.0.host.peer_messages_since(None, seen.get());
        let Some((last, _)) = received.last() else {
            return Vec::new();
        };
        seen.set(*last);
        received
            .into_iter()
            .filter(|(_, message)| message.channel == M::ID)
            .filter_map(|(_, message)| {
                M::decode(&message.payload).map(|decoded| (message.from, decoded))
            })
            .collect()
    }

    pub fn show<P: PresenceKind>(&self, value: Option<&P>) {
        let value = value.and_then(|value| serde_json::to_vec(value).ok());
        self.0.host.show_presence(None, P::ID, value);
    }

    pub fn send_to_peers<M: ChannelKind>(&self, to: Option<u64>, message: &M) {
        self.0.host.send_to_peers(None, to, M::ID, message.encode());
    }

    pub fn set_intrinsic_size(&self, size: Option<Vec2>) {
        self.0.intrinsic.set(size);
    }

    pub fn intrinsic_size(&self) -> Option<Vec2> {
        self.0.intrinsic.get()
    }

    pub fn report_resize(&self, size: Vec2) {
        self.0.pending_resize.set(Some(size));
    }

    pub fn report_presence_visible(&self, visible: bool) {
        self.0.pending_presence.set(Some(visible));
    }

    pub fn report_reveal(&self, client_id: u64) {
        self.0.pending_reveal.set(Some(client_id));
    }

    pub fn on_replace_child(&self, replace: impl Fn(Uuid, Uuid) -> bool + 'static) {
        *self.0.replace.borrow_mut() = Some(Rc::new(replace));
    }

    pub fn replace_child(&self, old: Uuid, new: Uuid) -> bool {
        let replace = self.0.replace.borrow().clone();
        replace.is_some_and(|replace| replace(old, new))
    }

    pub fn content_rect(&self) -> Rect {
        self.0.content_rect.get()
    }

    pub fn to_host(&self, rect: Rect) -> Rect {
        rect.scaled(self.ratio().recip())
    }

    pub fn occlude(&self, rect: Rect) {
        self.0.host.occlude(self.to_host(rect));
    }

    pub fn report_content(&self, rect: Rect) {
        self.0.content_rect.set(rect);
        if self.0.shown.borrow().placed != rect {
            self.0.host.request_frame_in(std::time::Duration::ZERO);
        }
        self.0.host.report_content(self.to_host(rect));
    }

    fn view_rect(&self) -> Option<Rect> {
        let ratio = self.ratio();
        self.0.host.view().map(|rect| rect.scaled(ratio))
    }

    fn view_scale(&self) -> f32 {
        self.0.host.view_scale().unwrap_or(1.0)
    }

    pub fn canvas(&self) -> Option<CanvasView> {
        let scale = self.view_scale();
        self.view_rect()
            .map(|rect| CanvasView::new(rect.min, scale))
    }

    pub fn pan(&self, delta: Vec2) {
        self.0.host.pan_view(delta / self.ratio());
    }

    pub fn zoom(&self, factor: f32) {
        self.0.host.zoom_view(factor, None);
    }

    pub fn zoom_at(&self, factor: f32, anchor: Pos2) {
        let ratio = self.ratio();
        self.0
            .host
            .zoom_view(factor, Some(Pos2::new(anchor.x / ratio, anchor.y / ratio)));
    }

    pub fn pan_drag(&self) {
        self.0.host.pan_drag_view();
    }

    pub fn keep_fitted(&self, area: Option<FitArea>) {
        self.0.host.keep_fitted(area);
    }

    pub fn fit_view(&self) {
        self.0.host.fit_view();
    }

    pub fn reveal(&self, target: Rect) {
        let Some(placement) = self.canvas() else {
            return;
        };
        let shown = placement.rect_to_screen(target).center();
        self.pan(self.0.content_rect.get().center() - shown);
    }

    pub fn begin_frame(&self) {
        let inner = &self.0;
        let ratio = self.ratio();
        let seen = Some((inner.host.changes(), ratio));
        let pushed = inner.seen.replace(seen) != seen;
        if pushed {
            inner.host.flush_graph();
        }
        let mut due = std::mem::take(&mut *inner.contents.due.borrow_mut());
        if pushed {
            due.extend(inner.host.updated_content());
        }
        self.pump(due);
        let wakes = inner.wakes.borrow().clone();
        for (_, count, run) in wakes {
            run(count.load(Ordering::Acquire));
        }
        let mut shown = inner.shown.borrow().clone();
        let host = &inner.host;
        if pushed {
            shown.files = host.files().map(|files| FileDrop {
                position: Pos2::new(files.position.x * ratio, files.position.y * ratio),
                ..files
            });
            shown.chrome = host.chrome_shown();
            shown.placed_content = host.placed_content().map(|rect| rect.scaled(ratio));
            shown.frame_anchor = host.frame_anchor().map(|rect| rect.scaled(ratio));
            shown.editable = host.editable();
            shown.presenting = host.presenting();
            shown.drag = host.drag().map(|drag| Drag {
                position: Pos2::new(drag.position.x * ratio, drag.position.y * ratio),
                block_id: drag.block_id,
                block_type: drag.block_type,
                dropped: drag.dropped,
            });
        }
        let rect = inner.content_rect.get();
        if pushed || inner.shown_rect.replace(Some(rect)) != Some(rect) {
            shown.placed = rect;
            let scale = self.view_scale();
            let divisor = scale.max(f32::EPSILON);
            shown.canvas = self.canvas();
            shown.world = self
                .view_rect()
                .map(|rect| Vec2::new(rect.width() / divisor, rect.height() / divisor));
            shown.scale = scale;
        }
        shown.pixels_per_point = inner.scale.get().pixels_per_point;
        shown.resized = inner.pending_resize.take();
        if let Some(visible) = inner.pending_presence.take() {
            shown.presence_visible = visible;
        }
        shown.revealed = inner.pending_reveal.take();
        if *inner.shown.borrow() != shown {
            *inner.shown.borrow_mut() = shown.clone();
            for run in inner.on_shown.all() {
                run(&shown);
            }
        }
        if pushed || inner.children.take_moved() {
            inner.children.follow(host);
        }
    }

    fn pump(&self, blocks: Vec<Option<Uuid>>) {
        let pumps: Vec<_> = {
            let held = self.0.contents.pumps.borrow();
            let mut pumped = HashSet::new();
            blocks
                .into_iter()
                .filter(|block| pumped.insert(*block))
                .filter_map(|block| held.get(&block).cloned())
                .collect()
        };
        for pump in pumps {
            pump();
        }
    }

    pub fn add_child(
        &self,
        on_state: impl Fn(&ChildState) + 'static,
        on_bar: impl Fn(BarAction) + 'static,
        on_focus: impl Fn() + 'static,
    ) -> PlacedChild {
        self.0.children.add(on_state, on_bar, on_focus)
    }
}

fn now_milliseconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
