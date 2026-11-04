use super::*;

#[test]
fn the_root_places_only_visible_tabs_of_its_docked_descendants() {
    let mut docks = nested();
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD, INNER_CHILD], 5)),
        Some((INNER_BLOCK, 5))
    );
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD], 1)),
        Some((OUTER_BLOCK, 1))
    );
    assert_eq!(docks.resolve(ROOT, &path(&[OUTER_CHILD], 2)), None);
    assert_eq!(docks.resolve(ROOT, &path(&[ChildId(99)], 1)), None);
    assert_eq!(docks.resolve(ROOT, &path(&[INNER_CHILD], 5)), None);
    assert_eq!(docks.resolve(ROOT, &path(&[], 1)), None);
    assert_eq!(docks.resolve(OUTER, &path(&[INNER_CHILD], 5)), None);
}
