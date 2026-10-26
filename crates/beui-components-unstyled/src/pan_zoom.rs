use crate::gestures;
use accesskit::{Node, Role};
use beui_macros::{component, view};

use beui_view::primitives::*;
use beui_view::reactive::CanvasView;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{CursorIcon, Key, KeyPress, ScrollGesture, ZoomGesture};
use beui_view::types::{Pos2, Rect, Vec2, pos2};

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

#[derive(Clone, Copy)]
struct Camera {
    view: Binding<PanZoomView>,
    rect: ReadSignal<Rect>,
    min_scale: Memo<f32>,
    max_scale: Memo<f32>,
}

impl Camera {
    fn apply(&self, next: PanZoomView) {
        self.view.set(next);
    }

    fn pan(&self, delta: Vec2) {
        self.apply(self.view.get_untracked().panned(delta));
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
        let middle = self.rect.get_untracked().center();
        match (pan, zoom) {
            (Some(delta), _) => self.pan(delta),
            (_, Some(factor)) => self.zoom(factor, middle),
            _ => self.apply(PanZoomView {
                scale: 1.0,
                ..self.view.get_untracked()
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
    #[prop(default = ComponentRef::new())] api: ComponentRef<PanZoomApi>,
    #[prop(default = PanZoomView::IDENTITY)] view: Binding<PanZoomView>,
    #[prop(default = MIN_SCALE)] min_scale: Prop<f32>,
    #[prop(default = MAX_SCALE)] max_scale: Prop<f32>,
    accessibility: Option<Prop<Node>>,
    #[prop(children)] content: Render<PanZoomHandle>,
) -> NodeId {
    let rect_held = create_signal(Rect::ZERO);
    let rect = rect_held.read_only();
    let current = view;

    let camera = Camera {
        view: current,
        rect,
        min_scale: create_memo(move || min_scale.get()),
        max_scale: create_memo(move || max_scale.get()),
    };

    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(Role::ScrollView)));
    component_accessibility(create_memo(move || accessibility.get()));

    let focused = create_signal(false);
    let recognised = gestures::pan_zoom(
        move |delta: Vec2| camera.pan(delta),
        move |gesture: ZoomGesture| camera.zoom(gesture.factor, gesture.pos),
    );
    let panning = recognised.panning;
    let cursor = create_memo(move || {
        if panning.get() {
            CursorIcon::Grabbing
        } else {
            CursorIcon::Default
        }
    });
    let content_node = content.call(PanZoomHandle {
        view: create_memo(move || Some(current.get().canvas(rect.get()))),
        scale: create_memo(move || current.get().scale),
        panning,
        focused: focused.read_only(),
    });

    api.fill(PanZoomApi(current));

    let built = view! {
        <Interactive
            focusable=true
            on_focus_change={focused}
            on_key={move |press: KeyPress| camera.key(press)}
            cursor={cursor}
            on_wheel={gestures::first_of([
                recognised.wheel,
                gestures::scroll_wheel(None, move |gesture: ScrollGesture| {
                    if gesture.modifiers.ctrl {
                        let factor = (gesture.delta.y * ZOOM_PER_SCROLL_POINT).exp();
                        camera.zoom(factor, gesture.pos);
                    } else {
                        camera.pan(gesture.delta);
                    }
                }),
            ])}
            on_pointer={recognised.pointer}
            children={Some(content_node)}
        />
    };
    let rect_measured = measure(built);
    create_effect(move || {
        if let Some(rect) = rect_measured.get() {
            rect_held.set(rect);
        }
    });
    built
}

#[derive(Clone, Copy)]
pub struct PanZoomApi(Binding<PanZoomView>);

impl PanZoomApi {
    pub fn view(&self) -> Binding<PanZoomView> {
        self.0
    }
}
