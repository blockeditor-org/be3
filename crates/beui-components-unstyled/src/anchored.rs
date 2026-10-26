use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::layer::{Dismiss, LayerPolicy};
use beui_core::node::NodeId;
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::*;

pub struct AnchoredSheetHandle {
    pub open: Memo<bool>,
    pub on_close: ClickCallback,
    pub content: Child,
}

#[derive(Clone, Copy)]
pub struct AnchoredHandle {
    pub sheet: bool,
    pub shown: Memo<bool>,
}

#[derive(Clone, Copy)]
pub struct AnchoredStyle {
    pub(crate) dropdown: RenderFn<Child>,
    pub(crate) sheet: RenderFn<AnchoredSheetHandle>,
}

impl AnchoredStyle {
    pub fn new(
        dropdown: impl Fn(Child) -> NodeId + 'static,
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

#[component]
pub fn Anchored(
    style: AnchoredStyle,
    anchor: Prop<OverlayAnchor>,
    #[prop(default = Placement::BelowStart)] placement: Prop<Placement>,
    open: Prop<bool>,
    #[prop(default = SheetWhen::Touched)] sheet: Prop<SheetWhen>,
    #[prop(default = true)] traps_focus: Prop<bool>,
    #[prop(default = LayerPolicy::BlocksAll)] policy: Prop<LayerPolicy>,
    trigger: Option<NodeRef>,
    #[prop(default = false)] keep_alive: bool,
    #[prop(default = NodeRef::new())] overlay: NodeRef,
    on_dismiss: ClickCallback,
    #[prop(children)] content: RenderFn<AnchoredHandle>,
) -> NodeId {
    let touch = use_touch();
    let mut was_open = false;
    let mut touched = false;
    let as_sheet = create_memo(move || {
        let now = open.get();
        if now && !was_open {
            touched = touch.get_untracked();
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
    view! {
        <List spacing=0.0>
            <Overlay
                @node_ref=&overlay
                anchor
                placement
                policy
                traps_focus
                trigger={trigger}
                open={dropped}
                keep_alive
                on_dismiss={move |_: Dismiss| on_dismiss.call()}
                content={move || style.dropdown.call(content.call(AnchoredHandle {
                    sheet: false,
                    shown: dropped,
                }))}
            />
            <Show condition={sheet_built}>|| {
                style.sheet.call(AnchoredSheetHandle {
                    open: raised,
                    on_close: ClickCallback::new(move || on_dismiss.call()),
                    content: content.call(AnchoredHandle {
                        sheet: true,
                        shown: raised,
                    }),
                })
            }</Show>
        </List>
    }
}
