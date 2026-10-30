use super::*;

#[test]
fn blocks_are_carved_from_the_pages_grown_for_them() {
    let blocks = Blocks {
        size: 48,
        align: 16,
        pages: 1,
    };
    let carved = blocks.carve(65536, 1);
    assert_eq!(carved.len(), 65536 / 48);
    assert_eq!(carved[..2], [65536, 65584]);
    assert!(carved.iter().all(|at| at % 16 == 0 && at + 48 <= 131072));
    assert!(
        Blocks {
            size: 0,
            align: 4,
            pages: 0
        }
        .carve(0, 1)
        .is_empty()
    );
}
