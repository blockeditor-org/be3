use super::{BLOCK_ID, ORGANIZATION_ID, block_url, image_embed_directive};

#[test]
fn image_embed_directive_uses_plain_url() {
    assert_eq!(
        image_embed_directive(
            "https://example.com/",
            ORGANIZATION_ID,
            &BLOCK_ID,
            "Pasted Image.png",
            false
        ),
        block_url("https://example.com/", ORGANIZATION_ID, BLOCK_ID)
    );
}
