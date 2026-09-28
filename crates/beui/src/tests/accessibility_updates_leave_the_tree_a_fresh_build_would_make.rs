use std::collections::HashMap;

use accesskit::{Node as AccessNode, NodeId as AccessNodeId, TreeUpdate};

use super::*;
use crate::accessibility::{AccessibilityTree, WINDOW_NODE};
use crate::reactive::{
    ForEach, List, Show, Text, WriteSignal, build, create_memo, create_signal, view,
    with_reactive_scope,
};

type Step = Box<dyn Fn(&Setters)>;

struct Setters {
    title: WriteSignal<String>,
    rows: WriteSignal<Vec<u32>>,
    panel: WriteSignal<bool>,
}

#[test]
fn accessibility_updates_leave_the_tree_a_fresh_build_would_make() {
    let taken: Rc<RefCell<Option<Setters>>> = Rc::new(RefCell::new(None));
    let sink = taken.clone();
    let document = build(move || {
        let (title, set_title) = create_signal("Inbox".to_owned());
        let (rows, set_rows) = create_signal(vec![1u32, 2, 3]);
        let (panel, set_panel) = create_signal(true);
        sink.replace(Some(Setters {
            title: set_title,
            rows: set_rows,
            panel: set_panel,
        }));
        view! {
            <List spacing=0.0>
                <Text string={create_memo(move || title.get())} />
                <ForEach keys={rows}>
                    {|row: u32| view! {
                        <styled::Button
                            variant=styled::ButtonVariant::Primary
                            label={format!("Row {row}")}
                            on_click={|| {}}
                        />
                    }}
                </ForEach>
                <Show condition={panel}>
                    <List spacing=0.0>
                        <Text string="Details" />
                        <styled::Button
                            variant=styled::ButtonVariant::Primary
                            label="Archive"
                            on_click={|| {}}
                        />
                    </List>
                </Show>
            </List>
        }
    });
    let setters = taken.take().expect("the view published its setters");
    let mut harness = Harness::new(document);
    let mut mirror = Mirror::default();

    let output = harness.frame(Vec::new());
    apply(&mut mirror, output.accessibility_tree("Test", VIEWPORT));
    assert_matches_fresh(&harness, &mirror, "the first frame");

    let output = harness.frame(Vec::new());
    let update = output.accessibility_tree("Test", VIEWPORT);
    assert_eq!(
        update.nodes.len(),
        1,
        "an idle frame sends only the window, not the document's nodes"
    );
    apply(&mut mirror, update);
    assert_matches_fresh(&harness, &mirror, "an idle frame");

    let steps: Vec<(&str, Step)> = vec![
        (
            "a retitle",
            Box::new(|s: &Setters| s.title.set("Archive".to_owned())),
        ),
        (
            "rows arriving",
            Box::new(|s: &Setters| s.rows.set(vec![1, 2, 3, 4, 5])),
        ),
        (
            "rows leaving",
            Box::new(|s: &Setters| s.rows.set(vec![2, 5])),
        ),
        (
            "rows reordering",
            Box::new(|s: &Setters| s.rows.set(vec![5, 2])),
        ),
        ("a panel hiding", Box::new(|s: &Setters| s.panel.set(false))),
        (
            "a panel returning",
            Box::new(|s: &Setters| s.panel.set(true)),
        ),
    ];
    for (step, change) in &steps {
        with_reactive_scope(harness.document_mut(), || change(&setters));
        let output = harness.frame(Vec::new());
        apply(&mut mirror, output.accessibility_tree("Test", VIEWPORT));
        assert_matches_fresh(&harness, &mirror, step);
    }

    harness.key(Key::Tab, Modifiers::NONE);
    let output = harness.frame(Vec::new());
    apply(&mut mirror, output.accessibility_tree("Test", VIEWPORT));
    assert_matches_fresh(&harness, &mirror, "a focus move");

    *harness.viewport_mut() = vec2(VIEWPORT.x / 2.0, VIEWPORT.y);
    let output = harness.frame(Vec::new());
    apply(&mut mirror, output.accessibility_tree("Test", VIEWPORT));
    assert_matches_fresh(&harness, &mirror, "a resize");
}

#[derive(Default)]
struct Mirror {
    nodes: HashMap<AccessNodeId, AccessNode>,
    focus: Option<AccessNodeId>,
}

fn apply(mirror: &mut Mirror, update: TreeUpdate) {
    mirror.focus = Some(update.focus);
    mirror.nodes.extend(update.nodes);
    let mut kept = HashMap::new();
    let mut pending = vec![WINDOW_NODE];
    while let Some(id) = pending.pop() {
        if let Some(node) = mirror.nodes.remove(&id) {
            pending.extend(node.children().iter().copied());
            kept.insert(id, node);
        }
    }
    mirror.nodes = kept;
}

fn assert_matches_fresh(harness: &Harness, mirror: &Mirror, step: &str) {
    let document = harness.document();
    let kept = document
        .accessibility_tree
        .replace(AccessibilityTree::default());
    let fresh = document
        .accessibility_update(true)
        .expect("the document has an accessibility tree");
    document.accessibility_tree.replace(kept);

    let window = &mirror.nodes[&WINDOW_NODE];
    assert_eq!(window.children(), [fresh.root], "after {step}");
    assert_eq!(
        mirror.focus,
        Some(fresh.focus.unwrap_or(WINDOW_NODE)),
        "focus after {step}"
    );

    let fresh: HashMap<AccessNodeId, AccessNode> = fresh.nodes.into_iter().collect();
    let mut reachable = HashMap::new();
    let mut pending = vec![window.children()[0]];
    while let Some(id) = pending.pop() {
        let node = mirror
            .nodes
            .get(&id)
            .unwrap_or_else(|| panic!("after {step}, {id:?} is referenced but was never sent"));
        pending.extend(node.children().iter().copied());
        reachable.insert(id, node.clone());
    }
    assert_eq!(reachable.len(), fresh.len(), "node count after {step}");
    for (id, node) in &fresh {
        assert_eq!(reachable.get(id), Some(node), "{id:?} after {step}");
    }
}
