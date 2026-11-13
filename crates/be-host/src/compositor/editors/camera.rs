use std::cell::RefCell;
use std::collections::HashMap;

use be_sdk_api::{EditorInstanceId, FitArea, PanMode, ViewChange, ViewId};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Pos2, Rect, ScrollGesture, Vec2, ZoomGesture, vec2};
use uuid::Uuid;

use super::super::app_view::PanDrag;
use be_host_runtime::EditorView;

type Key = (String, EditorInstanceId);

/// The view changes instances asked for, until the camera that holds an instance's content
/// takes them: it may not be shown yet when its instance asks. An instance that never shows
/// one (a window, a creation) keeps only its latest few, and none once it closes.
#[derive(Default)]
pub(super) struct HeldChanges {
    held: HashMap<Key, Vec<ViewChange>>,
}

impl HeldChanges {
    /// How many changes an instance keeps while no camera takes them.
    const KEPT: usize = 16;

    pub(super) fn hold(&mut self, key: Key, change: ViewChange) {
        let held = self.held.entry(key).or_default();
        held.push(change);
        if held.len() > Self::KEPT {
            held.remove(0);
        }
    }

    pub(super) fn take(&mut self, key: &Key) -> Vec<ViewChange> {
        self.held.remove(key).unwrap_or_default()
    }

    pub(super) fn forget(&mut self, key: &Key) {
        self.held.remove(key);
    }
}

thread_local! {
    static HELD: RefCell<HeldChanges> = RefCell::new(HeldChanges::default());
    /// The cameras showing each instance, woken when it asks for a change.
    static CAMERAS: RefCell<HashMap<Key, Vec<(u64, Signal<u64>)>>> = RefCell::new(HashMap::new());
    static NEXT_CAMERA: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Keeps a view change `instance` asked for until its camera takes it.
pub(crate) fn heard_view_change(app_id: String, instance: EditorInstanceId, change: ViewChange) {
    let key = (app_id, instance);
    let cameras = CAMERAS.with_borrow(|cameras| cameras.get(&key).cloned().unwrap_or_default());
    HELD.with_borrow_mut(|held| held.hold(key, change));
    for (_, changed) in cameras {
        changed.update(|changed| *changed += 1);
    }
}

/// The instance closed: the changes it asked for go with it.
pub(crate) fn closed(app_id: String, instance: EditorInstanceId) {
    HELD.with_borrow_mut(|held| held.forget(&(app_id, instance)));
}

/// A signal that moves when the instance asks for a change, while the calling scope lives.
fn view_changed(app_id: &str, instance: EditorInstanceId) -> ReadSignal<u64> {
    let changed = create_signal(0u64);
    let id = NEXT_CAMERA.with(|next| {
        next.set(next.get() + 1);
        next.get()
    });
    let key = (app_id.to_owned(), instance);
    CAMERAS.with_borrow_mut(|cameras| cameras.entry(key.clone()).or_default().push((id, changed)));
    on_cleanup(move || {
        CAMERAS.with_borrow_mut(|cameras| {
            if let Some(held) = cameras.get_mut(&key) {
                held.retain(|(camera, _)| *camera != id);
                if held.is_empty() {
                    cameras.remove(&key);
                }
            }
        });
    });
    changed.read_only()
}

fn take_view_changes(app_id: &str, instance: EditorInstanceId) -> Vec<ViewChange> {
    HELD.with_borrow_mut(|held| held.take(&(app_id.to_owned(), instance)))
}

const MIN_ZOOM: f32 = 1.0 / 64.0;
pub(super) const MAX_ZOOM: f32 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Viewport {
    zoom: f32,
    pan: Vec2,
    moved: bool,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            moved: false,
        }
    }
}

impl Viewport {
    fn shown(self, area: Rect, content: Vec2) -> Rect {
        Rect::from_center_size(area.center() + self.pan, content * self.zoom)
    }
}

thread_local! {
    static VIEWPORTS: RefCell<HashMap<Uuid, Viewport>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Copy)]
struct CameraOwner(Callback<Command>);

#[derive(Clone, Copy)]
enum Command {
    Pan(Vec2),
    Zoom { factor: f32, anchor: Option<Pos2> },
}

fn settle(viewport: &mut Viewport, command: Command, rect: Rect, max_zoom: f32) {
    match command {
        Command::Pan(delta) => {
            viewport.pan += delta;
            viewport.moved = true;
        }
        Command::Zoom { factor, anchor } => {
            zoom(
                viewport,
                factor,
                anchor.unwrap_or_else(|| rect.center()),
                rect,
                max_zoom,
            );
            viewport.moved = true;
        }
    }
}

