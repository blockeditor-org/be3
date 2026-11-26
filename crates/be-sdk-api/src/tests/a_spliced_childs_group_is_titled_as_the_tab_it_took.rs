use super::*;

#[test]
fn a_spliced_childs_group_is_titled_as_the_tab_it_took() {
    let child = ChildId(9);
    let prefix = dock_prefix("nested", [9; 16]);
    let mut spliced = declaration();
    let host_title = spliced
        .tab(&DockTabPath::own(3))
        .map(|tab| tab.title.clone())
        .expect("the outer dock declares the tab");
    assert_eq!(
        spliced.splice(3, child, &prefix, &child_declaration()),
        Ok(true)
    );
    let nodes = spliced.tree.nest().unwrap();
    let groups = nodes.groups();
    let group = groups
        .iter()
        .find(|group| group.child == Some(child))
        .expect("the child's dock is a group in place of the tab");
    assert_eq!(
        group.title, host_title,
        "the group keeps the title of the tab it took, not its own tabs'"
    );
}
