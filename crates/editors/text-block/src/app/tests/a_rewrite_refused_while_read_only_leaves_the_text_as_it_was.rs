use super::{Uuid, WORKSPACE_ID, block_url, editor, text};

fn shown(document: &beui::Document, id: beui::NodeId, out: &mut String) {
    if let Some(node) = document.arena.kind_of(id) {
        out.push_str(document.text(node));
    }
    for child in document.children(id) {
        shown(document, child, out);
    }
}

#[test]
fn a_rewrite_refused_while_read_only_leaves_the_text_as_it_was() {
    let old = Uuid::from_u128(0x01d0_0000_0000_4000_8000_0000_0000_0001);
    let new = Uuid::from_u128(0x0e00_0000_0000_4000_8000_0000_0000_0002);
    let original = format!("see {}\n", block_url(WORKSPACE_ID, old));
    let mut editor = editor(&original);
    editor.host().set_editable(false);

    editor.replace_child(old, new);
    editor.run();

    assert_eq!(text(&editor), original);
    let root = editor.document().root().expect("the editor built a root");
    let mut painted = String::new();
    shown(editor.document(), root, &mut painted);
    assert!(
        !painted.contains(&new.to_string()),
        "the editor shows a rewrite its block never took"
    );
}