fn follow(
    viewport: &mut Viewport,
    area: Option<FitArea>,
    rect: Rect,
    content: Vec2,
    max_zoom: f32,
) {
    let Some(area) = area.filter(|_| !viewport.moved) else {
        return;
    };
    let (scale, centre) = area.fit(content, rect.size());
    viewport.zoom = scale.clamp(MIN_ZOOM, max_zoom);
    viewport.pan = (content * 0.5 - centre.to_vec2()) * viewport.zoom;
}

fn zoom(viewport: &mut Viewport, factor: f32, anchor: Pos2, rect: Rect, max_zoom: f32) {
    let old = viewport.zoom;
    let new = (old * factor).clamp(MIN_ZOOM, max_zoom);
    if new != old {
        let offset = anchor - rect.center();
        viewport.pan = offset - (offset - viewport.pan) * (new / old);
        viewport.zoom = new;
    }
}

#[derive(Clone, Copy)]
pub(super) struct CameraSpec {
    pub(super) block: Uuid,
    pub(super) app_id: Stored<String>,
    pub(super) instance: EditorInstanceId,
    pub(super) view: ViewId,
    pub(super) pan_and_zoom: bool,
    pub(super) max_zoom: Option<u32>,
    pub(super) pan: PanMode,
    pub(super) placed: bool,
    pub(super) read_only: bool,
    pub(super) revision: ReadSignal<u64>,
}

pub(super) struct CameraView {
    pub(super) view: Memo<Option<EditorView>>,
    pub(super) pan_drag: Option<PanDrag>,
    pub(super) rect: ReadSignal<Rect>,
}

