use std::ops::Range;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{Callback, Memo, NodeRef, RenderFn, create_memo};

use super::{Context, TextAreaState};

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone)]
pub struct CompletionMenu {
    pub items: Memo<Vec<Completion>>,
    pub highlighted: Memo<usize>,
    pub pick: Callback<usize>,
    pub highlight: Callback<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Query {
    pub(super) range: Range<usize>,
    pub(super) text: String,
}

const QUERY_LIMIT: usize = 32;

#[component]
pub(super) fn Completions(cx: Context, render: RenderFn<CompletionMenu>) -> NodeId {
    let anchor = NodeRef::new();
    anchor.fill(cx.anchor);
    let (pick_cx, highlight_cx) = (cx.clone(), cx.clone());
    let highlighted = cx.highlighted.clone();
    let content = render.call(CompletionMenu {
        items: cx.completions.clone(),
        highlighted: create_memo(move || highlighted.get()),
        pick: Callback::new(move |index: usize| pick_cx.complete(index)),
        highlight: Callback::new(move |index: usize| highlight_cx.set_highlighted.set(index)),
    });
    view! {
        <Overlay
            anchor={OverlayAnchor::Node(anchor)}
            placement=Placement::BelowStart
            mode=OverlayMode::Floating
            traps_focus=false
            open=true
        >
            {content}
        </Overlay>
    }
}

pub(super) fn query(state: &TextAreaState, trigger: u8) -> Option<Query> {
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

