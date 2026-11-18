use super::*;

#[test]
fn a_restarted_app_s_views_describe_nothing_and_offer_no_actions() {
    let mut run = running();
    let mut views = described();
    assert_eq!(grafted_nodes(&mut views.graft), 3);

    run.restart(
        Session::new("Test", None),
        Preferences::default(),
        "demo",
        &mut Docks::default(),
        &mut views,
    );
    assert_eq!(
        grafted_nodes(&mut views.graft),
        1,
        "the pane stays, empty, until the new run describes it"
    );
    assert!(views.actions.is_empty());
}
