use super::*;

#[test]
fn a_docked_child_that_declares_nothing_stays_a_plain_child() {
    let mut docks = Docks::default();
    docks.register(ROOT, "shell", Some(Uuid::from_u128(1)));
    docks.register(OUTER, "outer", Some(OUTER_BLOCK));
    docks.register(INNER, "inner", Some(INNER_BLOCK));
    docks.placed(
        ROOT,
        &[DockedChild {
            child: OUTER_CHILD,
            tab: 100,
            block: OUTER_BLOCK,
        }],
    );
    docks.heard("outer", vec![declare(OUTER, Some(outer()))]);
    docks.placed(
        OUTER,
        &[DockedChild {
            child: INNER_CHILD,
            tab: 2,
            block: INNER_BLOCK,
        }],
    );
    assert!(docks.docked(INNER));
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
    assert!(updates(&mut docks, "inner").is_empty());
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD, INNER_CHILD], 5)),
        None
    );

    docks.placed(OUTER, &[]);
    assert!(!docks.docked(INNER));
    assert!(docks.docked(OUTER));
}
