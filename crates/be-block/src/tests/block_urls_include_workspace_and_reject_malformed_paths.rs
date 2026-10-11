use crate::block_url::{block_url, parse_block_urls};
use uuid::Uuid;

#[test]
fn block_urls_include_workspace_and_reject_malformed_paths() {
    let workspace_id = Uuid::new_v4();
    let block_id = Uuid::new_v4();
    let url = block_url("https://example.com/be3/", workspace_id, block_id);
    assert_eq!(
        url,
        format!("https://example.com/be3/#block/{workspace_id}/{block_id}")
    );
    let parsed = parse_block_urls(url.as_bytes());
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].workspace_id, workspace_id);
    assert_eq!(parsed[0].block, block_id);
    assert_eq!(parsed[0].range, 0..url.len());

    for malformed in [
        format!("https://example.com/#block/{block_id}"),
        format!("https://example.com/#block/not-a-uuid/{block_id}"),
        format!("https://example.com/#block/{workspace_id}/not-a-uuid"),
        format!("{url}/extra"),
        format!("https://example.com/#blocks/{workspace_id}/{block_id}"),
        format!("https://example.com/{workspace_id}/{block_id}"),
    ] {
        assert!(
            parse_block_urls(malformed.as_bytes()).is_empty(),
            "{malformed}"
        );
    }
}
