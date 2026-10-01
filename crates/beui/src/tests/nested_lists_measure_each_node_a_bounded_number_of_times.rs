use super::*;
use crate::base::Align;

const DEPTH: usize = 24;

#[test]
fn nested_lists_measure_each_node_a_bounded_number_of_times() {
    for align in [Align::Stretch, Align::Center] {
        let mut document = Document::new();
        let mut nodes = 0;
        let mut inner: Option<NodeOf<ListNode>> = None;
        for level in 0..DEPTH {
            let direction = match level % 2 {
                0 => Direction::Horizontal,
                _ => Direction::Vertical,
            };
            let list = document.create_list(direction, 4.0);
            document.set_list_align(list, align);
            let label = document.create_text(format!("level {level}"), 14.0, Color32::WHITE);
            document.append_child(list, label.id(), ItemSize::Intrinsic);
            nodes += 2;
            if let Some(child) = inner {
                document.append_child(list, child.id(), ItemSize::Intrinsic);
            }
            inner = Some(list);
        }
        document.set_root(inner.expect("the loop builds at least one list").id());
        let mut harness = Harness::new(document);
        harness.frame(Vec::new());

        let measured = harness.document().performance().latest.work.measured;
        assert!(
            measured <= nodes * 2,
            "{align:?}: laying out {nodes} nodes measured {measured} times"
        );
    }
}
