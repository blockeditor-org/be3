use super::*;

#[test]
fn a_closed_instance_withdraws_its_dock_from_the_root() {
    let mut docks = nested();
    updates(&mut docks, "shell");
    docks.forget(INNER);
    assert!(!docks.docked(INNER));
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: Some(outer()),
            })
        )]
    );
}
