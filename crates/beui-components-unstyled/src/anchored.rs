use beui_view::components::overlay::Overlay;
use beui_view::measure::measure;
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
    /// Builds the content, each time the sheet opens.
    pub content: BuildFn<NodeId>,
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
    anchor: Memo<OverlayAnchor>,
}

impl Dropdown {
    /// The width of the node the popup hangs from, in its own units, once it is laid out (a
    /// frame late on the HTML core), and `None` while it opens at a point: a select's list is
    /// at least as wide as its trigger.
    pub fn anchor_width(self) -> Memo<Option<f32>> {
        let anchor = self.anchor;
        create_memo(move || {
            let node = match anchor.get() {
                OverlayAnchor::Node(node) | OverlayAnchor::Band { node, .. } => node.try_get()?,
                _ => return None,
            };
            Some(measure(node).get()?.size.x)
        })
    }
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
/// dismissal, so it never claims Back or a press outside. Content is built each time a look
/// opens and removed once it has closed (a caller keeps its state while closed by wrapping it
/// in a `Kept`); bind focus to the handle's `shown`, not to `open`. `beak` points
/// an arrow at the anchor from the frame the look binds to `Dropdown::pointer`.
#[component]
pub fn Anchored(
    style: AnchoredStyle,
    anchor: Prop<OverlayAnchor>,
    #[prop(default = None)] at: Prop<Option<Pos2>>,
    #[prop(default = Placement::BELOW_START)] placement: Prop<Placement>,
    #[prop(default = None)] bounds: Prop<Option<Rect>>,
    #[prop(default = 0.0)] beak: Prop<f32>,
    open: Binding<bool>,
    #[prop(default = SheetWhen::Touched)] sheet: Prop<SheetWhen>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    #[prop(default = None)] trigger: Prop<Option<NodeRef>>,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    #[prop(children)] content: RenderFn<AnchoredHandle, NodeId>,
) -> NodeId {
    let never_sheet = matches!(sheet, Prop::Static(SheetWhen::Never));
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
    let look = move |sheet: bool, shown: Memo<bool>| content.call(AnchoredHandle { sheet, shown });
    let dropdown = move || {
        view! {
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
                    anchor,
                })}
            />
        }
    };
    // A popup that never becomes a sheet (a tooltip) is its overlay alone: no list around it
    // and no empty `Show` for a sheet it never builds.
    match never_sheet {
        true => dropdown(),
        false => view! {
            <List spacing=0.0>
                {dropdown()}
                <Show value={sheet_built}>|| {
                    style.sheet.call(AnchoredSheetHandle {
                        open: raised,
                        on_close: close,
                        content: BuildFn::new(move || look(true, raised)),
                    })
                }</Show>
            </List>
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
