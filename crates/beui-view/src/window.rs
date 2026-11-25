use crate::reactive::{NodeId, OverlayAnchor};
use crate::reactive::{component, view};
use crate::types::window::{
    WindowEdge, WindowId, WindowLevel, WindowPlacement, WindowReport, WindowRequest, WindowRole,
    WindowSpec,
};
use crate::types::{Color, Dismiss, Extent, LayerLevel, LayerPolicy, Placement, Pos2, Rect, Vec2};

use crate::components::frame::Frame;
use crate::components::overlay::Overlay;
use crate::measure::measure;
use crate::reactive::{
    Callback, Get, Memo, NodeRef, Prop, RenderFn, Set, create_memo, create_signal,
    provide_context, use_context, use_viewport_size,
};

/// What a window points at, in its window's points, while it is not a window of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowAnchor {
    /// Beside this rect (`Placement::CARD`), pointing a beak at it from the `pointer` frame,
    /// and following its owner as the owner moves.
    Beside(Rect),
    /// Beside this node, as `Beside` its rect, following it as it moves: a small form beside
    /// the control that opened it.
    Node(NodeRef),
    /// At the top centre of its owner (of its window, with no owner), pointing at nothing:
    /// where a window goes whose anchor does not show.
    Top,
    /// Centred on its owner (on its window, with no owner): a confirmation, a large form.
    Centre,
}

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

    pub fn real(self) -> Memo<Option<WindowId>> {
        self.real
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
        crate::core::request_window(WindowRequest::BeginMove(self.id))
    }

    pub fn begin_resize(self, edge: WindowEdge) -> bool {
        crate::core::request_window(WindowRequest::BeginResize {
            window: self.id,
            edge,
        })
    }

    pub fn close(self) -> bool {
        crate::core::request_window(WindowRequest::Close(self.id))
    }

    pub fn place(self, placement: WindowPlacement) -> bool {
        crate::core::request_window(WindowRequest::Place {
            window: self.id,
            placement,
        })
    }
}

fn provided(handle: WindowHandle, content: RenderFn<WindowHandle, NodeId>) -> NodeId {
    provide_context(handle);
    content.call(handle)
}

pub fn use_window() -> Option<WindowHandle> {
    use_context::<WindowHandle>()
}

