use super::{BLOCK_ID, ORGANIZATION_ID, block_url, parse_embeds};

#[test]
fn classifies_markdown_image() {
    let url = block_url("https://example.com/", ORGANIZATION_ID, BLOCK_ID);
    let markdown_image = format!("![link]({url})");

    let image = parse_embeds(markdown_image.as_bytes(), ORGANIZATION_ID, true);
    let plain = parse_embeds(url.as_bytes(), ORGANIZATION_ID, true);

    assert_eq!(image.len(), 1);
    assert_eq!(image[0].range, 8..8 + url.len());
    assert!(image[0].large);
    assert_eq!(plain.len(), 1);
    assert!(!plain[0].large);
}
