use crate::node::NodeId;

pub fn children(node: NodeId) -> Vec<NodeId> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut found = Vec::new();
        if let Some(dom) = crate::dom::dom(node) {
            nearest_below(&dom, &mut |_| true, &mut found);
        }
        found
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = node;
        Vec::new()
    }
}

pub fn focusables_within(node: NodeId) -> Vec<NodeId> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        let mut found = Vec::new();
        if let Some(dom) = crate::dom::dom(node) {
            every_below(
                &dom,
                &mut |dom: &web_sys::Node| {
                    dom.dyn_ref::<web_sys::Element>()
                        .and_then(|element| element.get_attribute("tabindex"))
                        .is_some()
                },
                &mut found,
            );
        }
        found
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = node;
        Vec::new()
    }
}

pub fn focus_into(node: NodeId) {
    if let Some(first) = focusables_within(node).first() {
        crate::focus::set_focus(Some(*first));
    }
}

pub fn text(node: NodeId) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        crate::dom::dom(node)
            .and_then(|dom| dom.text_content())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = node;
        String::new()
    }
}

pub fn selected_text(node: NodeId) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(selection) =
            crate::dom::window().and_then(|window| window.get_selection().ok().flatten())
        else {
            return String::new();
        };
        let inside = crate::dom::dom(node).is_some_and(|dom| {
            selection
                .anchor_node()
                .is_some_and(|anchor| dom.contains(Some(&anchor)))
        });
        match inside {
            true => selection.to_string().into(),
            false => String::new(),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = node;
        String::new()
    }
}

pub fn select_all(node: NodeId) {
    #[cfg(target_arch = "wasm32")]
    if let Some(selection) =
        crate::dom::window().and_then(|window| window.get_selection().ok().flatten())
        && let Some(dom) = crate::dom::dom(node)
    {
        let _ = selection.select_all_children(&dom);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = node;
}

#[cfg(target_arch = "wasm32")]
fn nearest_below(
    dom: &web_sys::Node,
    wanted: &mut impl FnMut(&web_sys::Node) -> bool,
    found: &mut Vec<NodeId>,
) {
    let mut next = dom.first_child();
    while let Some(child) = next {
        match crate::dom::node_of(&child) {
            Some(node) if wanted(&child) => found.push(node),
            _ => nearest_below(&child, wanted, found),
        }
        next = child.next_sibling();
    }
}

#[cfg(target_arch = "wasm32")]
fn every_below(
    dom: &web_sys::Node,
    wanted: &mut impl FnMut(&web_sys::Node) -> bool,
    found: &mut Vec<NodeId>,
) {
    let mut next = dom.first_child();
    while let Some(child) = next {
        if wanted(&child)
            && let Some(node) = crate::dom::node_of(&child)
        {
            found.push(node);
        }
        every_below(&child, wanted, found);
        next = child.next_sibling();
    }
}

pub fn stacked_at(nodes: &[NodeId], pos: beui_types::Pos2) -> Vec<NodeId> {
    #[cfg(target_arch = "wasm32")]
    {
        let layers = crate::overlay::stacked_layers();
        let within = |layer: NodeId, node: NodeId| {
            let (Some(layer), Some(node)) = (crate::dom::dom(layer), crate::dom::dom(node)) else {
                return false;
            };
            layer.contains(Some(&node))
        };
        let rank = |node: NodeId| {
            layers
                .iter()
                .rposition(|(layer, _)| within(*layer, node))
                .map_or(0, |index| index + 1)
        };
        let mut placed: Vec<(usize, NodeId)> = nodes
            .iter()
            .filter(|node| crate::measure::rect(**node).is_some())
            .map(|node| (rank(*node), *node))
            .collect();
        placed.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));
        let foreign = (0..layers.len()).rev().find(|index| {
            layers[*index].1.contains(pos) && !placed.iter().any(|(rank, _)| *rank == index + 1)
        });
        placed
            .into_iter()
            .filter(|(rank, _)| foreign.is_none_or(|foreign| *rank > foreign + 1))
            .map(|(_, node)| node)
            .collect()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (nodes, pos);
        Vec::new()
    }
}