#[component]
pub fn Window(
    id: Option<WindowId>,
    size: Prop<Vec2>,
    #[prop(default = WindowPlacement::AtPointer)] placement: Prop<WindowPlacement>,
    #[prop(default = Pos2::ZERO)] at: Prop<Pos2>,
    #[prop(default = None)] title: Prop<Option<String>>,
    #[prop(default = None)] notice: Prop<Option<String>>,
    #[prop(default = WindowRole::Normal)] role: Prop<WindowRole>,
    #[prop(default = None)] fit: Prop<Option<Vec2>>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = LayerLevel::Normal)] level: Prop<LayerLevel>,
    #[prop(default = false)] fullscreen: Prop<bool>,
    /// The node the window belongs to (a frame, a tab's panel): it hangs from it, follows it
    /// as it moves and, while it `blocks`, keeps presses and focus off it. It may change while
    /// the window is open (an owner built after the window).
    #[prop(default = None)]
    owner: Prop<Option<NodeId>>,
    /// What it points at while it is not a window of its own; `None` opens it at `at`.
    #[prop(default = None)]
    anchor: Prop<Option<WindowAnchor>>,
    /// How far the beak reaches from the frame bound to `pointer` towards a `Beside` anchor.
    #[prop(default = 0.0)]
    beak: Prop<f32>,
    pointer: Option<NodeRef>,
    /// Blocks its owner (with no owner, its whole window) while open: no presses reach it,
    /// `scrim` covers it, focus is trapped in the window, and a press there asks the window to
    /// close (`on_dismiss`).
    #[prop(default = false)]
    blocks: Prop<bool>,
    #[prop(default = Color::TRANSPARENT)] scrim: Prop<Color>,
    #[prop(default = 0.0)] scrim_radius: Prop<f32>,
    /// Whether a host that opens windows opens this one as a window of its own; when false it
    /// is always an overlay in its window, sized to its content (`size` unused).
    #[prop(default = true)]
    own_window: Prop<bool>,
    /// Which of the document's windows shows it while it is an overlay (see `Overlay`).
    #[prop(default = None)]
    shown_in: Prop<Option<WindowId>>,
    /// Hears Escape, back and a press it shuts out, as an overlay's `on_dismiss` does.
    on_dismiss: Option<Callback<Dismiss>>,
    #[prop(default = Callback::default())] on_report: Callback<WindowReport>,
    #[prop(children)] content: RenderFn<WindowHandle, NodeId>,
) -> NodeId {
    let id = id.unwrap_or_else(WindowId::next);
    let opens = opens_windows();
    let placed = crate::core::placed_windows();
    let asked = placement.clone();
    let separate = own_window.clone();
    let hosted = create_memo(move || {
        separate.get()
            && (opens.get() || (placed.get() && matches!(asked.get(), WindowPlacement::Host(_))))
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
    let sized = own_window.clone();
    let fitting = fit.clone();
    let width = create_memo(move || match (sized.get(), fitting.get()) {
        (false, _) => Extent::INTRINSIC,
        (true, Some(_)) => Extent::fraction(1.0),
        (true, None) => Extent::px(shown.get().x),
    });
    let sized = own_window.clone();
    let fitting = fit.clone();
    let height = create_memo(move || match (sized.get(), fitting.get()) {
        (false, _) => Extent::INTRINSIC,
        (true, Some(_)) => Extent::fraction(1.0),
        (true, None) => Extent::px(shown.get().y),
    });
    let viewport = use_viewport_size();
    let owner = create_memo(move || owner.get());
    // The overlay hangs from this, filled with the owner whenever the owner changes; a pinned
    // anchor reads it again at each layout.
    let hangs_from = NodeRef::new();
    let owned = create_memo(move || Some(measure(owner.get()?).get()?.rect));
    // Where the overlay goes, how, and whether its beak may point: hosted windows are placed by
    // their host, at `placement`, so this is only the overlay's.
    let placed = create_memo(move || {
        let point = |at: Pos2| Rect::from_min_size(at, Vec2::ZERO);
        let pinned = |rect: Rect| match owner.get() {
            Some(node) => {
                if hangs_from.try_get() != Some(node) {
                    hangs_from.fill(node);
                }
                OverlayAnchor::Pinned {
                    node: hangs_from,
                    rect,
                }
            }
            None => OverlayAnchor::Rect(rect),
        };
        match anchor.get() {
            Some(WindowAnchor::Beside(rect)) => (pinned(rect), Placement::CARD, true),
            Some(WindowAnchor::Node(node)) => (OverlayAnchor::Node(node), Placement::CARD, true),
            Some(WindowAnchor::Top) => {
                let frame = owned
                    .get()
                    .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, viewport.get()));
                let top = Rect::from_min_max(frame.min, Pos2::new(frame.max.x, frame.min.y));
                (pinned(top), Placement::CARD, false)
            }
            Some(WindowAnchor::Centre) => match (owner, owned.get()) {
                (Some(node), Some(rect)) => (
                    OverlayAnchor::Pinned { node, rect },
                    Placement::CenterOn,
                    false,
                ),
                _ => (OverlayAnchor::Point(Pos2::ZERO), Placement::Center, false),
            },
            None => (pinned(point(at.get())), Placement::At, false),
        }
    });
    let anchored = create_memo(move || placed.get().0);
    let placement_of = create_memo(move || placed.get().1);
    let reach = create_memo(move || match placed.get().2 {
        true => beak.get(),
        false => 0.0,
    });
    let blocking = blocks.clone();
    // With no owner, its whole window is what it blocks.
    let policy = create_memo(move || match (blocking.get(), owner.is_some()) {
        (true, true) => LayerPolicy::BlocksParent,
        (true, false) => LayerPolicy::BlocksAll,
        (false, _) => LayerPolicy::Pass,
    });
    let blocking = blocks.clone();
    let dims = create_memo(move || match blocking.get() {
        true => scrim.get(),
        false => Color::TRANSPARENT,
    });
    let traps = create_memo(move || blocks.get());
    let dismiss = on_dismiss.unwrap_or_default();
    let leveled = level.clone();
    let separate = own_window.clone();
    let spec = create_memo(move || {
        if !separate.get() {
            return None;
        }
        Some(WindowSpec {
            id,
            size: size.get(),
            placement: placement.get(),
            title: title.get(),
            notice: notice.get(),
            role: role.get(),
            level: match (fullscreen.get(), leveled.get()) {
                (true, _) => WindowLevel::Fullscreen,
                (false, LayerLevel::Normal) => WindowLevel::Normal,
                (false, LayerLevel::Raised | LayerLevel::Top | LayerLevel::Lock) => {
                    WindowLevel::Above
                }
            },
            fit: fit.get(),
            on_report: heard,
        })
    });
    view! {
        <Overlay
            anchor={anchored}
            placement={placement_of}
            policy={policy}
            scrim={dims}
            scrim_radius={scrim_radius}
            pointer={pointer}
            beak={reach}
            level={level}
            shown_in={shown_in}
            window={spec}
            traps_focus={traps}
            open={open}
            on_dismiss={dismiss}
        >
            <Frame width=width height=height>{provided(handle, content)}</Frame>
        </Overlay>
    }
}

pub fn opens_windows() -> crate::reactive::ReadSignal<bool> {
    crate::core::window::opens_windows()
}

pub fn window_origin(window: WindowId) -> Option<Pos2> {
    crate::core::window::window_origin(window)
}

pub fn set_work_area(window: WindowId, area: Option<Rect>) {
    crate::core::window::set_work_area(window, area);
}
