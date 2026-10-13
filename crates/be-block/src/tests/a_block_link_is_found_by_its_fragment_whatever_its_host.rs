use crate::block_url::{block_url, parse_block_urls};
use uuid::Uuid;

#[test]
fn a_block_link_is_found_by_its_fragment_whatever_its_host() {
    let workspace_id = Uuid::from_u128(1);
    let block_id = Uuid::from_u128(2);
    let fragment = block_url("", workspace_id, block_id);
    assert_eq!(fragment, format!("#block/{workspace_id}/{block_id}"));

    assert_eq!(
        block_url("https://someurl.com/be3/index.html", workspace_id, block_id),
        format!("https://someurl.com/be3/index.html#block/{workspace_id}/{block_id}")
    );

    for base in [
        "http://localhost:9090",
        "http://[::1]:9090/",
        "https://someurl.com/be3/",
        "https://someurl.com/be3/index.html",
    ] {
        let url = block_url(base, workspace_id, block_id);
        let text = format!("see [the notes]({url}) or [{url}].");
        let parsed = parse_block_urls(text.as_bytes());
        assert_eq!(parsed.len(), 2, "{text}");
        for found in parsed {
            assert_eq!(&text[found.range.clone()], url);
            assert_eq!((found.workspace_id, found.block), (workspace_id, block_id));
        }
    }

    let text = format!("bare {fragment} and after-text{fragment}");
    let ranges: Vec<_> = parse_block_urls(text.as_bytes())
        .into_iter()
        .map(|found| text[found.range].to_owned())
        .collect();
    assert_eq!(ranges, [fragment.clone(), fragment]);
}
