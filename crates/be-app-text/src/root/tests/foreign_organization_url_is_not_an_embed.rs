use super::{BLOCK_ID, ORGANIZATION_ID, block_url, parse_embeds};
use uuid::Uuid;

#[test]
fn foreign_organization_url_is_not_an_embed() {
    let url = block_url("https://example.com/", Uuid::new_v4(), BLOCK_ID);
    assert!(parse_embeds(url.as_bytes(), ORGANIZATION_ID, true).is_empty());
}
