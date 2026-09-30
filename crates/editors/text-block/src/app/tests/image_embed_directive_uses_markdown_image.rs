use super::{BLOCK_ID, WORKSPACE_ID, block_url, image_embed_directive};

#[test]
fn image_embed_directive_uses_markdown_image() {
    assert_eq!(
        image_embed_directive(WORKSPACE_ID, &BLOCK_ID, "Pasted Image.png", true),
        format!("![Pasted Image.png]({})", block_url(WORKSPACE_ID, BLOCK_ID))
    );
}
