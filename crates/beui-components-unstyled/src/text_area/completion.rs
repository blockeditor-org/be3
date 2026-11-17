use beui_view::types::{Dismiss, LayerPolicy};
use std::ops::Range;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_view::components::overlay::Overlay;
use beui_view::primitives::*;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::reactive::*;
use beui_view::types::CursorIcon;
use beui_view::types::Placement;

use super::{Context, TextAreaState};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Completion {
    pub label: String,
    pub insert: String,
}

pub(super) type Search = Rc<dyn Fn(&str) -> Vec<Completion>>;

#[derive(Clone, Default)]
pub struct Completer {
    pub(super) trigger: Option<u8>,
    pub(super) search: Option<Search>,
}

impl Completer {
    pub fn new(trigger: char, search: impl Fn(&str) -> Vec<Completion> + 'static) -> Self {
        Self {
            trigger: u8::try_from(trigger).ok(),
            search: Some(Rc::new(search)),
        }
    }

    pub fn none() -> Self {
        Self::default()
    }
}

pub struct CompletionRowHandle {
    pub index: ReadSignal<usize>,
    pub completion: Completion,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

#[derive(Clone, Copy)]
pub struct CompletionMenu {
    row: RenderFn<CompletionRowHandle, NodeId>,
    panel: RenderFn<NodeId, NodeId>,
}

impl CompletionMenu {
    pub fn new(
        row: impl Fn(CompletionRowHandle) -> NodeId + 'static,
        panel: impl Fn(NodeId) -> NodeId + 'static,
    ) -> Self {
        Self {
            row: RenderFn::new(row),
            panel: RenderFn::new(panel),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Query {
    pub(super) range: Range<usize>,
    pub(super) text: String,
}

const QUERY_LIMIT: usize = 32;

#[component]
pub(super) fn Completions(cx: Context, menu: CompletionMenu) -> NodeId {
    let anchor = NodeRef::new();
    anchor.fill(cx.anchor);
    let items = cx.completions;
    let CompletionMenu { row, panel } = menu;
    let layer = cx.completion_layer;
    view! {
        <Overlay
            @node_ref=&layer
            anchor={OverlayAnchor::Node(anchor)}
            placement=Placement::BELOW_START
            policy=LayerPolicy::Pass
            traps_focus=false
            open=true
            on_dismiss={move |_: Dismiss| cx.dismiss()}
        >
            {panel.call(view! {
                <List spacing=0.0>
                    <ForEach value={items}>|completion: Completion, index: ReadSignal<usize>|
                        <CompletionRow cx completion index row />
                    </ForEach>
                </List>
            })}
        </Overlay>
    }
}

#[component]
fn CompletionRow(
    cx: Context,
    completion: Completion,
    index: ReadSignal<usize>,
    row: RenderFn<CompletionRowHandle, NodeId>,
) -> NodeId {
    let highlighted = cx.highlighted;
    let highlighted = create_memo(move || highlighted.get() == index.get());
    let hovered = create_signal(false);
    let content = row.call(CompletionRowHandle {
        index,
        completion,
        highlighted,
        hovered: hovered.read_only(),
    });
    view! {
        <Interactive
            @test_id={move || format!("text.completion.{}", index.get())}
            cursor=CursorIcon::PointingHand
            on_click={move || cx.complete(index.get_untracked())}
            on_hover_change={move |inside: bool| {
                hovered.set(inside);
                if inside {
                    cx.highlighted.set(index.get_untracked());
                }
            }}
            children={Some(content)}
        />
    }
}

pub(super) fn query(state: TextAreaState, trigger: u8) -> Option<Query> {
    let ranges = state.selection_ranges();
    let [range] = ranges.as_slice() else {
        return None;
    };
    if !range.is_empty() {
        return None;
    }
    let caret = range.start;
    let bytes = state.bytes();
    let mut start = caret;
    while start > 0
        && caret - start <= QUERY_LIMIT
        && bytes
            .get(start - 1)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'+' | b'-'))
    {
        start -= 1;
    }
    if caret - start > QUERY_LIMIT || start == 0 || bytes.get(start - 1) != Some(&trigger) {
        return None;
    }
    let at = start - 1;
    let opens = at
        .checked_sub(1)
        .and_then(|before| bytes.get(before))
        .is_none_or(|before| before.is_ascii_whitespace() || b"([{\"'".contains(before));
    opens.then(|| Query {
        range: at..caret,
        text: String::from_utf8_lossy(&bytes[start..caret]).into_owned(),
    })
}
