use beui_macros::{component, view};
use beui_view::components::overlay::{Overlay, use_overlay_presence};
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Placement;
use beui_view::types::{Dismiss, LayerPolicy};
use beui_view::types::{Pos2, Rect, Vec2};

pub struct AnchoredSheetHandle {
    pub open: Memo<bool>,
    pub on_close: Callback<()>,
    pub content: Build<NodeId>,
}

#[derive(Clone, Copy)]
pub struct AnchoredHandle {
    pub sheet: bool,
    pub shown: Memo<bool>,
}

/// What a dropdown look wraps: the popup's content, and the frame its arrow points from. A
/// look that draws a frame around the content binds `pointer` to it, so a popup given a
/// `beak` stands that frame off its anchor and points the beak from it.
#[derive(Clone, Copy)]
pub struct Dropdown {
    pub content: NodeId,
    pub pointer: NodeRef,
}

#[derive(Clone, Copy)]
pub struct AnchoredStyle {
    pub(crate) dropdown: RenderFn<Dropdown, NodeId>,
    pub(crate) sheet: RenderFn<AnchoredSheetHandle, NodeId>,
}

impl AnchoredStyle {
    pub fn new(
        dropdown: impl Fn(Dropdown) -> NodeId + 'static,
        sheet: impl Fn(AnchoredSheetHandle) -> NodeId + 'static,
    ) -> Self {
        Self {
            dropdown: RenderFn::new(dropdown),
            sheet: RenderFn::new(sheet),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SheetWhen {
    #[default]
    Touched,
    Always,
    Never,
}

/// The surface every popup sits on: a dropdown placed by `placement` beside `anchor` (or
/// at the point `at`, pinned to the anchor's node) when a mouse or pen opened it, the
/// same content in a sheet when a finger did (`touched_by_finger`), unless `sheet` says
/// otherwise. A dismiss writes `open` false; an `Inert` one (a tooltip) takes no
/// dismissal, so it never claims Back or a press outside. Content is built once per look the first
/// time that look opens, and removed on close unless `keep_alive` keeps it, with its
/// state, while closed; bind focus to the handle's `shown`, not to `open`. `beak` points
/// an arrow at the anchor from the frame the look binds to `Dropdown::pointer`.
#[component]
pub fn Anchored(
    style: AnchoredStyle,
    anchor: Prop<OverlayAnchor>,
    #[prop(default = None)] at: Prop<Option<Pos2>>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    /// Every popup takes one; only `Popover` keeps its content by default.
    #[prop(default = false)]
    keep_alive: bool,
    #[prop(default = 0.0)] beak: Prop<f32>,
    open: Binding<bool>,
    #[prop(default = SheetWhen::Touched)] sheet: Prop<SheetWhen>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = None)] trigger: Prop<Option<NodeRef>>,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    #[prop(children)] content: RenderFn<AnchoredHandle, NodeId>,
) -> NodeId {
    let inert = policy.clone();
    let dismissable = create_memo(move || inert.get() != LayerPolicy::Inert);
    let mut was_open = false;
    let mut touched = false;
    let as_sheet = create_memo(move || {
        let now = open.get();
        if now && !was_open {
            touched = untrack(touched_by_finger);
        }
        was_open = now;
        match sheet.get() {
            SheetWhen::Touched => touched,
            SheetWhen::Always => true,
            SheetWhen::Never => false,
        }
    });
    let dropped = create_memo(move || !as_sheet.get() && open.get());
    let raised = create_memo(move || as_sheet.get() && open.get());
    let mut ever_raised = false;
    let sheet_built = create_memo(move || {
        ever_raised |= raised.get();
        ever_raised
    });
    let anchor = create_memo(move || at_point(anchor.get(), at.get()));
    let close = Callback::new(move |()| open.set(false));
    let pointer = NodeRef::new();
    let look = move |sheet: bool, shown: Memo<bool>| {
        let built = move || content.call(AnchoredHandle { sheet, shown });
        match keep_alive {
            false => built(),
            true => {
                let presence = use_overlay_presence();
                view! {
                    <Kept key={sheet}>|| {
                        if let Some(presence) = presence {
                            presence.provide();
                        }
                        built()
                    }</Kept>
                }
            }
        }
    };
    let surfaces = move || {
        view! {
            <List spacing=0.0>
                <Overlay
                    @node_ref=&overlay
                    anchor={anchor}
                    placement
                    bounds
                    policy
                    traps_focus
                    trigger
                    pointer={pointer}
                    beak
                    open={dropped}
                    dismissable={dismissable}
                    on_dismiss={move |_: Dismiss| close.call(())}
                    content={move || style.dropdown.call(Dropdown {
                        content: look(false, dropped),
                        pointer,
                    })}
                />
                <Show value={sheet_built}>|| {
                    style.sheet.call(AnchoredSheetHandle {
                        open: raised,
                        on_close: close,
                        content: Build::new(move || look(true, raised)),
                    })
                }</Show>
            </List>
        }
    };
    match keep_alive {
        false => surfaces(),
        true => view! {
            <KeepAlive>{surfaces()}</KeepAlive>
        },
    }
}

/// `anchor`, or the point `at` pinned to its node so it follows the node as it moves.
fn at_point(anchor: OverlayAnchor, at: Option<Pos2>) -> OverlayAnchor {
    let Some(at) = at else {
        return anchor;
    };
    match anchor.node() {
        Some(node) => OverlayAnchor::Pinned {
            node,
            rect: Rect::from_min_size(at, Vec2::ZERO),
        },
        None => OverlayAnchor::Point(at),
    }
}
