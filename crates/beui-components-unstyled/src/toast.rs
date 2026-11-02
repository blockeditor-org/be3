use std::hash::{Hash, Hasher};
use std::time::Duration;

use accesskit::{Live, Node, Role};
use beui_macros::{component, view};
use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::Image;
use beui_view::types::Placement;
use beui_view::types::{LayerLevel, LayerPolicy};

use crate::floating::Edge;
use crate::scroll::{Scroll, ScrollbarStyle};
use beui_view::types::{Extent, Sides};

pub const TOAST_DURATION: Duration = Duration::from_secs(8);
pub const TOAST_LIMIT: usize = 3;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ToastAction {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub source: String,
    pub title: String,
    pub message: String,
    pub danger: bool,
    pub image: Option<Image>,
    pub actions: Vec<ToastAction>,
    pub activates: bool,
    pub sticky: bool,
}

impl Toast {
    pub fn expires(&self) -> bool {
        !self.sticky && !self.danger
    }
}

impl Hash for Toast {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.source.hash(state);
        self.title.hash(state);
        self.message.hash(state);
        self.danger.hash(state);
        self.image.as_ref().map(Image::id).hash(state);
        self.actions.hash(state);
        self.activates.hash(state);
        self.sticky.hash(state);
    }
}

#[derive(Clone)]
pub struct ToastHandle {
    pub toast: Toast,
    pub dismiss: ClickCallback,
    pub activate: ClickCallback,
    pub act: Callback<String>,
}

#[component]
pub fn Toasts(
    anchor: Prop<OverlayAnchor>,
    toasts: Prop<Vec<Toast>>,
    #[prop(default = Some(TOAST_DURATION))] duration: Prop<Option<Duration>>,
    edge: Prop<Edge>,
    width: Prop<f32>,
    margin: Prop<f32>,
    spacing: Prop<f32>,
    #[prop(default = String::from("Notifications"))] label: Prop<String>,
    #[prop(default = TOAST_LIMIT)] limit: Prop<usize>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    more: RenderFn<Memo<usize>>,
    #[prop(children)] card: RenderFn<ToastHandle>,
) -> NodeId {
    let all = create_memo(move || toasts.get());
    let open = create_memo(move || all.with(|toasts| !toasts.is_empty()));
    let hidden = create_memo(move || all.with(|toasts| toasts.len().saturating_sub(limit.get())));
    let toasts = create_memo(move || all.with(|toasts| toasts[hidden.get()..].to_vec()));
    let bottom =
        edge.map(|edge| matches!(edge, Edge::Bottom | Edge::BottomStart | Edge::BottomEnd));
    let placement = edge.map(|edge| match edge {
        Edge::Top => Placement::InsideTop,
        Edge::TopStart => Placement::InsideTopStart,
        Edge::TopEnd => Placement::InsideTopEnd,
        Edge::Bottom => Placement::InsideBottom,
        Edge::BottomStart => Placement::InsideBottomStart,
        Edge::BottomEnd => Placement::InsideBottomEnd,
    });
    let label = create_memo(move || label.get());
    let padding = create_memo(move || Sides::all(margin.get()));
    let outer = create_memo(move || Extent::px(width.get() + 2.0 * margin.get()));
    view! {
        <Overlay
            anchor
            placement={placement}
            policy=LayerPolicy::Pass
            level=LayerLevel::Top
            traps_focus=false
            open={open}
        >
            <Frame width={outer}>
                <Scroll fit=true follow_end={bottom} scrollbar>
                    <Frame padding={padding}>
                        <ToastStack
                            toasts={toasts}
                            hidden={hidden}
                            duration
                            spacing
                            label={label}
                            on_dismiss
                            on_action
                            on_activate
                            more
                            card
                        />
                    </Frame>
                </Scroll>
            </Frame>
        </Overlay>
    }
}

#[component]
fn ToastStack(
    toasts: Memo<Vec<Toast>>,
    hidden: Memo<usize>,
    duration: Prop<Option<Duration>>,
    spacing: Prop<f32>,
    label: Prop<String>,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    more: RenderFn<Memo<usize>>,
    card: RenderFn<ToastHandle>,
) -> NodeId {
    let hiding = create_memo(move || hidden.get() > 0);
    component_accessibility(create_memo(move || {
        let mut node = Node::new(Role::Log);
        node.set_label(label.get());
        node.set_live(Live::Polite);
        node
    }));
    view! {
        <List spacing>
            <Show condition={hiding}>|| { more.call(hidden) }</Show>
            <ForEach keys={toasts}>|toast: Toast| {
                let lasts = match toast.expires() {
                    true => duration.get_untracked(),
                    false => None,
                };
                view! {
                    <ToastItem toast lasts on_dismiss on_action on_activate card />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn ToastItem(
    toast: Toast,
    lasts: Option<Duration>,
    on_dismiss: Callback<u64>,
    on_action: Callback<(u64, String)>,
    on_activate: Callback<u64>,
    card: RenderFn<ToastHandle>,
) -> NodeId {
    let id = toast.id;
    let expiry = create_timer(move || {
        on_dismiss.call(id);
        None
    });
    if let Some(lasts) = lasts {
        expiry.start(lasts);
    }
    let mut node = match toast.danger {
        true => Node::new(Role::Alert),
        false => Node::new(Role::Status),
    };
    node.set_live(match toast.danger {
        true => Live::Assertive,
        false => Live::Polite,
    });
    node.set_label(spoken(&toast));
    component_accessibility(node);
    let activate = match toast.activates {
        true => ClickCallback::new(move || on_activate.call(id)),
        false => ClickCallback::empty(),
    };
    let handle = ToastHandle {
        toast,
        dismiss: ClickCallback::new(move || on_dismiss.call(id)),
        activate,
        act: Callback::new(move |key: String| on_action.call((id, key))),
    };
    view! {
        <List spacing=0.0>{card.call(handle)}</List>
    }
}

fn spoken(toast: &Toast) -> String {
    match (toast.title.is_empty(), toast.message.is_empty()) {
        (false, false) => format!("{}: {}", toast.title, toast.message),
        (false, true) => toast.title.clone(),
        (true, _) => toast.message.clone(),
    }
}
