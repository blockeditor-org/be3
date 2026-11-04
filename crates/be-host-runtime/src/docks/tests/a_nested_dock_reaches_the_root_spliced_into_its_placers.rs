use super::*;

#[test]
fn a_nested_dock_reaches_the_root_spliced_into_its_placers() {
    let mut docks = nested();
    assert!(docks.docked(OUTER));
    assert!(docks.docked(INNER));
    assert!(!docks.docked(ROOT));
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: Some(spliced()),
            })
        )]
    );
    assert!(updates(&mut docks, "outer").is_empty());
    assert!(updates(&mut docks, "inner").is_empty());
}
