use beui_core::base::text::TextNode;
use beui_core::document::Document;
use beui_types::NodeId;

/// How [`find_text`] compares a text node's text with what it looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// The whole text is the words.
    Exact,
    /// The text holds the words.
    Contains,
}

/// The first text node at or under `root`, depth first, that is laid out (has a rect) and whose
/// text matches `text`.
pub fn find_text(document: &Document, root: NodeId, text: &str, how: Match) -> Option<NodeId> {
    let here = document.node_rect(root).is_some()
        && document
            .arena
            .kind_of::<TextNode>(root)
            .is_some_and(|node| {
                let shown = document.text(node);
                match how {
                    Match::Exact => shown == text,
                    Match::Contains => shown.contains(text),
                }
            });
    if here {
        return Some(root);
    }
    document
        .children(root)
        .into_iter()
        .find_map(|child| find_text(document, child, text, how))
}
