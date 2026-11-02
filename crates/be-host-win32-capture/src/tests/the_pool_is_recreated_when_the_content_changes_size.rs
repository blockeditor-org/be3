use super::*;

#[test]
fn the_pool_is_recreated_when_the_content_changes_size() {
    let pool = Size {
        width: 800,
        height: 600,
    };
    let larger = Size {
        width: 1024,
        height: 600,
    };
    assert_eq!(pool_after(pool, Some(pool)), None);
    assert_eq!(pool_after(pool, Some(larger)), Some(larger));
    assert_eq!(pool_after(pool, None), None);
}
