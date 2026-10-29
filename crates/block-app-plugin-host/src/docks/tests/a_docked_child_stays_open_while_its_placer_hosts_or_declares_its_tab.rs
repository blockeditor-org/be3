use super::*;

#[test]
fn a_docked_child_stays_open_while_its_placer_hosts_or_declares_its_tab() {
    let mut docks = nested();
    docks.placed(ROOT, &[]);
    docks.placed(OUTER, &[]);
    assert!(docks.docked(OUTER));
    assert!(docks.docked(INNER));
    host(&mut docks, Some(hosted_by_root()));
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
            DockUpdate::Hosted(DockHostedRegion {
                region: EditorRegion::Frame,
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

    host(&mut docks, None);
    assert!(!docks.docked(OUTER));
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
