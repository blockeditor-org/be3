use beui_core::base::overlay::{OverlayAnchor, OverlayNode, Placement, WindowSpec};
use beui_core::geometry::{Pos2, Vec2};
use beui_core::layer::{LayerLevel, LayerPolicy};
use beui_core::node::NodeId;
use beui_core::window::{
    WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowRole,
};
use beui_macros::{component, view};

use crate::components::frame::Frame;
use crate::components::overlay::Overlay;
use crate::reactive::{
    Callback, Get, IntoProp, Memo, Prop, ReadSignal, RenderFn, Set, create_binding, create_memo,
    create_signal, provide_context, use_context, with_document,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct WindowHandle {
    pub id: WindowId,
    pub size: Memo<Vec2>,
    hosted: Memo<bool>,
    real: Memo<Option<WindowId>>,
}

impl WindowHandle {
    pub fn is_window(self) -> bool {
        self.hosted.get()
    }

    pub fn real_window(self) -> Option<WindowId> {
        self.real.get_untracked()
    }

    pub fn following(
        id: WindowId,
        current: impl Fn() -> Option<WindowHandle> + Clone + 'static,
    ) -> Self {
        let live = move || {
            current().filter(|handle| {
                handle.size.is_alive() && handle.hosted.is_alive() && handle.real.is_alive()
            })
        };
        let last = std::rc::Rc::new(std::cell::Cell::new((Vec2::ZERO, false, None)));
        let follow = move || {
            if let Some(handle) = live() {
                last.set((handle.size.get(), handle.hosted.get(), handle.real.get()));
            }
            last.get()
        };
        let sized = follow.clone();
        let size = create_memo(move || sized().0);
        let hosting = follow.clone();
        let hosted = create_memo(move || hosting().1);
        let real = create_memo(move || follow().2);
        Self {
            id,
            size,
            hosted,
            real,
        }
    }

    pub fn begin_move(self) -> bool {
        self.ask(WindowRequest::BeginMove(self.id))
    }

    pub fn begin_resize(self, edge: WindowEdge) -> bool {
        self.ask(WindowRequest::BeginResize {
            window: self.id,
            edge,
        })
    }

    pub fn close(self) -> bool {
        self.ask(WindowRequest::Close(self.id))
    }

    fn ask(self, request: WindowRequest) -> bool {
        with_document(|document| document.request_window(request))
    }
}

fn provided(handle: WindowHandle, content: RenderFn<WindowHandle>) -> NodeId {
    provide_context(handle);
    content.call(handle)
}

pub fn use_window() -> Option<WindowHandle> {
    use_context::<WindowHandle>()
}

pub fn opens_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_opens_windows())
}

#[component]
pub fn Window(
    id: Option<WindowId>,
    size: Prop<Vec2>,
    #[prop(default = WindowPlacement::AtPointer)] placement: Prop<WindowPlacement>,
    #[prop(default = Pos2::ZERO)] at: Prop<Pos2>,
    #[prop(default = None)] title: Prop<Option<String>>,
    #[prop(default = WindowRole::Normal)] role: Prop<WindowRole>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = false)] keep_alive: bool,
    on_report: Callback<WindowReport>,
    #[prop(children)] content: RenderFn<WindowHandle>,
) -> NodeId {
    let id = id.unwrap_or_else(WindowId::next);
    let opens = opens_windows();
    let placed = with_document(|document| document.watch_placed_windows());
    let asked = placement.clone();
    let hosted = create_memo(move || {
        opens.get() || (placed.get() && matches!(asked.get(), WindowPlacement::Host(_)))
    });
    let outer = use_window();
    let real = create_memo(move || match hosted.get() {
        true => Some(id),
        false => outer.and_then(|outer| outer.real.get()),
    });
    let reported = create_signal(None::<Vec2>);
    let shown = create_memo(move || match (hosted.get(), reported.get()) {
        (true, Some(reported)) => reported,
        _ => size.get(),
    });
    let handle = WindowHandle {
        id,
        size: shown,
        hosted,
        real,
    };
    let heard = Callback::new(move |report: WindowReport| {
        if let WindowReport::Resized { size, .. } = report {
            reported.set(Some(size));
        }
        on_report.call(report);
    });
    let width = create_memo(move || shown.get().x);
    let height = create_memo(move || shown.get().y);
    let anchor = create_memo(move || OverlayAnchor::Point(at.get())).into_prop();
    let node = view! {
        <Overlay
            anchor={anchor}
            placement=Placement::At
            policy=LayerPolicy::Pass
            level={level}
            traps_focus=false
            open={open}
            keep_alive={keep_alive}
        >
            <Frame width=width height=height>{provided(handle, content)}</Frame>
        </Overlay>
    };
    let overlay = with_document(|document| document.arena.kind_of::<OverlayNode>(node))
        .expect("an Overlay builds an overlay node");
    create_binding(move || {
        let spec = WindowSpec {
            id,
            size: size.get(),
            placement: placement.get(),
            title: title.get(),
            role: role.get(),
            on_report: heard,
        };
        with_document(|document| document.set_overlay_window(overlay, Some(spec)));
    });
    node
}
