use be_block_content::block_url::block_url;
use be_sdk_type_guest::api::{Change, ChildChange, ReplaceChild, guest::exports::replicas::Guest};
use uuid::Uuid;

use super::*;

const ORGANIZATION: Uuid = Uuid::from_u128(0x7a20_a314_e4aa_4ca7_b7ae_d68c_3249_0d9d);

fn link(block: Uuid) -> String {
    block_url("https://example.com/", ORGANIZATION, block)
}

fn changed(content: &TextContent, change: ChildChange) -> Option<TextContent> {
    pollster::block_on(crate::block_types().apply(
        TextContent::CONTENT_TYPE,
        Some(content.encode()),
        3,
        Change::Child(change),
    ))
    .expect("the change applies")
    .map(|bytes| TextContent::decode(&bytes).unwrap())
}

#[test]
fn a_replaced_child_repoints_every_block_url_naming_it() {
    let old = Uuid::from_u128(0x01d0_0000_0000_4000_8000_0000_0000_0001);
    let new = Uuid::from_u128(0x0e00_0000_0000_4000_8000_0000_0000_0002);
    let other = Uuid::from_u128(0x07e5_0000_0000_4000_8000_0000_0000_0003);
    let text = |block: Uuid| {
        format!(
            "see {}\nnot {}\nand ✓ {}\n",
            link(block),
            link(other),
            link(block)
        )
    };
    let content = TextBlock::of(&text(old));

    let replaced = changed(&content, ChildChange::Replace(ReplaceChild { old, new }))
        .expect("the text takes the replacement");
    assert_eq!(TextBlock::text(&replaced), text(new));
    assert_eq!(replaced.references(), [new, other]);

    // A peer's edit made meanwhile keeps, since the rewrite only touches the ids.
    let typing = TextBlock::insert(&content, 4, 0, b"> ").unwrap();
    let mut both = replaced.clone();
    both.apply(&typing);
    assert_eq!(TextBlock::text(&both), format!("> {}", text(new)));

    let unlinked = Uuid::from_u128(0x0b5e_0000_0000_4000_8000_0000_0000_0004);
    assert!(
        changed(
            &content,
            ChildChange::Replace(ReplaceChild { old: unlinked, new })
        )
        .is_none(),
        "a text naming no such block is left alone"
    );
    assert!(
        changed(&content, ChildChange::Delete(old)).is_none(),
        "a deleted child keeps its URL"
    );
}
