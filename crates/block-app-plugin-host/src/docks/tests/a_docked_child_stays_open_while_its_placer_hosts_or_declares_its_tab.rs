use super::*;

#[test]
fn a_docked_child_stays_open_while_its_placer_hosts_or_declares_its_tab() {
    let mut docks = nested();
    docks.placed(ROOT, &[], None);
    docks.placed(OUTER, &[], None);
    assert!(docks.docked(OUTER));
    assert!(docks.docked(INNER));
    host(&mut docks, DockHold::Hosted(hosted_by_root()));
    assert!(docks.docked(OUTER));
    updates(&mut docks, "shell");
    updates(&mut docks, "inner");

    docks.heard(
        "outer",
        vec![declare(OUTER, Some(declaration("outer", &[(1, "one")])))],
    );
    assert!(!docks.docked(INNER));
    assert_eq!(
        updates(&mut docks, "inner"),
        [(
            INNER,
            DockUpdate::Hosted(DockHostedView {
                view: ViewId::MAIN,
                hosted: None,
            })
        )]
    );
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: Some(declaration("outer", &[(1, "one")])),
            })
        )]
    );

    host(&mut docks, DockHold::Placing);
    assert!(
        docks.docked(OUTER),
        "it stays while the root lays out the tab it hosted it in"
    );
    docks.placed(ROOT, &[], None);
    docks.placed(ROOT, &[], None);
    docks.placed(ROOT, &[], None);
    assert!(
        docks.docked(OUTER),
        "however many reports the root sends before it places it"
    );
    host(&mut docks, DockHold::Released);
    assert!(!docks.docked(OUTER), "until the root lets the tab go");
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::Child(DockDeclaredChild {
                child: OUTER_CHILD,
                declaration: None,
            })
        )]
    );
}
