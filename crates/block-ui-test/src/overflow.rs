use beui::{Document, NodeId};

pub(crate) fn overflows(document: &Document, allowed: &[String]) -> Vec<String> {
    document
        .overflows()
        .iter()
        .filter(|overflow| !within_allowed(document, overflow.node, allowed))
        .map(ToString::to_string)
        .collect()
}

fn within_allowed(document: &Document, node: NodeId, allowed: &[String]) -> bool {
    let mut current = Some(node);
    while let Some(node) = current {
        if !document.contains(node) {
            return false;
        }
        if document
            .node_test_ids(node)
            .iter()
            .any(|test_id| allowed.contains(test_id))
        {
            return true;
        }
        current = document.node_parent(node);
    }
    false
}

pub(crate) fn assert_fits(name: &str, mut found: Vec<String>) {
    found.sort();
    found.dedup();
    if found.is_empty() {
        return;
    }
    panic!(
        "the painting {name} shows {} element(s) out of bounds, cut off where nothing scrolls to them or drawn outside their own box:\n  {}",
        found.len(),
        found.join("\n  ")
    );
}
