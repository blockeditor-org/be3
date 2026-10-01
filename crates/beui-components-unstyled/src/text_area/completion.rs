use std::ops::Range;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::input::CursorIcon;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Child, ForEach, Interactive, List, Memo, NodeRef, ReadSignal, RenderFn, clone, create_memo,
    create_signal,
};

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

pub struct CompletionRowHandle {
    pub index: usize,
    pub completion: Memo<Option<Completion>>,
    pub highlighted: Memo<bool>,
    pub hovered: ReadSignal<bool>,
}

#[derive(Clone)]
pub struct CompletionMenu {
    row: RenderFn<CompletionRowHandle>,
    panel: RenderFn<Child>,
}

impl CompletionMenu {
    pub fn new(
        row: impl Fn(CompletionRowHandle) -> NodeId + 'static,
        panel: impl Fn(Child) -> NodeId + 'static,
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
    let items = cx.completions.clone();
    let keys = create_memo(clone!(items -> move || (0..items.get().len()).collect::<Vec<usize>>()));
    let CompletionMenu { row, panel } = menu;
    view! {
        <Overlay
            anchor={OverlayAnchor::Node(anchor)}
            placement=Placement::BelowStart
            mode=OverlayMode::Floating
            traps_focus=false
            open=true
        >
            {panel.call(view! {
                <List spacing=0.0>
                    <ForEach keys>
                        {move |index: usize| {
                            let (cx, row) = (cx.clone(), row.clone());
                            view! {
                                <CompletionRow cx index row />
                            }
                        }}
                    </ForEach>
                </List>
            })}
        </Overlay>
    }
}

#[component]
fn CompletionRow(cx: Context, index: usize, row: RenderFn<CompletionRowHandle>) -> NodeId {
    let items = cx.completions.clone();
    let completion = create_memo(move || items.get().get(index).cloned());
    let highlighted = cx.highlighted.clone();
    let highlighted = create_memo(move || highlighted.get() == index);
    let (hovered, set_hovered) = create_signal(false);
    let content = row.call(CompletionRowHandle {
        index,
        completion,
        highlighted,
        hovered,
    });
    let pick_cx = cx.clone();
    view! {
        <Interactive
            @test_id={format!("text.completion.{index}")}
            cursor=CursorIcon::PointingHand
            on_click={move || pick_cx.complete(index)}
            on_hover_change={move |inside: bool| {
                set_hovered.set(inside);
                if inside {
                    cx.set_highlighted.set(index);
                }
            }}
            children={Some(content)}
        />
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
