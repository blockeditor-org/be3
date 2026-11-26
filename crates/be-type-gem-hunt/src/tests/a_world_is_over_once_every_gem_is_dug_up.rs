use super::*;

#[test]
fn a_world_is_over_once_every_gem_is_dug_up() {
    let (ada, bo) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let start = HuntContent::new(&Hunt {
        gems: vec![3, 20],
        ..Hunt::default()
    });
    let content = edited(&start, [Hunt::dig(3, ada), Hunt::dig(4, bo)]);
    let hunt = content.root();
    assert!(!hunt.may_dig(3), "a dug cell is dug once");
    assert!(!hunt.may_dig(CELLS), "nor is a cell outside the world");
    assert!(hunt.may_dig(20));
    assert_eq!((hunt.found_by(ada), hunt.found_by(bo)), (1, 0));
    assert!(!hunt.over());

    let content = edited(&content, [Hunt::dig(20, bo)]);
    let hunt = content.root();
    assert_eq!((hunt.found(), hunt.found_by(bo)), (2, 1));
    assert!(hunt.over());
    assert!(!hunt.may_dig(21), "nothing is dug once every gem is found");
}