#[component]
pub(super) fn Camera(
    spec: CameraSpec,
    #[prop(default = None)] content: Prop<Option<Rect>>,
    #[prop(default = true)] fits: Prop<bool>,
    #[prop(children)] region: RenderFn<CameraView, NodeId>,
) -> NodeId {
    let CameraSpec {
        block,
        app_id,
        instance,
        view: shown_view,
        placed,
        read_only,
        revision,
        ..
    } = spec;
    let rect_held = create_signal(Rect::ZERO);
    let rect = rect_held.read_only();
    let holds = create_memo(move || {
        revision.get();
        match be_host_runtime::frame_rects(&app_id.value(), instance, shown_view) {
            Some(frame) => frame.holds_content(),
            None => shown_view == ViewId::MAIN,
        }
    });
    let zoomable = spec.pan_and_zoom && !placed;
    let owns = move || zoomable && holds.get_untracked();
    let max_zoom = spec.max_zoom.map_or(MAX_ZOOM, |zoom| zoom as f32);
    let viewport = create_signal(
        VIEWPORTS
            .with(|viewports| viewports.borrow().get(&block).copied())
            .unwrap_or_default(),
    );
    create_effect(move || {
        let state = viewport.get();
        VIEWPORTS.with(|viewports| viewports.borrow_mut().insert(block, state));
    });
    let viewport_rect = create_memo(move || {
        revision.get();
        let rect = rect.get();
        be_host_runtime::frame_rects(&app_id.value(), instance, shown_view)
            .map(|frame| frame.content(rect))
            .filter(Rect::is_positive)
            .unwrap_or(rect)
    });
    let content_size = create_memo(move || {
        revision.get();
        let size = viewport_rect.get().size().max(Vec2::splat(1.0));
        let intrinsic = be_host_runtime::intrinsic_size(&app_id.value(), instance)
            .unwrap_or(vec2(420.0, 240.0));
        vec2(size.x.max(intrinsic.x), size.y.max(intrinsic.y))
    });
    let content_rect = create_memo(move || {
        viewport
            .get()
            .shown(viewport_rect.get(), content_size.get())
    });
    let shown = create_memo(move || {
        if !spec.pan_and_zoom || !holds.get() {
            return None;
        }
        let origin = rect.get().min;
        if !placed {
            let state = viewport.get();
            return Some(EditorView {
                rect: content_rect.get().translate(-origin.to_vec2()),
                scale: state.zoom,
            });
        }
        revision.get();
        let shown = content.get().unwrap_or(rect.get());
        let scale = be_host_runtime::intrinsic_size(&app_id.value(), instance)
            .filter(|intrinsic| intrinsic.x > 0.0 && intrinsic.y > 0.0)
            .map_or(1.0, |intrinsic| {
                (shown.width() / intrinsic.x).min(shown.height() / intrinsic.y)
            });
        Some(EditorView {
            rect: shown.translate(-origin.to_vec2()),
            scale,
        })
    });
    let fitted = create_signal(None::<FitArea>);
    let fit = move || {
        if !owns() {
            return;
        }
        let mut state = viewport.get_untracked();
        state.moved = false;
        follow(
            &mut state,
            fitted.get_untracked(),
            viewport_rect.get_untracked(),
            content_size.get_untracked(),
            max_zoom,
        );
        if state != viewport.get_untracked() {
            viewport.set(state);
        }
    };
    let owner = use_context::<CameraOwner>();
    let command = move |command: Command| {
        if !owns() {
            if let (Some(owner), true) = (owner, spec.pan_and_zoom) {
                owner.0.call(command);
            }
            return;
        }
        let mut state = viewport.get_untracked();
        settle(&mut state, command, viewport_rect.get_untracked(), max_zoom);
        if state != viewport.get_untracked() {
            viewport.set(state);
        }
    };
    if zoomable {
        provide_context(CameraOwner(Callback::new(command)));
    }
    create_effect(move || {
        if !zoomable || !holds.get() {
            return;
        }
        let rect = viewport_rect.get();
        let content = content_size.get();
        let area = fitted.get().filter(|_| fits.get());
        let mut state = viewport.get_untracked();
        follow(&mut state, area, rect, content, max_zoom);
        if state != viewport.get_untracked() {
            viewport.set(state);
        }
    });
    let pan_drags = create_signal(0u64);
    let changed = view_changed(&app_id.value(), instance);
    create_effect(move || {
        changed.get();
        if !holds.get() {
            return;
        }
        let rect = rect.get_untracked();
        for change in take_view_changes(&app_id.value(), instance) {
            command(match change {
                ViewChange::Pan(by) => Command::Pan(by),
                ViewChange::Zoom { factor, anchor } => Command::Zoom {
                    factor,
                    anchor: anchor.map(|at| rect.min + at.to_vec2()),
                },
                ViewChange::KeepFitted(area) => {
                    fitted.set(area);
                    continue;
                }
                ViewChange::Fit => {
                    fit();
                    continue;
                }
                ViewChange::PanDrag => {
                    pan_drags.update(|asked| *asked += 1);
                    continue;
                }
            });
        }
    });
    let gestures = spec.pan != PanMode::None && (zoomable || owner.is_some() && spec.pan_and_zoom);
    let pan_drag = (gestures && spec.pan == PanMode::OneFinger).then(|| PanDrag {
        asked: pan_drags.read_only(),
        pan: Callback::new(move |delta: Vec2| command(Command::Pan(delta))),
    });
    let steered =
        create_memo(move || (!read_only && !spec.pan_and_zoom).then(|| content_rect.get()));
    let scroll = move |gesture: ScrollGesture| {
        if !gestures
            || steered
                .get_untracked()
                .is_some_and(|rect| rect.contains(gesture.pos))
        {
            return;
        }
        match gesture.modifiers.ctrl && gesture.delta.y != 0.0 {
            true => command(Command::Zoom {
                factor: (gesture.delta.y * 0.002).exp(),
                anchor: Some(gesture.pos),
            }),
            false => command(Command::Pan(gesture.delta)),
        }
    };
    let zoom = move |gesture: ZoomGesture| {
        if gestures {
            command(Command::Zoom {
                factor: gesture.factor,
                anchor: Some(gesture.pos),
            });
        }
    };
    let pan = move |delta: Vec2| {
        if gestures {
            command(Command::Pan(delta));
        }
    };
    let gesture_area = create_memo(move || {
        if !gestures || !holds.get() {
            return beui::Area::Nowhere;
        }
        revision.get();
        let origin = rect.get().min.to_vec2();
        let mut area = beui::HitRegion::rect(viewport_rect.get().translate(-origin));
        if let Some(steered) = steered.get() {
            area = area.without(steered.translate(-origin));
        }
        if let Some(frame) = be_host_runtime::frame_rects(&app_id.value(), instance, shown_view) {
            for floating in frame.floating {
                area = area.without(floating);
            }
        }
        beui::Area::Within(area)
    });
    let recognised = beui::unstyled::gestures::pan_zoom(pan, zoom);
    let shown = region.call(CameraView {
        view: shown,
        pan_drag,
        rect,
    });
    let built = view! {
        <Interactive
            on_wheel={beui::unstyled::gestures::first_of([
                recognised.wheel,
                beui::unstyled::gestures::scroll_wheel(None, scroll),
            ])}
            on_pointer={recognised.pointer}
            gesture_area={gesture_area}
        >
            {shown}
        </Interactive>
    };
    let measured = measure(built);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            rect_held.set(measured.rect);
        }
    });
    built
}

#[cfg(test)]
mod tests;
