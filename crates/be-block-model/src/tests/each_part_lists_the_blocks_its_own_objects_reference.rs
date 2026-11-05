use super::*;
use crate::BlockRef;

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Links {
    links: List<Link>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Link {
    label: String,
    target: BlockRef,
}

#[test]
fn each_part_lists_the_blocks_its_own_objects_reference() {
    let target = |index: u64| BlockRef(uuid::Uuid::from_u128(u128::from(index) + 0x1000));
    let document = Document::new(&Links {
        links: List::with_ids((0..2000).map(|index| {
            (
                numbered(index),
                Link {
                    label: prose(index, 12),
                    target: target(index),
                },
            )
        })),
    });

    let root = document.to_parts();
    assert!(root.references().is_empty(), "the root holds no link itself");
    let mut listed = Vec::new();
    for part in &root.all()[1..] {
        assert!(!part.references().is_empty());
        listed.extend_from_slice(part.references());
    }
    let expected: Vec<_> = (0..2000).map(|index| *target(index)).collect();
    assert_eq!(listed, expected, "every link is listed once, in order");
}
