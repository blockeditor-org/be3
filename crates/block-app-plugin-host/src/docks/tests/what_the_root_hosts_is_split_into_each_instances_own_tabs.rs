use super::*;

#[test]
fn what_the_root_hosts_is_split_into_each_instances_own_tabs() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    host(&mut docks, Some(hosted_by_root()));
    let outer = hosted(&mut docks, "outer", OUTER);
    assert_eq!(outer.layout.tree, self::outer().tree);
    assert_eq!(outer.visible, [path(&[], 1)]);
    assert_eq!(outer.focused, Some(path(&[], 2)));
    assert!(outer.tabs.contains(&path(&[], 2)));
    let inner = hosted(&mut docks, "inner", INNER);
    assert_eq!(inner.layout.tree, self::inner().tree);
    assert_eq!(inner.visible, [path(&[], 5)]);
    assert_eq!(inner.tabs, [path(&[], 5)]);
}
