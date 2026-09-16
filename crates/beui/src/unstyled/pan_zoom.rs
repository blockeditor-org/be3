use accesskit::{Node, Role};
use beui_macros::{component, view};

use crate::base::canvas::CanvasView;
use crate::document::Document;
use crate::geometry::{Pos2, Rect, Vec2, pos2};
use crate::input::{CursorIcon, Key, KeyPress, ScrollGesture, ZoomGesture};
use crate::node::NodeId;
use crate::reactive::{
    Callback, ClickCatcher, Focusable, Memo, Prop, ReadSignal, Render, WriteSignal, clone,
    component_accessibility, component_rect, create_effect, create_memo, create_signal,
    set_component_state, untrack,
};

pub const MIN_SCALE: f32 = 0.05;
pub const MAX_SCALE: f32 = 32.0;

const ZOOM_PER_SCROLL_POINT: f32 = 0.004;
const KEY_STEP: f32 = 40.0;
const KEY_ZOOM_STEP: f32 = 1.25;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PanZoomView {
    pub center: Pos2,
    pub scale: f32,
}

impl PanZoomView {
    pub const IDENTITY: Self = Self {
        center: Pos2::ZERO,
        scale: 1.0,
    };

    pub const fn new(center: Pos2, scale: f32) -> Self {
        Self { center, scale }
    }

    pub fn canvas(self, rect: Rect) -> CanvasView {
        let middle = rect.center();
        CanvasView::new(
            pos2(
                middle.x - self.center.x * self.scale,
                middle.y - self.center.y * self.scale,
            ),
            self.scale,
        )
    }

    pub fn to_world(self, rect: Rect, point: Pos2) -> Pos2 {
        self.canvas(rect).to_canvas(point)
    }

    pub fn panned(self, delta: Vec2) -> Self {
        Self {
            center: self.center - delta * self.scale.recip(),
            scale: self.scale,
        }
    }

    pub fn zoomed(self, factor: f32, anchor: Pos2, rect: Rect, limits: (f32, f32)) -> Self {
        let (min, max) = limits;
        let scale = (self.scale * factor).clamp(min.min(max), max.max(min));
        let world = self.to_world(rect, anchor);
        Self {
            center: world - (anchor - rect.center()) * scale.recip(),
            scale,
        }
    }
}

pub struct PanZoomHandle {
    pub view: Memo<Option<CanvasView>>,
    pub scale: Memo<f32>,
    pub panning: ReadSignal<bool>,
    pub focused: ReadSignal<bool>,
}

#[derive(Clone)]
struct Camera {
    view: ReadSignal<PanZoomView>,
    set_view: WriteSignal<PanZoomView>,
    rect: ReadSignal<Rect>,
    min_scale: Memo<f32>,
    max_scale: Memo<f32>,
    on_change: Callback<PanZoomView>,
}

impl Camera {
    fn apply(&self, next: PanZoomView) {
        if untrack(|| self.view.get()) == next {
            return;
        }
        self.set_view.set(next);
        self.on_change.call(next);
    }

    fn pan(&self, delta: Vec2) {
        self.apply(untrack(|| self.view.get()).panned(delta));
    }

    fn key(&self, press: KeyPress) -> bool {
        if press.modifiers.alt {
            return false;
        }
        let pan = match press.key {
            Key::ArrowLeft => Some(Vec2::new(KEY_STEP, 0.0)),
            Key::ArrowRight => Some(Vec2::new(-KEY_STEP, 0.0)),
            Key::ArrowUp => Some(Vec2::new(0.0, KEY_STEP)),
            Key::ArrowDown => Some(Vec2::new(0.0, -KEY_STEP)),
            _ => None,
        };
        let zoom = match press.key {
            Key::Plus => Some(KEY_ZOOM_STEP),
            Key::Minus => Some(KEY_ZOOM_STEP.recip()),
            _ => None,
        };
        if pan.is_none() && zoom.is_none() && press.key != Key::Zero {
            return false;
        }
        if !press.pressed {
            return true;
        }
        let middle = untrack(|| self.rect.get()).center();
        match (pan, zoom) {
            (Some(delta), _) => self.pan(delta),
            (_, Some(factor)) => self.zoom(factor, middle),
            _ => self.apply(PanZoomView {
                scale: 1.0,
                ..untrack(|| self.view.get())
            }),
        }
        true
    }

    fn zoom(&self, factor: f32, anchor: Pos2) {
        let (view, rect, limits) = untrack(|| {
            (
                self.view.get(),
                self.rect.get(),
                (self.min_scale.get(), self.max_scale.get()),
            )
        });
        self.apply(view.zoomed(factor, anchor, rect, limits));
    }
}

#[component]
pub fn PanZoom(
    view: Prop<PanZoomView>,
    on_change: Callback<PanZoomView>,
    #[prop(default = MIN_SCALE)] min_scale: Prop<f32>,
    #[prop(default = MAX_SCALE)] max_scale: Prop<f32>,
    accessibility: Option<Prop<Node>>,
    #[prop(children)] content: Render<PanZoomHandle>,
) -> NodeId {
    let rect = component_rect();
    let (current, set_current) = create_signal(view.peek());
    create_effect(clone!(set_current -> move || set_current.set(view.get())));
    let (panning, set_panning) = create_signal(false);

    let camera = Camera {
        view: current.clone(),
        set_view: set_current,
        rect: rect.clone(),
        min_scale: create_memo(move || min_scale.get()),
        max_scale: create_memo(move || max_scale.get()),
        on_change,
    };
    let scroll_camera = camera.clone();
    let zoom_camera = camera.clone();

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::ScrollView)));
    component_accessibility(create_memo(move || accessibility.get()));

    let (focused, set_focused) = create_signal(false);
    let key_camera = camera.clone();
    let cursor = create_memo(clone!(panning -> move || if panning.get() {
        CursorIcon::Grabbing
    } else {
        CursorIcon::Default
    }));
    let content_node = content.call(PanZoomHandle {
        view: create_memo(clone!(current rect -> move || Some(current.get().canvas(rect.get())))),
        scale: create_memo(clone!(current -> move || current.get().scale)),
        panning: panning.clone(),
        focused: focused.clone(),
    });

    set_component_state(current);

    view! {
        <Focusable
            on_focus_change={move |focused: bool| set_focused.set(focused)}
            on_key={move |press: KeyPress| key_camera.key(press)}
        >
            <ClickCatcher
                cursor={cursor}
                on_scroll={move |gesture: ScrollGesture| {
                    if gesture.modifiers.ctrl {
                        let factor = (gesture.delta.y * ZOOM_PER_SCROLL_POINT).exp();
                        scroll_camera.zoom(factor, gesture.pos);
                    } else {
                        scroll_camera.pan(gesture.delta);
                    }
                }}
                on_zoom={move |gesture: ZoomGesture| zoom_camera.zoom(gesture.factor, gesture.pos)}
                on_pan_drag={move |delta: Vec2| camera.pan(delta)}
                on_pan_active_change={move |panning: bool| set_panning.set(panning)}
                children={Some(content_node)}
            />
        </Focusable>
    }
}

pub fn pan_zoom_view(document: &Document, pan_zoom: NodeId) -> ReadSignal<PanZoomView> {
    document
        .component_state::<ReadSignal<PanZoomView>>(pan_zoom)
        .clone()
}
